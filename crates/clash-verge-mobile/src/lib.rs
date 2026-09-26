mod links;
mod script;
pub use links::parse_links;

use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;
use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub const MAX_PROFILE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub source: Option<String>,
    #[serde(default)]
    pub home: Option<String>,
    #[serde(default)]
    pub extra: Option<ProfileExtra>,
    pub yaml: String,
    pub updated_at: u64,
    #[serde(default)]
    pub selected: Vec<SelectedProxy>,
    #[serde(default)]
    pub proxy_chain: Option<ProxyChain>,
    #[serde(default)]
    pub option: ProfileOptions,
    #[serde(default)]
    pub merge_yaml: Option<String>,
    #[serde(default)]
    pub rules_yaml: Option<String>,
    #[serde(default)]
    pub proxies_yaml: Option<String>,
    #[serde(default)]
    pub groups_yaml: Option<String>,
    #[serde(default)]
    pub script_js: Option<String>,
    #[serde(default)]
    pub dns_override_enabled: Option<bool>,
    #[serde(default)]
    pub dns_override_yaml: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileExtra {
    pub upload: u64,
    pub download: u64,
    pub total: u64,
    pub expire: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeEnhancements<'a> {
    pub rules: Option<&'a str>,
    pub proxies: Option<&'a str>,
    pub groups: Option<&'a str>,
    pub global_merge: Option<&'a str>,
    pub profile_merge: Option<&'a str>,
    pub global_script: Option<&'a str>,
    pub profile_script: Option<&'a str>,
    pub profile_name: &'a str,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProfileOptions {
    pub user_agent: Option<String>,
    pub with_proxy: Option<bool>,
    pub self_proxy: Option<bool>,
    pub update_interval: Option<u64>,
    pub timeout_seconds: Option<u64>,
    pub danger_accept_invalid_certs: Option<bool>,
    pub allow_auto_update: Option<bool>,
}

impl ProfileOptions {
    pub fn normalized(mut self) -> Result<Self, String> {
        self.user_agent = self
            .user_agent
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if self
            .user_agent
            .as_ref()
            .is_some_and(|value| value.len() > 512 || value.contains(['\r', '\n']))
        {
            return Err("Profile User-Agent is invalid".into());
        }
        if self
            .timeout_seconds
            .is_some_and(|seconds| !(1..=300).contains(&seconds))
        {
            return Err("Profile HTTP timeout must be between 1 and 300 seconds".into());
        }
        if self.with_proxy.unwrap_or(false) && self.self_proxy.unwrap_or(false) {
            return Err("System proxy and Clash proxy update modes are mutually exclusive".into());
        }
        Ok(self)
    }

    pub fn auto_update_enabled(&self) -> bool {
        self.allow_auto_update.unwrap_or(true)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedProxy {
    pub group: String,
    pub member: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyChain {
    pub target_group: String,
    pub nodes: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub active_id: Option<String>,
    pub profiles: Vec<Profile>,
    #[serde(default)]
    pub global_merge_yaml: Option<String>,
    #[serde(default)]
    pub global_script_js: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub nodes: Vec<Node>,
    pub groups: Vec<Group>,
    pub rules: Vec<String>,
    pub mode: String,
    pub provider_count: usize,
}

#[derive(Debug, Serialize)]
pub struct Node {
    pub name: String,
    pub protocol: String,
}

#[derive(Debug, Serialize)]
pub struct Group {
    pub name: String,
    pub kind: String,
    pub members: Vec<String>,
    pub providers: Vec<String>,
}

fn strings(value: Option<&Value>, label: &str) -> Result<Vec<String>, String> {
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Sequence(items)) => items
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| format!("{label} must contain strings"))
            })
            .collect(),
        _ => Err(format!("{label} must be an array")),
    }
}

fn sequence<'a>(root: &'a Value, key: &str) -> Result<&'a [Value], String> {
    match root.get(key) {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Sequence(items)) => Ok(items),
        _ => Err(format!("{key} must be an array")),
    }
}

fn required_string<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("Missing or invalid {key}"))
}

fn lowercase_top_level(mapping: &serde_yaml_ng::Mapping) -> serde_yaml_ng::Mapping {
    mapping
        .iter()
        .filter_map(|(key, value)| {
            key.as_str()
                .map(|key| (Value::String(key.to_ascii_lowercase()), value.clone()))
        })
        .collect()
}

fn deep_merge_value(existing: &mut Value, overlay: Value) {
    match (existing, overlay) {
        (Value::Mapping(existing), Value::Mapping(overlay)) => {
            for (key, value) in overlay {
                if let Some(current) = existing.get_mut(&key) {
                    deep_merge_value(current, value);
                } else {
                    existing.insert(key, value);
                }
            }
        }
        (existing, overlay) => *existing = overlay,
    }
}

fn parse_merge_yaml(yaml: &str) -> Result<serde_yaml_ng::Mapping, String> {
    if yaml.len() > MAX_PROFILE_BYTES {
        return Err("Merge YAML exceeds 4 MiB".into());
    }
    let value: Value = serde_yaml_ng::from_str(yaml).map_err(|e| format!("Merge YAML: {e}"))?;
    value.as_mapping().cloned().ok_or("Merge YAML must be a mapping".into())
}

fn apply_merge_yaml(map: &mut serde_yaml_ng::Mapping, yaml: Option<&str>) -> Result<(), String> {
    let Some(yaml) = yaml.map(str::trim).filter(|yaml| !yaml.is_empty()) else {
        return Ok(());
    };
    let merge = lowercase_top_level(&parse_merge_yaml(yaml)?);
    for (key, value) in merge {
        match (map.get_mut(&key), value) {
            (Some(Value::Mapping(existing)), Value::Mapping(overlay)) if key.as_str() == Some("dns") => {
                existing.extend(overlay);
            }
            (Some(existing), value) if key.as_str() != Some("hosts") => {
                deep_merge_value(existing, value);
            }
            (_, value) => {
                map.insert(key, value);
            }
        }
    }
    Ok(())
}

pub fn validate_merge_yaml(yaml: &str) -> Result<(), String> {
    let _ = parse_merge_yaml(yaml)?;
    Ok(())
}

fn validate_script_text(script: &str) -> Result<(), String> {
    if script.len() > MAX_PROFILE_BYTES {
        return Err("Script exceeds 4 MiB".into());
    }
    Ok(())
}

fn apply_script_js(map: &mut serde_yaml_ng::Mapping, script: Option<&str>, profile_name: &str) {
    let Some(script) = script.map(str::trim).filter(|script| !script.is_empty()) else {
        return;
    };
    let (next, _logs) = script::use_script(script, std::mem::take(map), profile_name);
    *map = next;
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct SeqMap {
    prepend: serde_yaml_ng::Sequence,
    append: serde_yaml_ng::Sequence,
    delete: Vec<String>,
}

fn parse_seq_yaml(yaml: &str, label: &str) -> Result<SeqMap, String> {
    if yaml.len() > MAX_PROFILE_BYTES {
        return Err(format!("{label} enhancement exceeds 4 MiB"));
    }
    serde_yaml_ng::from_str::<SeqMap>(yaml).map_err(|e| format!("{label} enhancement YAML: {e}"))
}

fn collect_proxy_names(seq: &serde_yaml_ng::Sequence) -> Vec<String> {
    seq.iter()
        .filter_map(|item| match item {
            Value::Mapping(map) => map.get("name").and_then(Value::as_str).map(str::to_owned),
            Value::String(name) => Some(name.to_owned()),
            _ => None,
        })
        .collect()
}

fn is_selector_group(group: &serde_yaml_ng::Mapping) -> bool {
    group
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| matches!(kind.to_ascii_lowercase().as_str(), "select" | "selector"))
}

