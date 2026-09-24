use percent_encoding::percent_decode_str;
use serde_yaml_ng::{Mapping, Value};
use std::collections::{HashMap, HashSet};
use url::Url;

fn decode(s: &str) -> Result<String, String> {
    percent_decode_str(s)
        .decode_utf8()
        .map(|s| s.into_owned())
        .map_err(|_| "Invalid UTF-8 in node link".into())
}
fn field(map: &mut Mapping, key: &str, value: impl Into<Value>) {
    map.insert(Value::String(key.into()), value.into());
}
fn text(map: &mut Mapping, key: &str, value: &str) {
    field(map, key, Value::String(value.into()));
}

pub fn parse_links(input: &str) -> Result<String, String> {
    if input.len() > super::MAX_PROFILE_BYTES {
        return Err("Node links exceed 4 MiB".into());
    }
    let mut nodes = Vec::new();
    let mut names = Vec::new();
    let mut unique = HashSet::new();
    for (index, line) in input.lines().map(str::trim).filter(|s| !s.is_empty()).enumerate() {
        let url = Url::parse(line).map_err(|_| format!("Invalid node URI on line {}", index + 1))?;
        let scheme = url.scheme();
        if !matches!(scheme, "vless" | "hy2" | "hysteria2" | "anytls") {
            return Err(format!(
                "Unsupported node protocol: {scheme}; use a Mihomo YAML for other protocols"
            ));
        }
        let server = url.host_str().ok_or("Missing node server")?.trim_matches(['[', ']']);
        let port = url.port().unwrap_or(443);
        if port == 0 {
            return Err("Port must be between 1 and 65535".into());
        }
        let auth = if let Some(pass) = url.password() {
            format!("{}:{}", decode(url.username())?, decode(pass)?)
        } else {
            decode(url.username())?
        };
        if auth.is_empty() {
            return Err("Node credentials are missing".into());
        }
        let name = match url.fragment() {
            Some(s) if !s.is_empty() => decode(s)?,
            _ => format!("{scheme}-{}", index + 1),
        };
        if matches!(name.as_str(), "PROXY" | "DIRECT" | "REJECT" | "GLOBAL") {
            return Err(format!("Node name {name} conflicts with a built-in policy"));
        }
        if !unique.insert(name.clone()) {
            return Err(format!("Duplicate node name: {name}"));
        }
        let mut query = HashMap::new();
        for (k, v) in url.query_pairs() {
            if query.insert(k.into_owned(), v.into_owned()).is_some() {
                return Err("Duplicate URI query option".into());
            }
        }
        let allowed: &[&str] = if scheme == "vless" {
            &[
                "type",
                "security",
                "sni",
                "servername",
                "fp",
                "pbk",
                "sid",
                "flow",
                "alpn",
                "encryption",
                "host",
                "path",
                "serviceName",
                "allowInsecure",
                "insecure",
                "packetEncoding",
            ]
        } else {
            &[
                "sni",
                "peer",
                "insecure",
                "allowInsecure",
                "alpn",
                "obfs",
                "obfs-password",
                "up",
                "down",
            ]
        };
        for key in query.keys() {
            if !allowed.contains(&key.as_str()) {
                return Err(format!(
                    "Unsupported URI option {key}; import a Mihomo YAML to preserve it"
                ));
            }
        }
        let value = |key: &str| query.get(key).map(String::as_str);
        let mut node = Mapping::new();
        text(&mut node, "name", &name);
        text(&mut node, "type", if scheme == "hy2" { "hysteria2" } else { scheme });
        text(&mut node, "server", server);
        field(&mut node, "port", Value::from(port));
        let sni = value("sni").or_else(|| value("servername")).or_else(|| value("peer"));
        if let Some(sni) = sni {
            text(&mut node, if scheme == "vless" { "servername" } else { "sni" }, sni);
        }
        if let Some(insecure) = value("insecure").or_else(|| value("allowInsecure")) {
            match insecure {
                "0" | "false" => field(&mut node, "skip-cert-verify", Value::Bool(false)),
                "1" | "true" => field(&mut node, "skip-cert-verify", Value::Bool(true)),
                _ => return Err("Invalid insecure flag".into()),
            }
        }
        if let Some(alpn) = value("alpn") {
            field(
                &mut node,
                "alpn",
                Value::Sequence(alpn.split(',').map(|s| Value::String(s.into())).collect()),
            );
        }
        if scheme == "vless" {
            if url.password().is_some() {
                return Err("VLESS expects a UUID, not username:password".into());
            }
            text(&mut node, "uuid", &auth);
            if let Some(encryption) = value("encryption")
                && encryption != "none"
            {
                return Err("Only VLESS encryption=none is supported by URI import".into());
            }
            if let Some(flow) = value("flow") {
                text(&mut node, "flow", flow);
            }
            if let Some(fp) = value("fp") {
                text(&mut node, "client-fingerprint", fp);
            }
            if let Some(packet) = value("packetEncoding") {
                text(&mut node, "packet-encoding", packet);
            }
            match value("security").unwrap_or("none") {
                "none" => field(&mut node, "tls", Value::Bool(false)),
                "tls" => field(&mut node, "tls", Value::Bool(true)),
                "reality" => {
                    field(&mut node, "tls", Value::Bool(true));
                    let mut reality = Mapping::new();
                    text(
                        &mut reality,
                        "public-key",
                        value("pbk").filter(|s| !s.is_empty()).ok_or("Reality requires pbk")?,
                    );
                    if let Some(sid) = value("sid") {
                        text(&mut reality, "short-id", sid);
                    }
                    field(&mut node, "reality-opts", Value::Mapping(reality));
                }
                other => return Err(format!("Unsupported VLESS security: {other}")),
            }
            match value("type").unwrap_or("tcp") {
                "tcp" => {}
                "ws" => {
                    text(&mut node, "network", "ws");
                    let mut ws = Mapping::new();
                    if let Some(path) = value("path") {
                        text(&mut ws, "path", path);
                    }
                    if let Some(host) = value("host") {
                        let mut headers = Mapping::new();
                        text(&mut headers, "Host", host);
                        field(&mut ws, "headers", Value::Mapping(headers));
                    }
                    field(&mut node, "ws-opts", Value::Mapping(ws));
                }
                "grpc" => {
                    text(&mut node, "network", "grpc");
                    let mut grpc = Mapping::new();
                    text(&mut grpc, "grpc-service-name", value("serviceName").unwrap_or(""));
                    field(&mut node, "grpc-opts", Value::Mapping(grpc));
                }
                other => return Err(format!("Unsupported transport {other}; import YAML instead")),
            }
        } else {
            text(&mut node, "password", &auth);
            for key in ["obfs", "obfs-password", "up", "down"] {
                if let Some(v) = value(key) {
                    if scheme == "anytls" {
                        return Err(format!("Unsupported AnyTLS option: {key}"));
                    }
                    text(&mut node, key, v);
                }
            }
        }
        names.push(Value::String(name));
        nodes.push(Value::Mapping(node));
    }
    if nodes.is_empty() {
        return Err("Paste at least one VLESS, Hysteria2 or AnyTLS link".into());
    }
    let mut group = Mapping::new();
    text(&mut group, "name", "PROXY");
    text(&mut group, "type", "select");
    field(&mut group, "proxies", Value::Sequence(names));
    let mut root = Mapping::new();
    field(&mut root, "proxies", Value::Sequence(nodes));
    field(&mut root, "proxy-groups", Value::Sequence(vec![Value::Mapping(group)]));
    field(
        &mut root,
        "rules",
        Value::Sequence(vec![Value::String("MATCH,PROXY".into())]),
    );
    serde_yaml_ng::to_string(&root).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imports_both_protocols_without_pruning() {
        let y=parse_links("vless://123@a.test:443?security=reality&pbk=key&sni=a.test#Alpha\nhysteria2://p%40ss@b.test:8443?sni=b.test&obfs=salamander&obfs-password=x#Beta").unwrap();
        let s = super::super::inspect(&y).unwrap();
        assert_eq!(s.nodes.len(), 2);
        assert_eq!(s.groups[0].members, ["Alpha", "Beta"]);
        assert_eq!(s.rules, ["MATCH,PROXY"]);
    }
    #[test]
    fn unsupported_options_are_not_silently_dropped() {
        assert!(parse_links("hy2://p@a.test:443?made-up=1").is_err());
        assert!(parse_links("vless://p@a.test:443?security=reality").is_err());
        assert!(parse_links("hy2://p@a.test:443#Same\nhy2://q@b.test:443#Same").is_err());
        assert!(parse_links("hy2://p@a.test:443#PROXY").is_err());
    }
    #[test]
    fn vless_sni_uses_the_mihomo_servername_field() {
        let yaml = parse_links("vless://id@127.0.0.1:443?security=tls&sni=example.test#TLS").unwrap();
        let config: Value = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(config["proxies"][0]["servername"].as_str(), Some("example.test"));
        assert!(config["proxies"][0].get("sni").is_none());
    }
}
