use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path, time::Duration};

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub version: String,
    pub mode: String,
    pub groups: Vec<ProxyGroup>,
    pub rules: Vec<Rule>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyGroup {
    pub name: String,
    pub kind: String,
    pub selected: Option<String>,
    pub members: Vec<String>,
    pub selectable: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub kind: String,
    pub payload: String,
    pub proxy: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connections {
    pub download_total: u64,
    pub upload_total: u64,
    pub memory: u64,
    pub connections: Vec<Connection>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub id: String,
    pub network: String,
    pub inbound: String,
    pub source: String,
    pub destination: String,
    pub host: String,
    pub process: String,
    pub rule: String,
    pub rule_payload: String,
    pub chains: Vec<String>,
    pub upload: u64,
    pub download: u64,
    pub start: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyProviderSummary {
    pub name: String,
    pub vehicle_type: String,
    pub updated_at: Option<String>,
    pub subscription_info: Option<SubscriptionInfo>,
    pub proxy_count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionInfo {
    pub upload: u64,
    pub download: u64,
    pub total: u64,
    pub expire: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleProviderSummary {
    pub name: String,
    pub vehicle_type: String,
    pub behavior: String,
    pub rule_count: u64,
    pub updated_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePreferences {
    pub ipv6: bool,
    pub unified_delay: bool,
    pub log_level: String,
    pub tcp_concurrent: bool,
    pub find_process_mode: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunPreferences {
    pub stack: String,
    pub strict_route: bool,
    pub auto_detect_interface: bool,
    pub dns_hijack: Vec<String>,
    pub mtu: u32,
}

#[derive(Clone, Debug)]
pub struct MixedPortSettings {
    pub mixed_port: u16,
    pub allow_lan: bool,
    pub bind_address: String,
}

#[derive(Deserialize)]
struct VersionResponse {
    version: String,
}

impl Client {
    pub fn new(socket: impl AsRef<Path>) -> Result<Self, String> {
        let http = reqwest::Client::builder()
            .unix_socket(socket.as_ref())
            .timeout(Duration::from_secs(5))
            .no_proxy()
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { http })
    }

    async fn json<T: DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        self.http
            .get(format!("http://localhost{path}"))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json::<T>()
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn version(&self) -> Result<String, String> {
        Ok(self.json::<VersionResponse>("/version").await?.version)
    }

    pub async fn snapshot(&self) -> Result<Snapshot, String> {
        let version = self.version().await?;
        let config = self.json::<Value>("/configs").await?;
        let proxies = self.json::<Value>("/proxies").await?;
        let rules = self.json::<Value>("/rules").await?;
        Ok(Snapshot {
            version,
            mode: config.get("mode").and_then(Value::as_str).unwrap_or("rule").to_owned(),
            groups: parse_groups(&proxies)?,
            rules: parse_rules(&rules)?,
        })
    }

    pub async fn select(&self, group: &str, member: &str) -> Result<(), String> {
        if group.is_empty() || member.is_empty() {
            return Err("Proxy group and member are required".into());
        }
        let group = utf8_percent_encode(group, NON_ALPHANUMERIC);
        self.http
            .put(format!("http://localhost/proxies/{group}"))
            .json(&json!({ "name": member }))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn set_mode(&self, mode: &str) -> Result<(), String> {
        let mode = mode.to_ascii_lowercase();
        if !matches!(mode.as_str(), "rule" | "global" | "direct") {
            return Err("Mode must be rule, global or direct".into());
        }
        self.http
            .patch("http://localhost/configs")
            .json(&json!({ "mode": mode }))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn connections(&self) -> Result<Connections, String> {
        parse_connections(&self.json::<Value>("/connections/").await?)
    }

    pub async fn close_connection(&self, id: &str) -> Result<(), String> {
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
            return Err("Invalid Mihomo connection id".into());
        }
        self.http
            .delete(format!("http://localhost/connections/{id}"))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn close_all_connections(&self) -> Result<(), String> {
        self.http
            .delete("http://localhost/connections/")
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn delay(&self, proxy: &str, test_url: &str, timeout_ms: u64) -> Result<u64, String> {
        if proxy.is_empty() {
            return Err("Proxy name is required".into());
        }
        if !(500..=30_000).contains(&timeout_ms) {
            return Err("Delay timeout must be between 500 and 30000 ms".into());
        }
        let proxy = utf8_percent_encode(proxy, NON_ALPHANUMERIC);
        let test_url = utf8_percent_encode(test_url, NON_ALPHANUMERIC);
        let value = self
            .http
            .get(format!(
                "http://localhost/proxies/{proxy}/delay?url={test_url}&timeout={timeout_ms}"
            ))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json::<Value>()
            .await
            .map_err(|e| e.to_string())?;
        value
            .get("delay")
            .and_then(Value::as_u64)
            .ok_or_else(|| "Mihomo delay response is missing delay".into())
    }

    pub async fn proxy_providers(&self) -> Result<Vec<ProxyProviderSummary>, String> {
        parse_proxy_providers(&self.json::<Value>("/providers/proxies").await?)
    }

    pub async fn update_proxy_provider(&self, name: &str) -> Result<(), String> {
        self.update_provider("proxies", name).await
    }

    pub async fn rule_providers(&self) -> Result<Vec<RuleProviderSummary>, String> {
        parse_rule_providers(&self.json::<Value>("/providers/rules").await?)
    }

    pub async fn update_rule_provider(&self, name: &str) -> Result<(), String> {
        self.update_provider("rules", name).await
    }

    pub async fn runtime_preferences(&self) -> Result<RuntimePreferences, String> {
        let config = self.json::<Value>("/configs").await?;
        Ok(RuntimePreferences {
            ipv6: config.get("ipv6").and_then(Value::as_bool).unwrap_or(false),
            unified_delay: config.get("unified-delay").and_then(Value::as_bool).unwrap_or(false),
            log_level: config
                .get("log-level")
                .and_then(Value::as_str)
                .unwrap_or("info")
                .to_owned(),
            tcp_concurrent: config.get("tcp-concurrent").and_then(Value::as_bool).unwrap_or(false),
            find_process_mode: config
                .get("find-process-mode")
                .and_then(Value::as_str)
                .unwrap_or("strict")
                .to_owned(),
        })
    }

    pub async fn mixed_port_settings(&self) -> Result<MixedPortSettings, String> {
        let config = self.json::<Value>("/configs").await?;
        Ok(MixedPortSettings {
            mixed_port: value_u64(config.get("mixed-port")).try_into().unwrap_or(0),
            allow_lan: config.get("allow-lan").and_then(Value::as_bool).unwrap_or(false),
            bind_address: config
                .get("bind-address")
                .and_then(Value::as_str)
                .unwrap_or("*")
                .to_owned(),
        })
    }

    pub async fn patch_mixed_port_settings(&self, settings: &MixedPortSettings) -> Result<(), String> {
        self.http
            .patch("http://localhost/configs")
            .json(&json!({
                "mixed-port": settings.mixed_port,
                "allow-lan": settings.allow_lan,
                "bind-address": settings.bind_address,
            }))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn patch_runtime_preferences(&self, preferences: &RuntimePreferences) -> Result<(), String> {
        self.http
            .patch("http://localhost/configs")
            .json(&json!({
                "ipv6": preferences.ipv6,
                "unified-delay": preferences.unified_delay,
                "log-level": preferences.log_level,
                "tcp-concurrent": preferences.tcp_concurrent,
                "find-process-mode": preferences.find_process_mode,
            }))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn tun_preferences(&self) -> Result<TunPreferences, String> {
        let config = self.json::<Value>("/configs").await?;
        let tun = config
            .get("tun")
            .and_then(Value::as_object)
            .ok_or("Mihomo config is missing tun settings")?;
        Ok(TunPreferences {
            stack: tun
                .get("stack")
                .and_then(Value::as_str)
                .unwrap_or("mixed")
                .to_ascii_lowercase(),
            strict_route: tun.get("strict-route").and_then(Value::as_bool).unwrap_or(true),
            auto_detect_interface: tun
                .get("auto-detect-interface")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            dns_hijack: tun
                .get("dns-hijack")
                .and_then(Value::as_array)
                .map(|items| items.iter().filter_map(Value::as_str).map(str::to_owned).collect())
                .unwrap_or_else(|| vec!["any:53".into(), "tcp://any:53".into()]),
            mtu: value_u64(tun.get("mtu")).try_into().unwrap_or(1500),
        })
    }

    pub async fn flush_fakeip(&self) -> Result<(), String> {
        self.post_empty("/cache/fakeip/flush", Duration::from_secs(5)).await
    }

    pub async fn flush_dns(&self) -> Result<(), String> {
        self.post_empty("/cache/dns/flush", Duration::from_secs(5)).await
    }

    pub async fn update_geo(&self) -> Result<(), String> {
        self.post_empty("/configs/geo", Duration::from_secs(60)).await
    }

    async fn post_empty(&self, path: &str, timeout: Duration) -> Result<(), String> {
        self.http
            .post(format!("http://localhost{path}"))
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn update_provider(&self, kind: &str, name: &str) -> Result<(), String> {
        if name.trim().is_empty() {
            return Err("Provider name is required".into());
        }
        let name = utf8_percent_encode(name, NON_ALPHANUMERIC);
        self.http
            .put(format!("http://localhost/providers/{kind}/{name}"))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn value_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Number(value)) => value.to_string(),
        _ => String::new(),
    }
}

fn value_u64(value: Option<&Value>) -> u64 {
    match value {
        Some(Value::Number(value)) => value.as_u64().unwrap_or(0),
        Some(Value::String(value)) => value.parse().unwrap_or(0),
        _ => 0,
    }
}

fn subscription_info(value: Option<&Value>) -> Option<SubscriptionInfo> {
    let value = value?.as_object()?;
    let get = |lower: &str, upper: &str| value.get(lower).or_else(|| value.get(upper));
    Some(SubscriptionInfo {
        upload: value_u64(get("upload", "Upload")),
        download: value_u64(get("download", "Download")),
        total: value_u64(get("total", "Total")),
        expire: value_u64(get("expire", "Expire")),
    })
}

fn real_provider_vehicle(value: Option<&Value>) -> Option<String> {
    let value = value.and_then(Value::as_str)?;
    match value.to_ascii_lowercase().as_str() {
        "http" => Some("HTTP".into()),
        "file" => Some("File".into()),
        "inline" => Some("Inline".into()),
        _ => None,
    }
}

fn parse_proxy_providers(root: &Value) -> Result<Vec<ProxyProviderSummary>, String> {
    let providers = root
        .get("providers")
        .and_then(Value::as_object)
        .ok_or("Mihomo proxy provider response is missing providers")?;
    let mut output = Vec::new();
    for (key, value) in providers {
        let Some(vehicle_type) = real_provider_vehicle(value.get("vehicleType")) else {
            continue;
        };
        output.push(ProxyProviderSummary {
            name: value.get("name").and_then(Value::as_str).unwrap_or(key).to_owned(),
            vehicle_type,
            updated_at: value
                .get("updatedAt")
                .and_then(Value::as_str)
                .filter(|value| !value.starts_with("0001-01-01"))
                .map(str::to_owned),
            subscription_info: subscription_info(value.get("subscriptionInfo")),
            proxy_count: value.get("proxies").and_then(Value::as_array).map_or(0, Vec::len),
        });
    }
    output.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(output)
}

fn parse_rule_providers(root: &Value) -> Result<Vec<RuleProviderSummary>, String> {
    let providers = root
        .get("providers")
        .and_then(Value::as_object)
        .ok_or("Mihomo rule provider response is missing providers")?;
    let mut output = Vec::new();
    for (key, value) in providers {
        let Some(vehicle_type) = real_provider_vehicle(value.get("vehicleType")) else {
            continue;
        };
        output.push(RuleProviderSummary {
            name: value.get("name").and_then(Value::as_str).unwrap_or(key).to_owned(),
            vehicle_type,
            behavior: value_string(value.get("behavior")),
            rule_count: value_u64(value.get("ruleCount")),
            updated_at: value
                .get("updatedAt")
                .and_then(Value::as_str)
                .filter(|value| !value.starts_with("0001-01-01"))
                .map(str::to_owned),
        });
    }
    output.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(output)
}

fn parse_connections(root: &Value) -> Result<Connections, String> {
    let empty = Vec::new();
    let items = match root.get("connections") {
        Some(Value::Array(items)) => items,
        Some(Value::Null) | None => &empty,
        _ => return Err("Mihomo /connections response has invalid connections".into()),
    };
    let connections = items
        .iter()
        .map(|item| {
            let metadata = item.get("metadata").unwrap_or(&Value::Null);
            let source_ip = value_string(metadata.get("sourceIP"));
            let source_port = value_string(metadata.get("sourcePort"));
            let destination_ip = value_string(metadata.get("destinationIP"));
            let destination_port = value_string(metadata.get("destinationPort"));
            Connection {
                id: value_string(item.get("id")),
                network: value_string(metadata.get("network")),
                inbound: value_string(metadata.get("type")),
                source: match (source_ip.is_empty(), source_port.is_empty()) {
                    (false, false) => format!("{source_ip}:{source_port}"),
                    _ => source_ip,
                },
                destination: match (destination_ip.is_empty(), destination_port.is_empty()) {
                    (false, false) => format!("{destination_ip}:{destination_port}"),
                    _ => destination_ip,
                },
                host: value_string(metadata.get("host")),
                process: value_string(metadata.get("process")),
                rule: value_string(item.get("rule")),
                rule_payload: value_string(item.get("rulePayload")),
                chains: item
                    .get("chains")
                    .and_then(Value::as_array)
                    .map(|values| values.iter().filter_map(Value::as_str).map(str::to_owned).collect())
                    .unwrap_or_default(),
                upload: item.get("upload").and_then(Value::as_u64).unwrap_or(0),
                download: item.get("download").and_then(Value::as_u64).unwrap_or(0),
                start: value_string(item.get("start")),
            }
        })
        .collect();
    Ok(Connections {
        download_total: root.get("downloadTotal").and_then(Value::as_u64).unwrap_or(0),
        upload_total: root.get("uploadTotal").and_then(Value::as_u64).unwrap_or(0),
        memory: root.get("memory").and_then(Value::as_u64).unwrap_or(0),
        connections,
    })
}

fn parse_groups(root: &Value) -> Result<Vec<ProxyGroup>, String> {
    let proxies = root
        .get("proxies")
        .and_then(Value::as_object)
        .ok_or("Mihomo /proxies response is missing proxies")?;
    let mut groups = BTreeMap::new();
    for (name, value) in proxies {
        let members = value
            .get("all")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if members.is_empty() {
            continue;
        }
        let kind = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("Unknown")
            .to_owned();
        groups.insert(
            name.clone(),
            ProxyGroup {
                name: name.clone(),
                selected: value.get("now").and_then(Value::as_str).map(str::to_owned),
                selectable: kind.eq_ignore_ascii_case("selector"),
                kind,
                members,
            },
        );
    }
    Ok(groups.into_values().collect())
}

fn parse_rules(root: &Value) -> Result<Vec<Rule>, String> {
    let items = root
        .get("rules")
        .and_then(Value::as_array)
        .ok_or("Mihomo /rules response is missing rules")?;
    Ok(items
        .iter()
        .map(|item| Rule {
            kind: item.get("type").and_then(Value::as_str).unwrap_or("Unknown").to_owned(),
            payload: item
                .get("payload")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            proxy: item.get("proxy").and_then(Value::as_str).unwrap_or_default().to_owned(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_selector_without_treating_url_test_as_selectable() {
        let input = json!({
            "proxies": {
                "A": {"type":"Selector", "all":["one","two"], "now":"two"},
                "B": {"type":"URLTest", "all":["one","two"], "now":"one"},
                "one": {"type":"Hysteria2"}
            }
        });
        let groups = parse_groups(&input).unwrap();
        assert_eq!(groups.len(), 2);
        assert!(groups.iter().find(|g| g.name == "A").unwrap().selectable);
        assert!(!groups.iter().find(|g| g.name == "B").unwrap().selectable);
    }

    #[test]
    fn preserves_loaded_rule_order() {
        let input = json!({
            "rules": [
                {"type":"DomainSuffix","payload":"example.com","proxy":"1"},
                {"type":"Match","payload":"","proxy":"DIRECT"}
            ]
        });
        let rules = parse_rules(&input).unwrap();
        assert_eq!(rules[0].proxy, "1");
        assert_eq!(rules[1].kind, "Match");
    }

    #[test]
    fn parses_connection_totals_and_metadata() {
        let input = json!({
            "downloadTotal": 1200,
            "uploadTotal": 300,
            "memory": 4096,
            "connections": [{
                "id":"abc",
                "metadata": {
                    "network":"tcp",
                    "type":"TUN",
                    "sourceIP":"10.0.0.2",
                    "sourcePort":"1234",
                    "destinationIP":"1.1.1.1",
                    "destinationPort":"443",
                    "host":"example.com",
                    "process":"demo"
                },
                "upload":12,
                "download":34,
                "start":"2026-09-24T00:00:00Z",
                "chains":["1","node"],
                "rule":"DomainSuffix",
                "rulePayload":"example.com"
            }]
        });
        let parsed = parse_connections(&input).unwrap();
        assert_eq!(parsed.download_total, 1200);
        assert_eq!(parsed.upload_total, 300);
        assert_eq!(parsed.connections[0].destination, "1.1.1.1:443");
        assert_eq!(parsed.connections[0].chains, ["1", "node"]);
    }

    #[test]
    fn accepts_null_connections_when_core_is_idle() {
        let parsed = parse_connections(&json!({
            "downloadTotal": 0,
            "uploadTotal": 0,
            "connections": null,
            "memory": 0
        }))
        .unwrap();
        assert!(parsed.connections.is_empty());
    }

    #[test]
    fn providers_skip_internal_compatible_entries() {
        let proxies = json!({
            "providers": {
                "default": {"name":"default","vehicleType":"Compatible","proxies":[]},
                "remote": {
                    "name":"remote","vehicleType":"HTTP","proxies":[{"name":"one"}],
                    "updatedAt":"2026-09-24T08:00:00Z",
                    "subscriptionInfo":{"Upload":1,"Download":2,"Total":10,"Expire":123}
                }
            }
        });
        let parsed = parse_proxy_providers(&proxies).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "remote");
        assert_eq!(parsed[0].proxy_count, 1);
        assert_eq!(parsed[0].subscription_info.as_ref().unwrap().download, 2);

        let rules = json!({
            "providers": {
                "local": {"name":"local","vehicleType":"File","behavior":"Domain","ruleCount":12}
            }
        });
        let parsed = parse_rule_providers(&rules).unwrap();
        assert_eq!(parsed[0].behavior, "Domain");
        assert_eq!(parsed[0].rule_count, 12);
    }
}