fn apply_seq_map(mut config: serde_yaml_ng::Mapping, seq: SeqMap, field: &str) -> serde_yaml_ng::Mapping {
    let SeqMap {
        prepend,
        append,
        delete,
    } = seq;

    let added_proxy_names = if field == "proxies" {
        let mut names = collect_proxy_names(&prepend);
        names.extend(collect_proxy_names(&append));
        let mut seen = HashSet::new();
        names
            .into_iter()
            .filter(|name| seen.insert(name.clone()))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let mut updated = serde_yaml_ng::Sequence::new();
    updated.extend(prepend);
    if let Some(Value::Sequence(existing)) = config.remove(field) {
        updated.extend(existing.into_iter().filter(|item| {
            match item {
                Value::String(value) => !delete.contains(value),
                Value::Mapping(map) => map
                    .get("name")
                    .and_then(Value::as_str)
                    .is_none_or(|name| !delete.iter().any(|deleted| deleted == name)),
                _ => true,
            }
        }));
    }
    updated.extend(append);
    config.insert(Value::String(field.into()), Value::Sequence(updated));

    if field != "proxies" {
        return config;
    }
    let Some(groups_value) = config.remove("proxy-groups") else {
        return config;
    };
    let Value::Sequence(groups) = groups_value else {
        config.insert(Value::String("proxy-groups".into()), groups_value);
        return config;
    };
    let mut updated_groups = serde_yaml_ng::Sequence::new();
    let mut added_to_selector = false;
    for group in groups {
        let Value::Mapping(mut group_map) = group else {
            updated_groups.push(group);
            continue;
        };
        let mut group_proxies = match group_map.remove("proxies") {
            Some(Value::Sequence(proxies)) => Some(
                proxies
                    .into_iter()
                    .filter(|proxy| {
                        proxy
                            .as_str()
                            .is_none_or(|name| !delete.iter().any(|deleted| deleted == name))
                    })
                    .collect::<serde_yaml_ng::Sequence>(),
            ),
            Some(value) => {
                group_map.insert(Value::String("proxies".into()), value);
                None
            }
            None => None,
        };
        if !added_to_selector && !added_proxy_names.is_empty() && is_selector_group(&group_map) {
            let base = group_proxies.unwrap_or_default();
            let mut merged = serde_yaml_ng::Sequence::new();
            let mut seen = HashSet::new();
            for name in &added_proxy_names {
                if seen.insert(name.clone()) {
                    merged.push(Value::String(name.clone()));
                }
            }
            for value in base {
                if let Value::String(name) = &value
                    && !seen.insert(name.clone())
                {
                    continue;
                }
                merged.push(value);
            }
            group_proxies = Some(merged);
            added_to_selector = true;
        }
        if let Some(group_proxies) = group_proxies {
            group_map.insert(Value::String("proxies".into()), Value::Sequence(group_proxies));
        }
        updated_groups.push(Value::Mapping(group_map));
    }
    config.insert(Value::String("proxy-groups".into()), Value::Sequence(updated_groups));
    config
}

fn apply_seq_yaml(
    map: &mut serde_yaml_ng::Mapping,
    yaml: Option<&str>,
    field: &str,
    label: &str,
) -> Result<(), String> {
    let Some(yaml) = yaml.map(str::trim).filter(|yaml| !yaml.is_empty()) else {
        return Ok(());
    };
    let next = apply_seq_map(std::mem::take(map), parse_seq_yaml(yaml, label)?, field);
    *map = next;
    Ok(())
}

pub fn validate_sequence_enhancement(yaml: &str, kind: &str) -> Result<(), String> {
    let label = match kind {
        "rules" => "Rules",
        "proxies" => "Proxies",
        "groups" => "Groups",
        _ => return Err("Unknown sequence enhancement kind".into()),
    };
    let _ = parse_seq_yaml(yaml, label)?;
    Ok(())
}

// Keep the original YAML, including fields not understood by this preview parser.
pub fn inspect(yaml: &str) -> Result<Summary, String> {
    if yaml.is_empty() || yaml.len() > MAX_PROFILE_BYTES {
        return Err("Configuration must be between 1 byte and 4 MiB".into());
    }
    let root: Value = serde_yaml_ng::from_str(yaml).map_err(|e| format!("YAML: {e}"))?;
    if !root.is_mapping() {
        return Err("Expected a Mihomo YAML mapping".into());
    }
    if !["proxies", "proxy-groups", "proxy-providers", "rules"]
        .iter()
        .any(|k| root.get(*k).is_some())
    {
        return Err("Not a Mihomo configuration: missing proxies, groups, providers and rules".into());
    }
    let mut names = HashSet::new();
    let mut nodes = Vec::new();
    for item in sequence(&root, "proxies")? {
        let name = required_string(item, "name")?.to_owned();
        if !names.insert(name.clone()) {
            return Err(format!("Duplicate proxy name: {name}"));
        }
        nodes.push(Node {
            name,
            protocol: required_string(item, "type")?.to_owned(),
        });
    }
    let mut groups = Vec::new();
    for item in sequence(&root, "proxy-groups")? {
        let name = required_string(item, "name")?.to_owned();
        if !names.insert(name.clone()) {
            return Err(format!("Duplicate proxy/group name: {name}"));
        }
        groups.push(Group {
            name,
            kind: required_string(item, "type")?.to_owned(),
            members: strings(item.get("proxies"), "group.proxies")?,
            providers: strings(item.get("use"), "group.use")?,
        });
    }
    let rules = strings(root.get("rules"), "rules")?;
    let mode = root.get("mode").and_then(Value::as_str).unwrap_or("rule").to_owned();
    let provider_count = match root.get("proxy-providers") {
        None | Some(Value::Null) => 0,
        Some(Value::Mapping(map)) => map.len(),
        _ => return Err("proxy-providers must be a mapping".into()),
    };
    Ok(Summary {
        nodes,
        groups,
        rules,
        mode,
        provider_count,
    })
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOverrides {
    pub ipv6: Option<bool>,
    pub unified_delay: Option<bool>,
    pub log_level: Option<String>,
    pub mode: Option<String>,
    pub tcp_concurrent: Option<bool>,
    pub find_process_mode: Option<String>,
    pub tun_stack: Option<String>,
    pub tun_strict_route: Option<bool>,
    pub tun_auto_detect_interface: Option<bool>,
    pub tun_dns_hijack: Option<Vec<String>>,
    pub tun_mtu: Option<u32>,
    pub dns_override_enabled: Option<bool>,
    pub dns_override_yaml: Option<String>,
    pub allow_lan: Option<bool>,
    pub bind_address: Option<String>,
    pub mixed_port: Option<u16>,
    pub http_port: Option<u16>,
    pub socks_port: Option<u16>,
    pub redir_port: Option<u16>,
    pub tproxy_port: Option<u16>,
    pub external_controller_enabled: Option<bool>,
    pub external_controller: Option<String>,
    pub external_controller_secret: Option<String>,
    pub external_controller_allow_private_network: Option<bool>,
    pub external_controller_allow_origins: Option<Vec<String>>,
}

fn validate_external_controller_address(address: &str) -> Result<String, String> {
    let address = address.trim();
    if address.is_empty() || address.len() > 256 || address.chars().any(char::is_whitespace) {
        return Err("Invalid external controller address".into());
    }
    let Some((host, port)) = address.rsplit_once(':') else {
        return Err("External controller address must include a port".into());
    };
    if host.is_empty() || port.parse::<u16>().ok().filter(|port| *port > 0).is_none() {
        return Err("External controller address has an invalid host or port".into());
    }
    Ok(address.to_owned())
}

fn apply_external_controller_overrides(
    map: &mut serde_yaml_ng::Mapping,
    overrides: &RuntimeOverrides,
) -> Result<(), String> {
    if !overrides.external_controller_enabled.unwrap_or(false) {
        return Ok(());
    }
    let address =
        validate_external_controller_address(overrides.external_controller.as_deref().unwrap_or("127.0.0.1:9090"))?;
    let secret = overrides.external_controller_secret.as_deref().unwrap_or("").trim();
    if secret.is_empty() || secret.len() > 512 {
        return Err("External controller secret must have 1-512 characters".into());
    }
    let origins = overrides.external_controller_allow_origins.clone().unwrap_or_default();
    if origins.len() > 64
        || origins
            .iter()
            .any(|origin| origin.trim().is_empty() || origin.len() > 1024)
    {
        return Err("External controller CORS origins are invalid".into());
    }
    map.insert(Value::String("external-controller".into()), Value::String(address));
    map.insert(Value::String("secret".into()), Value::String(secret.to_owned()));
    let mut cors = serde_yaml_ng::Mapping::new();
    cors.insert(
        Value::String("allow-private-network".into()),
        Value::Bool(overrides.external_controller_allow_private_network.unwrap_or(true)),
    );
    cors.insert(
        Value::String("allow-origins".into()),
        Value::Sequence(origins.into_iter().map(Value::String).collect()),
    );
    map.insert(Value::String("external-controller-cors".into()), Value::Mapping(cors));
    Ok(())
}

fn apply_listener_overrides(map: &mut serde_yaml_ng::Mapping, overrides: &RuntimeOverrides) -> Result<(), String> {
    let ports = [
        ("mixed-port", overrides.mixed_port.unwrap_or(0)),
        ("port", overrides.http_port.unwrap_or(0)),
        ("socks-port", overrides.socks_port.unwrap_or(0)),
        ("redir-port", overrides.redir_port.unwrap_or(0)),
        ("tproxy-port", overrides.tproxy_port.unwrap_or(0)),
    ];
    let mut seen = HashSet::new();
    for (key, port) in ports {
        if port == 0 {
            continue;
        }
        if !seen.insert(port) {
            return Err(format!("Proxy listener port is duplicated: {port}"));
        }
        map.insert(Value::String(key.into()), Value::Number(u64::from(port).into()));
    }
    map.insert(
        Value::String("allow-lan".into()),
        Value::Bool(overrides.allow_lan.unwrap_or(false)),
    );
    if let Some(bind) = overrides.bind_address.as_deref() {
        let bind = bind.trim();
        if bind.is_empty() || bind.len() > 128 || bind.chars().any(char::is_whitespace) {
            return Err("Invalid proxy bind address".into());
        }
        map.insert(Value::String("bind-address".into()), Value::String(bind.into()));
    }
    Ok(())
}

fn dns_override_mapping(overrides: &RuntimeOverrides) -> Result<Option<serde_yaml_ng::Mapping>, String> {
    if !overrides.dns_override_enabled.unwrap_or(false) {
        return Ok(None);
    }
    let yaml = overrides.dns_override_yaml.as_deref().unwrap_or("").trim();
    if yaml.is_empty() {
        return Err("DNS override is enabled but empty".into());
    }
    if yaml.len() > 256 * 1024 {
        return Err("DNS override exceeds 256 KiB".into());
    }
    let value: Value = serde_yaml_ng::from_str(yaml).map_err(|e| format!("DNS override YAML: {e}"))?;
    let mapping = value.as_mapping().ok_or("DNS override must be a YAML mapping")?;
    for key in mapping.keys() {
        let Some(key) = key.as_str() else {
            return Err("DNS override keys must be strings".into());
        };
        if !matches!(key, "dns" | "hosts") {
            return Err(format!("DNS override may only contain dns/hosts, found: {key}"));
        }
    }
    if let Some(dns) = mapping.get(Value::String("dns".into()))
        && !dns.is_mapping()
    {
        return Err("DNS override dns must be a mapping".into());
    }
    if let Some(hosts) = mapping.get(Value::String("hosts".into()))
        && !hosts.is_mapping()
    {
        return Err("DNS override hosts must be a mapping".into());
    }
    Ok(Some(mapping.clone()))
}

fn apply_dns_override(map: &mut serde_yaml_ng::Mapping, overrides: &RuntimeOverrides) -> Result<(), String> {
    let Some(override_map) = dns_override_mapping(overrides)? else {
        return Ok(());
    };
    for key in ["dns", "hosts"] {
        let value_key = Value::String(key.into());
        if let Some(value) = override_map.get(&value_key) {
            map.insert(value_key, value.clone());
        }
    }
    Ok(())
}

fn apply_runtime_overrides(map: &mut serde_yaml_ng::Mapping, overrides: &RuntimeOverrides) -> Result<(), String> {
    if let Some(ipv6) = overrides.ipv6 {
        map.insert(Value::String("ipv6".into()), Value::Bool(ipv6));
    }
    if let Some(unified_delay) = overrides.unified_delay {
        map.insert(Value::String("unified-delay".into()), Value::Bool(unified_delay));
    }
    if let Some(log_level) = overrides.log_level.as_deref() {
        if !matches!(log_level, "debug" | "info" | "warning" | "warn" | "error" | "silent") {
            return Err("Invalid Mihomo log level".into());
        }
        map.insert(
            Value::String("log-level".into()),
            Value::String(if log_level == "warn" {
                "warning".into()
            } else {
                log_level.into()
            }),
        );
    }
    if let Some(mode) = overrides.mode.as_deref() {
        let mode = mode.to_ascii_lowercase();
        if !matches!(mode.as_str(), "rule" | "global" | "direct") {
            return Err("Invalid Mihomo mode".into());
        }
        map.insert(Value::String("mode".into()), Value::String(mode));
    }
    if let Some(tcp_concurrent) = overrides.tcp_concurrent {
        map.insert(Value::String("tcp-concurrent".into()), Value::Bool(tcp_concurrent));
    }
    if let Some(find_process_mode) = overrides.find_process_mode.as_deref() {
        let find_process_mode = find_process_mode.to_ascii_lowercase();
        if !matches!(find_process_mode.as_str(), "off" | "strict" | "always") {
            return Err("Invalid Mihomo find-process-mode".into());
        }
        map.insert(
            Value::String("find-process-mode".into()),
            Value::String(find_process_mode),
        );
    }
    apply_dns_override(map, overrides)?;
    apply_listener_overrides(map, overrides)?;
    apply_external_controller_overrides(map, overrides)?;
    Ok(())
}

fn apply_proxy_chain(map: &mut serde_yaml_ng::Mapping, chain: Option<&ProxyChain>) -> Result<(), String> {
    let Some(chain) = chain else {
        return Ok(());
    };
    if chain.target_group.trim().is_empty() {
        return Err("Proxy chain target group is required".into());
    }
    if chain.nodes.len() < 2 {
        return Err("Proxy chain requires at least two nodes".into());
    }
    let mut seen = HashSet::new();
    for node in &chain.nodes {
        if node.trim().is_empty() || !seen.insert(node) {
            return Err("Proxy chain nodes must be non-empty and unique".into());
        }
    }
    let proxies = map
        .get_mut(Value::String("proxies".into()))
        .and_then(Value::as_sequence_mut)
        .ok_or("Proxy chain requires a proxies sequence")?;
    // Match Clash Verge Rev's runtime chain behavior: chain mode owns the
    // temporary dialer-proxy graph, so clear any pre-existing runtime links
    // before rebuilding the requested ordered chain. The saved profile YAML is
    // never mutated.
    for proxy in proxies.iter_mut() {
        if let Some(proxy) = proxy.as_mapping_mut() {
            proxy.remove(Value::String("dialer-proxy".into()));
        }
    }
    for (index, name) in chain.nodes.iter().enumerate() {
        let proxy = proxies
            .iter_mut()
            .find(|proxy| proxy.get("name").and_then(Value::as_str) == Some(name.as_str()))
            .and_then(Value::as_mapping_mut)
            .ok_or_else(|| format!("Proxy chain node is not a concrete runtime proxy: {name}"))?;
        if index > 0 {
            proxy.insert(
                Value::String("dialer-proxy".into()),
                Value::String(chain.nodes[index - 1].clone()),
            );
        }
    }
    Ok(())
}

pub fn runtime_preview_config(yaml: &str) -> Result<String, String> {
    runtime_preview_config_with_overrides(yaml, &RuntimeOverrides::default())
}

pub fn runtime_preview_config_with_overrides(yaml: &str, overrides: &RuntimeOverrides) -> Result<String, String> {
    runtime_preview_config_with_state(yaml, overrides, None)
}

pub fn runtime_preview_config_with_state(
    yaml: &str,
    overrides: &RuntimeOverrides,
    chain: Option<&ProxyChain>,
) -> Result<String, String> {
    runtime_preview_config_with_enhancements(yaml, overrides, RuntimeEnhancements::default(), chain)
}

pub fn runtime_preview_config_with_enhancements(
    yaml: &str,
    overrides: &RuntimeOverrides,
    enhancements: RuntimeEnhancements<'_>,
    chain: Option<&ProxyChain>,
) -> Result<String, String> {
    inspect(yaml)?;
    let mut root: Value = serde_yaml_ng::from_str(yaml).map_err(|e| format!("YAML: {e}"))?;
    let map = root.as_mapping_mut().ok_or("Expected a Mihomo YAML mapping")?;
    apply_seq_yaml(map, enhancements.rules, "rules", "Rules")?;
    apply_seq_yaml(map, enhancements.proxies, "proxies", "Proxies")?;
    apply_seq_yaml(map, enhancements.groups, "proxy-groups", "Groups")?;
    apply_merge_yaml(map, enhancements.global_merge)?;
    apply_script_js(map, enhancements.global_script, enhancements.profile_name);
    apply_merge_yaml(map, enhancements.profile_merge)?;
    apply_script_js(map, enhancements.profile_script, enhancements.profile_name);
    remove_runtime_endpoints(map);
    map.insert(Value::String("allow-lan".into()), Value::Bool(false));
    apply_runtime_overrides(map, overrides)?;
    apply_proxy_chain(map, chain)?;
    for name in ["tun", "dns"] {
        if let Some(Value::Mapping(section)) = map.get_mut(Value::String(name.into())) {
            section.insert(Value::String("enable".into()), Value::Bool(false));
        }
    }
    serde_yaml_ng::to_string(&root).map_err(|e| e.to_string())
}

/// Render the profile for Android's non-root `VpnService` path.
///
/// Android owns the actual TUN interface and routes, so the config loaded into
/// the in-process mihomo bridge must never ask mihomo to create its own TUN.
/// DNS stays enabled because the VpnService TUN redirects its synthetic DNS
/// address into mihomo's resolver.
pub fn runtime_android_vpn_config_with_enhancements(
    yaml: &str,
    overrides: &RuntimeOverrides,
    enhancements: RuntimeEnhancements<'_>,
    chain: Option<&ProxyChain>,
) -> Result<String, String> {
    inspect(yaml)?;
    let mut root: Value = serde_yaml_ng::from_str(yaml).map_err(|e| format!("YAML: {e}"))?;
    let map = root.as_mapping_mut().ok_or("Expected a Mihomo YAML mapping")?;
    apply_seq_yaml(map, enhancements.rules, "rules", "Rules")?;
    apply_seq_yaml(map, enhancements.proxies, "proxies", "Proxies")?;
    apply_seq_yaml(map, enhancements.groups, "proxy-groups", "Groups")?;
    apply_merge_yaml(map, enhancements.global_merge)?;
    apply_script_js(map, enhancements.global_script, enhancements.profile_name);
    apply_merge_yaml(map, enhancements.profile_merge)?;
    apply_script_js(map, enhancements.profile_script, enhancements.profile_name);
    remove_runtime_endpoints(map);
    map.insert(Value::String("allow-lan".into()), Value::Bool(false));
    apply_runtime_overrides(map, overrides)?;
    apply_proxy_chain(map, chain)?;

    let mut tun = serde_yaml_ng::Mapping::new();
    tun.insert(Value::String("enable".into()), Value::Bool(false));
    map.insert(Value::String("tun".into()), Value::Mapping(tun));

    let dns_key = Value::String("dns".into());
    if !map.contains_key(&dns_key) {
        map.insert(dns_key.clone(), Value::Mapping(serde_yaml_ng::Mapping::new()));
    }
    let dns = map
        .get_mut(&dns_key)
        .and_then(Value::as_mapping_mut)
        .ok_or("dns must be a mapping")?;
    dns.insert(Value::String("enable".into()), Value::Bool(true));
    dns.insert(
        Value::String("ipv6".into()),
        Value::Bool(overrides.ipv6.unwrap_or(true)),
    );
    // The Android bridge owns the TUN-side DNS socket; a profile listen address
    // would create a second unrelated listener inside the app sandbox.
    dns.remove(Value::String("listen".into()));
    dns.entry(Value::String("enhanced-mode".into()))
        .or_insert_with(|| Value::String("fake-ip".into()));
    dns.entry(Value::String("fake-ip-range".into()))
        .or_insert_with(|| Value::String("198.18.0.1/16".into()));
    for key in ["default-nameserver", "nameserver", "proxy-server-nameserver"] {
        dns.entry(Value::String(key.into()))
            .or_insert_with(|| string_sequence(&["223.5.5.5", "119.29.29.29"]));
    }

    serde_yaml_ng::to_string(&root).map_err(|e| e.to_string())
}

pub fn runtime_config_tun_enabled(yaml: &str) -> bool {
    serde_yaml_ng::from_str::<Value>(yaml)
        .ok()
        .and_then(|root| root.get("tun").cloned())
        .and_then(|tun| tun.get("enable").and_then(Value::as_bool))
        .unwrap_or(false)
}

pub fn runtime_config_with_interface(yaml: &str, interface: &str) -> Result<String, String> {
    let interface = interface.trim();
    if interface.is_empty()
        || interface.len() > 15
        || interface
            .bytes()
            .any(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':')))
    {
        return Err("Invalid network interface name".into());
    }
    let mut root: Value = serde_yaml_ng::from_str(yaml).map_err(|e| format!("YAML: {e}"))?;
    let map = root.as_mapping_mut().ok_or("Expected a Mihomo YAML mapping")?;
    map.insert(
        Value::String("interface-name".into()),
        Value::String(interface.to_owned()),
    );
    serde_yaml_ng::to_string(&root).map_err(|e| e.to_string())
}

