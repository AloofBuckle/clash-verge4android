use boa_engine::{Context, JsString, JsValue, Source, js_string, native_function::NativeFunction, property::Attribute};
use parking_lot::Mutex;
use serde_yaml_ng::Mapping;
use std::{
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

use super::lowercase_top_level;

const MAX_OUTPUTS: usize = 1000;
const MAX_OUTPUT_SIZE: usize = 1024 * 1024;
const MAX_JSON_SIZE: usize = 10 * 1024 * 1024;
const MAX_LOOP_ITERATIONS: u64 = 10_000_000;
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) type ScriptLogs = Vec<(String, String)>;

pub(crate) fn use_script(script: &str, config: Mapping, name: &str) -> (Mapping, ScriptLogs) {
    let json = match serde_json::to_string(&lowercase_top_level(&config)) {
        Ok(json) if json.len() <= MAX_JSON_SIZE => json,
        Ok(_) => {
            return (
                config,
                vec![(
                    "exception".into(),
                    "Configuration size exceeds maximum allowed size".into(),
                )],
            );
        }
        Err(error) => return (config, vec![("exception".into(), error.to_string())]),
    };

    let script = script.to_owned();
    let name = name.to_owned();
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let _ = sender.send(eval_script(&script, &json, &name));
    });
    match receiver.recv_timeout(SCRIPT_TIMEOUT) {
        Ok((Ok(result), logs)) => (result, logs),
        Ok((Err(reason), mut logs)) => {
            logs.push(("exception".into(), reason));
            (config, logs)
        }
        Err(mpsc::RecvTimeoutError::Timeout) => (
            config,
            vec![(
                "exception".into(),
                format!("script execution timed out after {SCRIPT_TIMEOUT:?}"),
            )],
        ),
        Err(mpsc::RecvTimeoutError::Disconnected) => (
            config,
            vec![("exception".into(), "script task terminated unexpectedly".into())],
        ),
    }
}

type EvalOutcome = (Result<Mapping, String>, ScriptLogs);

fn eval_script(script: &str, config_json: &str, name: &str) -> EvalOutcome {
    let mut context = Context::default();
    context
        .runtime_limits_mut()
        .set_loop_iteration_limit(MAX_LOOP_ITERATIONS);

    let outputs = Arc::new(Mutex::new(Vec::new()));
    let total_size = Arc::new(Mutex::new(0usize));
    let outputs_clone = Arc::clone(&outputs);
    let total_size_clone = Arc::clone(&total_size);
    let bail = |reason: String| (Err(reason), outputs.lock().to_vec());

    let _ = context.register_global_builtin_callable("__verge_log__".into(), 2, unsafe {
        NativeFunction::from_closure(move |_: &JsValue, args: &[JsValue], context: &mut Context| {
            let level = args
                .first()
                .ok_or_else(|| boa_engine::JsError::from_opaque(JsString::from("Missing level argument").into()))?
                .to_string(context)?
                .to_std_string()
                .map_err(|_| {
                    boa_engine::JsError::from_opaque(JsString::from("Failed to convert level to string").into())
                })?;
            let data = args
                .get(1)
                .ok_or_else(|| boa_engine::JsError::from_opaque(JsString::from("Missing data argument").into()))?
                .to_string(context)?
                .to_std_string()
                .map_err(|_| {
                    boa_engine::JsError::from_opaque(JsString::from("Failed to convert data to string").into())
                })?;
            if outputs_clone.lock().len() >= MAX_OUTPUTS {
                return Err(boa_engine::JsError::from_opaque(
                    JsString::from("Maximum number of log outputs exceeded").into(),
                ));
            }
            let mut size = total_size_clone.lock();
            let new_size = *size + level.len() + data.len();
            if new_size > MAX_OUTPUT_SIZE {
                return Err(boa_engine::JsError::from_opaque(
                    JsString::from("Maximum output size exceeded").into(),
                ));
            }
            *size = new_size;
            drop(size);
            outputs_clone.lock().push((level, data));
            Ok(JsValue::undefined())
        })
    });

    let _ = context.eval(Source::from_bytes(
        r#"var console = Object.freeze({
        log(data){__verge_log__("log",JSON.stringify(data, null, 2))},
        info(data){__verge_log__("info",JSON.stringify(data, null, 2))},
        error(data){__verge_log__("error",JSON.stringify(data, null, 2))},
        debug(data){__verge_log__("debug",JSON.stringify(data, null, 2))},
        warn(data){__verge_log__("warn",JSON.stringify(data, null, 2))},
        table(data){__verge_log__("table",JSON.stringify(data, null, 2))},
      });"#,
    ));

    if let Err(error) = context.register_global_property(
        js_string!("__verge_config__"),
        JsValue::from(JsString::from(config_json)),
        Attribute::all(),
    ) {
        return bail(format!("failed to bind config for script: {error}"));
    }

    let safe_name = escape_js_string_for_single_quote(name);
    if safe_name.len() > 1024 {
        return bail("Name parameter too long".into());
    }
    let code = format!(
        r"try{{
        {script};
        JSON.stringify(main(JSON.parse(globalThis.__verge_config__),'{safe_name}')||'')
      }} catch(err) {{
        `__error_flag__ ${{err.toString()}}`
      }}"
    );
    let result = match context.eval(Source::from_bytes(code.as_str())) {
        Ok(result) if result.is_string() => result,
        _ => return bail("main function should return object".into()),
    };
    let result = match result.to_string(&mut context) {
        Ok(result) => result,
        Err(error) => return bail(format!("Failed to convert JS result to string: {error}")),
    };
    let result = match result.to_std_string() {
        Ok(result) => result,
        Err(_) => return bail("Failed to convert JS string to std string".into()),
    };
    if result.len() > MAX_JSON_SIZE {
        return bail("Script result exceeds maximum allowed size".into());
    }
    match parse_json_safely(&result) {
        Ok(config) => (Ok(lowercase_top_level(&config)), outputs.lock().to_vec()),
        Err(error) => bail(error),
    }
}

fn parse_json_safely(value: &str) -> Result<Mapping, String> {
    if value.len() > MAX_JSON_SIZE {
        return Err("JSON string too large".into());
    }
    serde_json::from_str::<Mapping>(strip_outer_quotes(value)).map_err(|_| "Script execution failed".into())
}

fn strip_outer_quotes(value: &str) -> &str {
    let value = value.trim();
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"')) || (value.starts_with('\'') && value.ends_with('\'')))
    {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

fn escape_js_string_for_single_quote(value: &str) -> String {
    let value = if value.len() > 10240 { &value[..10240] } else { value };
    value
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml_ng::Value;

    #[test]
    fn script_receives_profile_name_and_captures_console_output() {
        let config: Mapping = serde_yaml_ng::from_str("rules: []\n").unwrap();
        let (result, logs) = use_script(
            "function main(config, profileName) { console.info(profileName); config['x-script'] = profileName; return config; }",
            config,
            "Main",
        );
        assert_eq!(result.get("x-script").and_then(Value::as_str), Some("Main"));
        assert_eq!(logs.first().map(|(level, _)| level.as_str()), Some("info"));
    }
}