fn remove_runtime_endpoints(map: &mut serde_yaml_ng::Mapping) {
    for name in [
        "port",
        "socks-port",
        "redir-port",
        "tproxy-port",
        "mixed-port",
        "listeners",
        "bind-address",
        "external-controller",
        "external-controller-unix",
        "external-controller-pipe",
        "external-controller-cors",
        "external-ui",
        "external-ui-url",
        "secret",
    ] {
        map.remove(Value::String(name.into()));
    }
}

fn string_sequence(values: &[&str]) -> Value {
    Value::Sequence(values.iter().map(|value| Value::String((*value).into())).collect())
}

pub fn runtime_tun_config(yaml: &str) -> Result<String, String> {
    runtime_tun_config_with_overrides(yaml, &RuntimeOverrides::default())
}

pub fn runtime_tun_config_with_overrides(yaml: &str, overrides: &RuntimeOverrides) -> Result<String, String> {
    runtime_tun_config_with_state(yaml, overrides, None)
}

pub fn runtime_tun_config_with_state(
    yaml: &str,
    overrides: &RuntimeOverrides,
    chain: Option<&ProxyChain>,
) -> Result<String, String> {
    runtime_tun_config_with_enhancements(yaml, overrides, RuntimeEnhancements::default(), chain)
}

pub fn runtime_tun_config_with_enhancements(
    yaml: &str,
    overrides: &RuntimeOverrides,
    enhancements: RuntimeEnhancements<'_>,
    chain: Option<&ProxyChain>,
) -> Result<String, String> {
    inspect(yaml)?;
    let mut root: Value = serde_yaml_ng::from_str(yaml).map_err(|e| format!("YAML: {e}"))?;
    let map = root.as_mapping_mut().ok_or("Expected a Mihomo YAML mapping")?;
    apply_seq_yaml(map, enhancements.rules, "rules", "Rules")?;
    apply_seq_yaml(map, enhancements.proxies, "proxies", "Proxies")?;
    apply_seq_yaml(map, enhancements.groups, "proxy-groups", "Groups")?;
    apply_merge_yaml(map, enhancements.global_merge)?;
    apply_script_js(map, enhancements.global_script, enhancements.profile_name);
    apply_merge_yaml(map, enhancements.profile_merge)?;
    apply_script_js(map, enhancements.profile_script, enhancements.profile_name);
    remove_runtime_endpoints(map);
    map.insert(Value::String("allow-lan".into()), Value::Bool(false));
    apply_runtime_overrides(map, overrides)?;
    apply_proxy_chain(map, chain)?;
    // Keep Android's PROTECT_FROM_VPN bit (0x20000) and add a CV4A-private
    // high bit. The root agent bypasses the TUN only for the private bit, so
    // Android system traffic that independently uses 0x20000 still reaches
    // Mihomo while Mihomo's own outbound sockets continue through netd.
    map.insert(Value::String("routing-mark".into()), Value::Number(537_001_984.into()));

    let mut tun = serde_yaml_ng::Mapping::new();
    tun.insert(Value::String("enable".into()), Value::Bool(true));
    let stack = overrides.tun_stack.as_deref().unwrap_or("mixed").to_ascii_lowercase();
    if !matches!(stack.as_str(), "system" | "gvisor" | "mixed" | "mips") {
        return Err("Invalid Mihomo TUN stack".into());
    }
    tun.insert(Value::String("stack".into()), Value::String(stack));
    tun.insert(Value::String("device".into()), Value::String("Mihomo".into()));
    tun.insert(Value::String("auto-route".into()), Value::Bool(true));
    // Keep our route table/rule range separate from Box/sing-tun defaults
    // (2022 / 9000) so the Android fork can be tested and torn down without
    // mutating another Mihomo instance that may still be present on the phone.
    tun.insert(Value::String("iproute2-table-index".into()), Value::Number(3022.into()));
    tun.insert(Value::String("iproute2-rule-index".into()), Value::Number(8900.into()));
    tun.insert(Value::String("auto-redirect".into()), Value::Bool(false));
    // Mihomo's interface monitor can select an Android VPN's tun0 as the
    // default outbound interface. Once that happens the root core sends its
    // own proxy traffic back into the Android VPN and can loop or fail TLS.
    // routing-mark above lets Android choose the current physical network, so
    // interface auto-detection must stay disabled for the root TUN runtime.
    tun.insert(Value::String("auto-detect-interface".into()), Value::Bool(false));
    tun.insert(
        Value::String("strict-route".into()),
        Value::Bool(overrides.tun_strict_route.unwrap_or(true)),
    );
    if let Some(mtu) = overrides.tun_mtu {
        if !(576..=65_535).contains(&mtu) {
            return Err("TUN MTU must be between 576 and 65535".into());
        }
        tun.insert(Value::String("mtu".into()), Value::Number(mtu.into()));
    }
    let default_dns_hijack = ["any:53".to_owned(), "tcp://any:53".to_owned()];
    let dns_hijack = overrides.tun_dns_hijack.as_deref().unwrap_or(&default_dns_hijack);
    tun.insert(
        Value::String("dns-hijack".into()),
        Value::Sequence(dns_hijack.iter().map(|value| Value::String(value.clone())).collect()),
    );
    map.insert(Value::String("tun".into()), Value::Mapping(tun));

    let dns_key = Value::String("dns".into());
    if !map.contains_key(&dns_key) {
        map.insert(dns_key.clone(), Value::Mapping(serde_yaml_ng::Mapping::new()));
    }
    let dns = map
        .get_mut(&dns_key)
        .and_then(Value::as_mapping_mut)
        .ok_or("dns must be a mapping")?;
    dns.insert(Value::String("enable".into()), Value::Bool(true));
    dns.insert(Value::String("ipv6".into()), Value::Bool(true));
    dns.remove(Value::String("listen".into()));
    dns.entry(Value::String("enhanced-mode".into()))
        .or_insert_with(|| Value::String("fake-ip".into()));
    dns.entry(Value::String("fake-ip-range".into()))
        .or_insert_with(|| Value::String("198.18.0.1/16".into()));
    for key in ["default-nameserver", "nameserver", "proxy-server-nameserver"] {
        dns.entry(Value::String(key.into()))
            .or_insert_with(|| string_sequence(&["223.5.5.5", "119.29.29.29"]));
    }
    serde_yaml_ng::to_string(&root).map_err(|e| e.to_string())
}

#[derive(Debug)]
pub struct Store {
    path: PathBuf,
    document: Document,
}

impl Store {
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let document = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<Document>(&bytes)
                .map_err(|e| format!("Cannot read profiles; original file was not modified: {e}"))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Document::default(),
            Err(e) => return Err(e.to_string()),
        };
        if document
            .active_id
            .as_ref()
            .is_some_and(|id| !document.profiles.iter().any(|p| &p.id == id))
        {
            return Err("Active profile is missing; original file was not modified".into());
        }
        Ok(Self { path, document })
    }
    pub fn document(&self) -> Document {
        self.document.clone()
    }
    fn commit(&mut self, next: Document) -> Result<Document, String> {
        let parent = self.path.parent().ok_or("Invalid profile storage path")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let temporary = self.path.with_extension("json.tmp");
        let result = (|| {
            let mut options = fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temporary).map_err(|e| e.to_string())?;
            serde_json::to_writer(&mut file, &next).map_err(|e| e.to_string())?;
            file.write_all(b"\n").map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
            fs::rename(&temporary, &self.path).map_err(|e| e.to_string())
        })();
        if let Err(error) = result {
            let _ = fs::remove_file(temporary);
            return Err(error);
        }
        self.document = next;
        Ok(self.document())
    }
    pub fn import(&mut self, name: String, yaml: String, source: Option<String>) -> Result<Document, String> {
        self.import_with_options(name, yaml, source, ProfileOptions::default())
    }

    pub fn import_with_options(
        &mut self,
        name: String,
        yaml: String,
        source: Option<String>,
        option: ProfileOptions,
    ) -> Result<Document, String> {
        self.import_with_metadata(name, yaml, source, option, None, None, None)
    }

    pub fn import_with_metadata(
        &mut self,
        name: String,
        yaml: String,
        source: Option<String>,
        option: ProfileOptions,
        description: Option<String>,
        home: Option<String>,
        extra: Option<ProfileExtra>,
    ) -> Result<Document, String> {
        inspect(&yaml)?;
        let option = option.normalized()?;
        let name = name.trim().to_owned();
        if name.is_empty() || name.chars().count() > 100 {
            return Err("Profile name must have 1-100 characters".into());
        }
        let mut next = self.document();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?;
        let id = format!("{:x}-{}", now.as_nanos(), next.profiles.len());
        next.profiles.push(Profile {
            id: id.clone(),
            name,
            description: description
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty()),
            yaml,
            source,
            home,
            extra,
            updated_at: now.as_secs(),
            selected: Vec::new(),
            proxy_chain: None,
            option,
            merge_yaml: None,
            rules_yaml: None,
            proxies_yaml: None,
            groups_yaml: None,
            script_js: None,
            dns_override_enabled: None,
            dns_override_yaml: None,
        });
        if next.active_id.is_none() {
            next.active_id = Some(id);
        }
        self.commit(next)
    }
    pub fn activate(&mut self, id: &str) -> Result<Document, String> {
        let profile = self
            .document
            .profiles
            .iter()
            .find(|p| p.id == id)
            .ok_or("Profile not found")?;
        inspect(&profile.yaml)?;
        let mut next = self.document();
        next.active_id = Some(id.to_owned());
        self.commit(next)
    }
    pub fn remove(&mut self, id: &str) -> Result<Document, String> {
        if self.document.active_id.as_deref() == Some(id) {
            return Err("Switch to another profile before deleting the selected profile".into());
        }
        let mut next = self.document();
        let index = next
            .profiles
            .iter()
            .position(|p| p.id == id)
            .ok_or("Profile not found")?;
        next.profiles.remove(index);
        self.commit(next)
    }

    pub fn remove_many(&mut self, ids: &[String]) -> Result<Document, String> {
        if ids.is_empty() {
            return Ok(self.document());
        }
        let ids = ids.iter().collect::<HashSet<_>>();
        for id in &ids {
            if !self.document.profiles.iter().any(|profile| &profile.id == *id) {
                return Err(format!("Profile not found: {id}"));
            }
        }
        let mut next = self.document();
        next.profiles.retain(|profile| !ids.contains(&profile.id));
        if next.active_id.as_ref().is_some_and(|active| ids.contains(active)) {
            next.active_id = next.profiles.first().map(|profile| profile.id.clone());
        }
        self.commit(next)
    }

    pub fn restore(&mut self, document: Document) -> Result<Document, String> {
        for profile in &document.profiles {
            inspect(&profile.yaml)?;
            if let Some(merge) = profile.merge_yaml.as_deref() {
                validate_merge_yaml(merge)?;
            }
            for (kind, yaml) in [
                ("rules", profile.rules_yaml.as_deref()),
                ("proxies", profile.proxies_yaml.as_deref()),
                ("groups", profile.groups_yaml.as_deref()),
            ] {
                if let Some(yaml) = yaml {
                    validate_sequence_enhancement(yaml, kind)?;
                }
            }
            if let Some(script) = profile.script_js.as_deref() {
                validate_script_text(script)?;
            }
        }
        if let Some(merge) = document.global_merge_yaml.as_deref() {
            validate_merge_yaml(merge)?;
        }
        if let Some(script) = document.global_script_js.as_deref() {
            validate_script_text(script)?;
        }
        if document
            .active_id
            .as_ref()
            .is_some_and(|id| !document.profiles.iter().any(|profile| &profile.id == id))
        {
            return Err("Cannot restore profile document with a missing active profile".into());
        }
        self.commit(document)
    }

    pub fn set_global_merge(&mut self, yaml: Option<String>) -> Result<Document, String> {
        let yaml = yaml.and_then(|yaml| {
            let trimmed = yaml.trim();
            (!trimmed.is_empty()).then(|| yaml)
        });
        if let Some(yaml) = yaml.as_deref() {
            validate_merge_yaml(yaml)?;
        }
        let mut next = self.document();
        next.global_merge_yaml = yaml;
        self.commit(next)
    }

    pub fn set_profile_merge(&mut self, id: &str, yaml: Option<String>) -> Result<Document, String> {
        let yaml = yaml.and_then(|yaml| {
            let trimmed = yaml.trim();
            (!trimmed.is_empty()).then(|| yaml)
        });
        if let Some(yaml) = yaml.as_deref() {
            validate_merge_yaml(yaml)?;
        }
        let mut next = self.document();
        let profile = next
            .profiles
            .iter_mut()
            .find(|profile| profile.id == id)
            .ok_or("Profile not found")?;
        profile.merge_yaml = yaml;
        self.commit(next)
    }

    pub fn set_profile_sequence(&mut self, id: &str, kind: &str, yaml: Option<String>) -> Result<Document, String> {
        let yaml = yaml.and_then(|yaml| {
            let trimmed = yaml.trim();
            (!trimmed.is_empty()).then(|| yaml)
        });
        if let Some(yaml) = yaml.as_deref() {
            validate_sequence_enhancement(yaml, kind)?;
        }
        let mut next = self.document();
        let profile = next
            .profiles
            .iter_mut()
            .find(|profile| profile.id == id)
            .ok_or("Profile not found")?;
        match kind {
            "rules" => profile.rules_yaml = yaml,
            "proxies" => profile.proxies_yaml = yaml,
            "groups" => profile.groups_yaml = yaml,
            _ => return Err("Unknown sequence enhancement kind".into()),
        }
        self.commit(next)
    }

    pub fn set_global_script(&mut self, script: Option<String>) -> Result<Document, String> {
        let script = script.and_then(|script| {
            let trimmed = script.trim();
            (!trimmed.is_empty()).then(|| script)
        });
        if let Some(script) = script.as_deref() {
            validate_script_text(script)?;
        }
        let mut next = self.document();
        next.global_script_js = script;
        self.commit(next)
    }

    pub fn set_profile_script(&mut self, id: &str, script: Option<String>) -> Result<Document, String> {
        let script = script.and_then(|script| {
            let trimmed = script.trim();
            (!trimmed.is_empty()).then(|| script)
        });
        if let Some(script) = script.as_deref() {
            validate_script_text(script)?;
        }
        let mut next = self.document();
        let profile = next
            .profiles
            .iter_mut()
            .find(|profile| profile.id == id)
            .ok_or("Profile not found")?;
        profile.script_js = script;
        self.commit(next)
    }

    pub fn set_profile_dns_override(
        &mut self,
        id: &str,
        enabled: Option<bool>,
        yaml: Option<String>,
    ) -> Result<Document, String> {
        let mut next = self.document();
        let profile = next
            .profiles
            .iter_mut()
            .find(|profile| profile.id == id)
            .ok_or("Profile not found")?;
        profile.dns_override_enabled = enabled;
        profile.dns_override_yaml = yaml;
        self.commit(next)
    }

    pub fn replace(&mut self, id: &str, yaml: String, expected: &str) -> Result<Document, String> {
        self.replace_with_metadata(id, yaml, expected, None, None)
    }

    pub fn replace_with_metadata(
        &mut self,
        id: &str,
        yaml: String,
        expected: &str,
        home: Option<String>,
        extra: Option<ProfileExtra>,
    ) -> Result<Document, String> {
        inspect(&yaml)?;
        let mut next = self.document();
        let profile = next
            .profiles
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or("Profile not found")?;
        if profile.yaml != expected {
            return Err("Profile changed during the operation; reload and retry".into());
        }
        profile.yaml = yaml;
        profile.home = home;
        profile.extra = extra;
        profile.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs();
        self.commit(next)
    }

    pub fn update(&mut self, id: &str, name: String, yaml: String, expected: &str) -> Result<Document, String> {
        let profile = self
            .document
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .ok_or("Profile not found")?;
        self.update_with_metadata(
            id,
            name,
            yaml,
            profile.source.clone(),
            profile.option.clone(),
            profile.description.clone(),
            expected,
        )
    }

    pub fn update_with_options(
        &mut self,
        id: &str,
        name: String,
        yaml: String,
        source: Option<String>,
        option: ProfileOptions,
        expected: &str,
    ) -> Result<Document, String> {
        let description = self
            .document
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .and_then(|profile| profile.description.clone());
        self.update_with_metadata(id, name, yaml, source, option, description, expected)
    }

    pub fn update_with_metadata(
        &mut self,
        id: &str,
        name: String,
        yaml: String,
        source: Option<String>,
        option: ProfileOptions,
        description: Option<String>,
        expected: &str,
    ) -> Result<Document, String> {
        inspect(&yaml)?;
        let option = option.normalized()?;
        let name = name.trim().to_owned();
        if name.is_empty() || name.chars().count() > 100 {
            return Err("Profile name must have 1-100 characters".into());
        }
        let mut next = self.document();
        let profile = next
            .profiles
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or("Profile not found")?;
        if profile.yaml != expected {
            return Err("Profile changed during the operation; reload and retry".into());
        }
        profile.name = name;
        profile.description = description
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        profile.yaml = yaml;
        profile.source = source
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        profile.option = option;
        profile.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs();
        self.commit(next)
    }

    pub fn reorder(&mut self, active_id: &str, over_id: &str) -> Result<Document, String> {
        if active_id == over_id {
            return Ok(self.document());
        }
        let mut next = self.document();
        let active_index = next
            .profiles
            .iter()
            .position(|profile| profile.id == active_id)
            .ok_or("Profile not found")?;
        let over_index = next
            .profiles
            .iter()
            .position(|profile| profile.id == over_id)
            .ok_or("Profile not found")?;
        let profile = next.profiles.remove(active_index);
        next.profiles.insert(over_index, profile);
        self.commit(next)
    }

    pub fn record_selection(&mut self, id: &str, group: String, member: String) -> Result<Document, String> {
        self.record_selections(id, vec![SelectedProxy { group, member }])
    }

    pub fn record_selections(&mut self, id: &str, selections: Vec<SelectedProxy>) -> Result<Document, String> {
        if selections.is_empty() {
            return Ok(self.document());
        }
        let mut next = self.document();
        let profile = next
            .profiles
            .iter_mut()
            .find(|profile| profile.id == id)
            .ok_or("Profile not found")?;
        let mut changed = false;
        for selection in selections {
            let group = selection.group.trim().to_owned();
            let member = selection.member.trim().to_owned();
            if group.is_empty() || member.is_empty() {
                return Err("Proxy group and member are required".into());
            }
            if let Some(selected) = profile.selected.iter_mut().find(|selected| selected.group == group) {
                if selected.member != member {
                    selected.member = member;
                    changed = true;
                }
            } else {
                profile.selected.push(SelectedProxy { group, member });
                changed = true;
            }
        }
        if changed {
            self.commit(next)
        } else {
            Ok(self.document())
        }
    }

    pub fn forget_selection(&mut self, id: &str, group: &str) -> Result<Document, String> {
        let mut next = self.document();
        let profile = next
            .profiles
            .iter_mut()
            .find(|profile| profile.id == id)
            .ok_or("Profile not found")?;
        profile.selected.retain(|selected| selected.group != group);
        self.commit(next)
    }

    pub fn set_proxy_chain(&mut self, id: &str, target_group: String, nodes: Vec<String>) -> Result<Document, String> {
        let target_group = target_group.trim().to_owned();
        if target_group.is_empty() || nodes.len() < 2 {
            return Err("Proxy chain requires a target group and at least two nodes".into());
        }
        let mut seen = HashSet::new();
        if nodes
            .iter()
            .any(|node| node.trim().is_empty() || !seen.insert(node.clone()))
        {
            return Err("Proxy chain nodes must be non-empty and unique".into());
        }
        let mut next = self.document();
        let profile = next
            .profiles
            .iter_mut()
            .find(|profile| profile.id == id)
            .ok_or("Profile not found")?;
        let summary = inspect(&profile.yaml)?;
        let concrete = summary
            .nodes
            .iter()
            .map(|node| node.name.as_str())
            .collect::<HashSet<_>>();
        if let Some(node) = nodes.iter().find(|node| !concrete.contains(node.as_str())) {
            return Err(format!(
                "Proxy chain node is not a concrete proxy in this profile: {node}"
            ));
        }
        let exit = nodes.last().cloned().ok_or("Proxy chain has no exit node")?;
        profile.proxy_chain = Some(ProxyChain {
            target_group: target_group.clone(),
            nodes,
        });
        if let Some(selected) = profile
            .selected
            .iter_mut()
            .find(|selected| selected.group == target_group)
        {
            selected.member = exit;
        } else {
            profile.selected.push(SelectedProxy {
                group: target_group,
                member: exit,
            });
        }
        self.commit(next)
    }

    pub fn clear_proxy_chain(&mut self, id: &str) -> Result<Document, String> {
        let mut next = self.document();
        let profile = next
            .profiles
            .iter_mut()
            .find(|profile| profile.id == id)
            .ok_or("Profile not found")?;
        profile.proxy_chain = None;
        self.commit(next)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub profile_management: bool,
    pub root_probe: bool,
    pub tun_control: bool,
    pub system_proxy: bool,
    pub live_core: bool,
    pub stage: &'static str,
}

pub const fn capabilities() -> Capabilities {
    Capabilities {
        profile_management: true,
        root_probe: true,
        tun_control: true,
        system_proxy: true,
        live_core: true,
        stage: "android-root-tun+vpnservice",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const YAML: &str = "proxies:\n- {name: a, type: hysteria2, server: a.test, port: 443, password: p}\n- {name: b, type: vless, server: b.test, port: 443, uuid: id}\nproxy-groups:\n- {name: '1', type: select, proxies: [a, b]}\n- {name: '2', type: select, proxies: [b, a]}\nrules:\n- GEOSITE,category-dev,1\n- MATCH,2\nx-custom: untouched\n";
    #[test]
    fn profiles_preserve_all_nodes_groups_rules_and_unknown_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profiles.json");
        let mut store = Store::open(path.clone()).unwrap();
        store.import("Main".into(), YAML.into(), None).unwrap();
        let restored = Store::open(path).unwrap();
        assert_eq!(restored.document().profiles[0].yaml, YAML);
        let parsed = inspect(YAML).unwrap();
        assert_eq!(parsed.nodes.len(), 2);
        assert_eq!(parsed.groups[0].members, ["a", "b"]);
        assert_eq!(parsed.groups[1].members, ["b", "a"]);
        assert_eq!(parsed.rules.len(), 2);
    }
    #[test]
    fn failed_update_or_import_does_not_destroy_saved_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().join("profiles.json")).unwrap();
        let original = store.import("Main".into(), YAML.into(), None).unwrap();
        let id = &original.profiles[0].id;
        assert!(store.replace(id, "<html>bad gateway</html>".into(), YAML).is_err());
        assert!(store.import("Bad".into(), "rules: [".into(), None).is_err());
        assert_eq!(store.document().profiles[0].yaml, YAML);
        assert!(store.remove(id).is_err());
        assert!(store.replace(id, YAML.into(), "stale").is_err());
    }
    #[test]
    fn active_profile_changes_persist_without_affecting_other_profiles() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profiles.json");
        let mut store = Store::open(path.clone()).unwrap();
        store.import("A".into(), YAML.into(), None).unwrap();
        let doc = store.import("B".into(), YAML.into(), None).unwrap();
        store.activate(&doc.profiles[1].id).unwrap();
        store.remove(&doc.profiles[0].id).unwrap();
        let doc = Store::open(path).unwrap().document();
        assert_eq!(doc.profiles.len(), 1);
        assert_eq!(doc.active_id.as_deref(), Some(doc.profiles[0].id.as_str()));
    }

    #[test]
    fn profiles_can_be_edited_and_reordered_without_losing_source() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().join("profiles.json")).unwrap();
        let first = store
            .import("A".into(), YAML.into(), Some("https://example.com/a".into()))
            .unwrap();
        let first_id = first.profiles[0].id.clone();
        let second = store.import("B".into(), YAML.into(), None).unwrap();
        let second_id = second.profiles[1].id.clone();

        let edited = store.update(&first_id, "A edited".into(), YAML.into(), YAML).unwrap();
        assert_eq!(edited.profiles[0].name, "A edited");
        assert_eq!(edited.profiles[0].source.as_deref(), Some("https://example.com/a"));

        let reordered = store.reorder(&second_id, &first_id).unwrap();
        assert_eq!(reordered.profiles[0].id, second_id);
        assert_eq!(reordered.profiles[1].id, first_id);
    }

    #[test]
    fn selections_are_profile_scoped_and_persisted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profiles.json");
        let mut store = Store::open(path.clone()).unwrap();
        let first = store.import("A".into(), YAML.into(), None).unwrap();
        let first_id = first.profiles[0].id.clone();
        let second = store.import("B".into(), YAML.into(), None).unwrap();
        let second_id = second.profiles[1].id.clone();

        store.record_selection(&first_id, "1".into(), "b".into()).unwrap();
        store.record_selection(&second_id, "1".into(), "a".into()).unwrap();

        let reopened = Store::open(path).unwrap().document();
        assert_eq!(
            reopened.profiles[0].selected,
            [SelectedProxy {
                group: "1".into(),
                member: "b".into(),
            }]
        );
        assert_eq!(reopened.profiles[1].selected[0].member, "a");
    }

    #[test]
    fn proxy_chain_is_profile_scoped_and_rendered_as_dialer_proxy() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().join("profiles.json")).unwrap();
        let document = store.import("Main".into(), YAML.into(), None).unwrap();
        let id = document.profiles[0].id.clone();
        let document = store
            .set_proxy_chain(&id, "1".into(), vec!["a".into(), "b".into()])
            .unwrap();
        let chain = document.profiles[0].proxy_chain.as_ref().unwrap();
        assert_eq!(chain.target_group, "1");
        assert_eq!(document.profiles[0].selected[0].member, "b");

        let rendered = runtime_tun_config_with_state(YAML, &RuntimeOverrides::default(), Some(chain)).unwrap();
        let root: Value = serde_yaml_ng::from_str(&rendered).unwrap();
        let proxies = root.get("proxies").unwrap().as_sequence().unwrap();
        assert!(proxies[0].get("dialer-proxy").is_none());
        assert_eq!(proxies[1].get("dialer-proxy").and_then(Value::as_str), Some("a"));
        assert!(!YAML.contains("dialer-proxy"));
    }

    #[test]
    fn batch_delete_reselects_a_remaining_profile_or_clears_active() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().join("profiles.json")).unwrap();
        let first = store.import("A".into(), YAML.into(), None).unwrap();
        let first_id = first.profiles[0].id.clone();
        let second = store.import("B".into(), YAML.into(), None).unwrap();
        let second_id = second.profiles[1].id.clone();

        let remaining = store.remove_many(std::slice::from_ref(&first_id)).unwrap();
        assert_eq!(remaining.active_id.as_deref(), Some(second_id.as_str()));
        assert_eq!(remaining.profiles.len(), 1);

        let empty = store.remove_many(std::slice::from_ref(&second_id)).unwrap();
        assert!(empty.active_id.is_none());
        assert!(empty.profiles.is_empty());
    }
    #[test]
    fn android_build_exposes_only_verified_network_capabilities() {
        let c = capabilities();
        assert!(c.system_proxy);
        assert!(c.tun_control);
        assert!(c.live_core);
        assert!(c.profile_management);
    }
    #[test]
    fn android_vpn_runtime_keeps_dns_but_never_creates_its_own_tun() {
        let source = format!(
            "mixed-port: 7890\nallow-lan: true\ntun:\n  enable: true\n  auto-route: true\ndns:\n  enable: false\n  listen: 0.0.0.0:1053\n{YAML}"
        );
        let rendered = runtime_android_vpn_config_with_enhancements(
            &source,
            &RuntimeOverrides::default(),
            RuntimeEnhancements::default(),
            None,
        )
        .unwrap();
        let root: Value = serde_yaml_ng::from_str(&rendered).unwrap();
        assert_eq!(root["tun"]["enable"].as_bool(), Some(false));
        assert_eq!(root["dns"]["enable"].as_bool(), Some(true));
        assert!(root["dns"].get("listen").is_none());
        assert!(root.get("mixed-port").is_none());
        assert_eq!(root["allow-lan"].as_bool(), Some(false));
    }
    #[test]
    fn preview_runtime_cannot_take_over_networking() {
        let source = format!(
            "mixed-port: 7890\nredir-port: 7892\nallow-lan: true\ntun:\n  enable: true\n  auto-route: true\ndns:\n  enable: true\n  listen: 0.0.0.0:1053\nexternal-controller: 0.0.0.0:9090\n{YAML}"
        );
        let rendered = runtime_preview_config(&source).unwrap();
        let root: Value = serde_yaml_ng::from_str(&rendered).unwrap();
        assert!(root.get("mixed-port").is_none());
        assert!(root.get("redir-port").is_none());
        assert!(root.get("external-controller").is_none());
        assert_eq!(root.get("allow-lan").and_then(Value::as_bool), Some(false));
        assert_eq!(
            root.get("tun").and_then(|v| v.get("enable")).and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            root.get("dns").and_then(|v| v.get("enable")).and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(root.get("x-custom").and_then(Value::as_str), Some("untouched"));
    }

    #[test]
    fn tun_runtime_owns_network_entrypoints_and_keeps_proxy_rules() {
        let source = format!(
            "mixed-port: 7890\nallow-lan: true\ntun:\n  enable: false\n  stack: system\ndns:\n  enable: false\n  listen: 0.0.0.0:1053\nexternal-controller: 0.0.0.0:9090\n{YAML}"
        );
        let rendered = runtime_tun_config(&source).unwrap();
        let root: Value = serde_yaml_ng::from_str(&rendered).unwrap();
        assert!(root.get("mixed-port").is_none());
        assert!(root.get("external-controller").is_none());
        assert_eq!(root.get("allow-lan").and_then(Value::as_bool), Some(false));
        let tun = root.get("tun").unwrap();
        assert_eq!(tun.get("enable").and_then(Value::as_bool), Some(true));
        assert_eq!(tun.get("stack").and_then(Value::as_str), Some("mixed"));
        assert_eq!(tun.get("device").and_then(Value::as_str), Some("Mihomo"));
        assert_eq!(tun.get("auto-route").and_then(Value::as_bool), Some(true));
        assert_eq!(tun.get("iproute2-table-index").and_then(Value::as_i64), Some(3022));
        assert_eq!(tun.get("iproute2-rule-index").and_then(Value::as_i64), Some(8900));
        assert_eq!(tun.get("auto-redirect").and_then(Value::as_bool), Some(false));
        assert_eq!(tun.get("auto-detect-interface").and_then(Value::as_bool), Some(false));
        assert_eq!(tun.get("strict-route").and_then(Value::as_bool), Some(true));
        assert_eq!(root.get("routing-mark").and_then(Value::as_i64), Some(537_001_984));
        let dns = root.get("dns").unwrap();
        assert_eq!(dns.get("enable").and_then(Value::as_bool), Some(true));
        assert!(dns.get("listen").is_none());
        assert_eq!(dns.get("enhanced-mode").and_then(Value::as_str), Some("fake-ip"));
        assert_eq!(root.get("x-custom").and_then(Value::as_str), Some("untouched"));
        assert_eq!(root.get("rules").and_then(Value::as_sequence).unwrap().len(), 2);
    }

    #[test]
    fn runtime_overrides_survive_generated_tun_config() {
        let rendered = runtime_tun_config_with_overrides(
            YAML,
            &RuntimeOverrides {
                ipv6: Some(false),
                unified_delay: Some(true),
                log_level: Some("warning".into()),
                mode: Some("global".into()),
                tcp_concurrent: Some(true),
                find_process_mode: Some("always".into()),
                tun_stack: Some("system".into()),
                tun_strict_route: Some(false),
                tun_auto_detect_interface: Some(false),
                tun_dns_hijack: Some(vec!["any:53".into()]),
                tun_mtu: Some(1400),
                ..RuntimeOverrides::default()
            },
        )
        .unwrap();
        let root: Value = serde_yaml_ng::from_str(&rendered).unwrap();
        assert_eq!(root.get("ipv6").and_then(Value::as_bool), Some(false));
        assert_eq!(root.get("unified-delay").and_then(Value::as_bool), Some(true));
        assert_eq!(root.get("log-level").and_then(Value::as_str), Some("warning"));
        assert_eq!(root.get("mode").and_then(Value::as_str), Some("global"));
        assert_eq!(root.get("tcp-concurrent").and_then(Value::as_bool), Some(true));
        assert_eq!(root.get("find-process-mode").and_then(Value::as_str), Some("always"));
        let tun = root.get("tun").unwrap();
        assert_eq!(tun.get("stack").and_then(Value::as_str), Some("system"));
        assert_eq!(tun.get("strict-route").and_then(Value::as_bool), Some(false));
        assert_eq!(tun.get("auto-detect-interface").and_then(Value::as_bool), Some(false));
        assert_eq!(tun.get("mtu").and_then(Value::as_u64), Some(1400));
        assert_eq!(
            tun.get("dns-hijack").and_then(Value::as_sequence).map(Vec::len),
            Some(1)
        );
    }

    #[test]
    fn runtime_interface_binding_is_explicit_and_validated() {
        let rendered = runtime_config_with_interface(YAML, "wlan0").unwrap();
        let root: Value = serde_yaml_ng::from_str(&rendered).unwrap();
        assert_eq!(root.get("interface-name").and_then(Value::as_str), Some("wlan0"));
        assert!(runtime_config_with_interface(YAML, "tun 0").is_err());
        assert!(runtime_config_with_interface(YAML, "").is_err());
    }

    #[test]
    fn dns_override_replaces_dns_and_hosts_without_mutating_source() {
        let override_yaml = r#"
dns:
  enable: true
  enhanced-mode: redir-host
  nameserver:
    - 1.1.1.1
hosts:
  example.test: 192.0.2.1
"#;
        let rendered = runtime_tun_config_with_overrides(
            YAML,
            &RuntimeOverrides {
                dns_override_enabled: Some(true),
                dns_override_yaml: Some(override_yaml.into()),
                ..RuntimeOverrides::default()
            },
        )
        .unwrap();
        let root: Value = serde_yaml_ng::from_str(&rendered).unwrap();
        let dns = root.get("dns").unwrap();
        assert_eq!(dns.get("enhanced-mode").and_then(Value::as_str), Some("redir-host"));
        assert_eq!(
            dns.get("nameserver")
                .and_then(Value::as_sequence)
                .and_then(|items| items.first())
                .and_then(Value::as_str),
            Some("1.1.1.1")
        );
        assert!(dns.get("listen").is_none());
        assert_eq!(
            root.get("hosts")
                .and_then(|hosts| hosts.get("example.test"))
                .and_then(Value::as_str),
            Some("192.0.2.1")
        );
        assert!(!YAML.contains("example.test"));
    }

    #[test]
    fn merge_matches_verge_precedence_and_keeps_app_owned_runtime_fields() {
        let global = r#"
X-Custom:
  global: true
  winner: global
dns:
  nameserver-policy:
    global.example: 1.1.1.1
hosts:
  global.example: 192.0.2.10
rules:
  - DOMAIN,global.example,DIRECT
mixed-port: 45678
allow-lan: true
"#;
        let profile = r#"
x-custom:
  winner: profile
  profile: true
dns:
  fallback-filter:
    geoip: false
hosts:
  profile.example: 192.0.2.20
rules:
  - DOMAIN,profile.example,DIRECT
mixed-port: 45679
allow-lan: true
"#;
        let rendered = runtime_tun_config_with_enhancements(
            YAML,
            &RuntimeOverrides::default(),
            RuntimeEnhancements {
                global_merge: Some(global),
                profile_merge: Some(profile),
                ..RuntimeEnhancements::default()
            },
            None,
        )
        .unwrap();
        let root: Value = serde_yaml_ng::from_str(&rendered).unwrap();
        assert_eq!(root["x-custom"]["global"].as_bool(), Some(true));
        assert_eq!(root["x-custom"]["profile"].as_bool(), Some(true));
        assert_eq!(root["x-custom"]["winner"].as_str(), Some("profile"));
        assert_eq!(
            root["dns"]["nameserver-policy"]["global.example"].as_str(),
            Some("1.1.1.1")
        );
        assert_eq!(root["dns"]["fallback-filter"]["geoip"].as_bool(), Some(false));
        assert!(root["hosts"].get("global.example").is_none());
        assert_eq!(root["hosts"]["profile.example"].as_str(), Some("192.0.2.20"));
        assert_eq!(
            root["rules"]
                .as_sequence()
                .and_then(|rules| rules.first())
                .and_then(Value::as_str),
            Some("DOMAIN,profile.example,DIRECT")
        );
        assert!(root.get("mixed-port").is_none());
        assert_eq!(root.get("allow-lan").and_then(Value::as_bool), Some(false));
    }

    #[test]
    fn sequence_enhancements_match_verge_order_and_proxy_group_repair() {
        let source = r#"
proxies:
  - name: old-a
    type: direct
  - name: old-b
    type: direct
proxy-groups:
  - name: selector-1
    type: select
    proxies: [old-a, old-b]
  - name: selector-2
    type: select
    proxies: [old-a]
rules:
  - DOMAIN,old.example,selector-1
  - MATCH,DIRECT
"#;
        let rules = r#"
prepend:
  - DOMAIN,first.example,DIRECT
append:
  - DOMAIN,last.example,DIRECT
delete:
  - DOMAIN,old.example,selector-1
"#;
        let proxies = r#"
prepend:
  - name: new-a
    type: direct
append:
  - name: new-b
    type: direct
delete:
  - old-a
"#;
        let groups = r#"
prepend:
  - name: top-group
    type: select
    proxies: [DIRECT]
append: []
delete:
  - selector-2
"#;
        let rendered = runtime_tun_config_with_enhancements(
            source,
            &RuntimeOverrides::default(),
            RuntimeEnhancements {
                rules: Some(rules),
                proxies: Some(proxies),
                groups: Some(groups),
                ..RuntimeEnhancements::default()
            },
            None,
        )
        .unwrap();
        let root: Value = serde_yaml_ng::from_str(&rendered).unwrap();
        let proxy_names = root["proxies"]
            .as_sequence()
            .unwrap()
            .iter()
            .filter_map(|proxy| proxy.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert_eq!(proxy_names, ["new-a", "old-b", "new-b"]);

        let groups = root["proxy-groups"].as_sequence().unwrap();
        assert_eq!(groups[0]["name"].as_str(), Some("top-group"));
        assert_eq!(groups[1]["name"].as_str(), Some("selector-1"));
        assert_eq!(groups.len(), 2);
        let selector_members = groups[1]["proxies"]
            .as_sequence()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();
        assert_eq!(selector_members, ["new-a", "new-b", "old-b"]);

        let rules = root["rules"]
            .as_sequence()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();
        assert_eq!(
            rules,
            [
                "DOMAIN,first.example,DIRECT",
                "MATCH,DIRECT",
                "DOMAIN,last.example,DIRECT"
            ]
        );
    }

    #[test]
    fn scripts_follow_verge_order_and_cannot_override_app_owned_fields() {
        let rendered = runtime_tun_config_with_enhancements(
            YAML,
            &RuntimeOverrides::default(),
            RuntimeEnhancements {
                global_merge: Some("x-stage: global-merge\n"),
                global_script: Some(
                    "function main(config) { config['x-stage'] = 'global-script'; return config; }",
                ),
                profile_merge: Some("x-stage: profile-merge\n"),
                profile_script: Some(
                    "function main(config, profileName) { config['x-stage'] = 'profile-script'; config['x-profile-name'] = profileName; config['allow-lan'] = true; config['mixed-port'] = 46125; return config; }",
                ),
                profile_name: "Main",
                ..RuntimeEnhancements::default()
            },
            None,
        )
        .unwrap();
        let root: Value = serde_yaml_ng::from_str(&rendered).unwrap();
        assert_eq!(root["x-stage"].as_str(), Some("profile-script"));
        assert_eq!(root["x-profile-name"].as_str(), Some("Main"));
        assert_eq!(root.get("allow-lan").and_then(Value::as_bool), Some(false));
        assert!(root.get("mixed-port").is_none());
    }
}
