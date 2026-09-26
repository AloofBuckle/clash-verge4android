use clash_verge_mobile::{
    Document, Profile, RuntimeEnhancements, RuntimeOverrides, Store, runtime_config_tun_enabled,
    runtime_preview_config_with_enhancements, runtime_tun_config_with_enhancements,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    ffi::CString,
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    net::TcpListener,
    os::{
        fd::AsRawFd,
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_ROOT: &str = "/data/adb/clash-verge4android";
const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024 + 16 * 1024;
const MAX_PROFILE_BYTES: usize = 4 * 1024 * 1024;
const MOBILE_BACKUP_FORMAT_VERSION: u32 = 1;
const MAX_MOBILE_BACKUP_BYTES: usize = 16 * 1024 * 1024;
const AUTO_BACKUP_KEEP: usize = 20;
const CORE_BYPASS_PREF: &str = "8899";
const CORE_BYPASS_MARK: &str = "0x20000000/0x20000000";

fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

fn build_http_client(
    timeout_seconds: u64,
    proxy: Option<&str>,
    accept_invalid_certs: bool,
) -> Result<reqwest::blocking::Client, String> {
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let tls = rustls::ClientConfig::builder_with_provider(rustls::crypto::aws_lc_rs::default_provider().into())
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_root_certificates(roots)
        .with_no_client_auth();
    let mut builder = reqwest::blocking::Client::builder()
        .tls_backend_preconfigured(tls)
        .timeout(Duration::from_secs(timeout_seconds))
        .connect_timeout(Duration::from_secs(timeout_seconds.min(10)))
        .danger_accept_invalid_certs(accept_invalid_certs)
        .no_proxy();
    if let Some(proxy) = proxy {
        builder = builder.proxy(reqwest::Proxy::all(proxy).map_err(|e| e.to_string())?);
    }
    builder.build().map_err(|e| e.to_string())
}

fn render_profile_runtime(
    document: &Document,
    profile: &Profile,
    overrides: &RuntimeOverrides,
    transparent: bool,
) -> Result<String, String> {
    let enhancements = RuntimeEnhancements {
        rules: profile.rules_yaml.as_deref(),
        proxies: profile.proxies_yaml.as_deref(),
        groups: profile.groups_yaml.as_deref(),
        global_merge: document.global_merge_yaml.as_deref(),
        profile_merge: profile.merge_yaml.as_deref(),
        global_script: document.global_script_js.as_deref(),
        profile_script: profile.script_js.as_deref(),
        profile_name: &profile.name,
    };
    if transparent {
        runtime_tun_config_with_enhancements(&profile.yaml, overrides, enhancements, profile.proxy_chain.as_ref())
    } else {
        runtime_preview_config_with_enhancements(&profile.yaml, overrides, enhancements, profile.proxy_chain.as_ref())
    }
}

fn app_process_running(uid: u32) -> bool {
    let Ok(entries) = fs::read_dir("/proc") else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry.file_name().to_string_lossy().parse::<u32>().is_ok()
            && entry.metadata().ok().is_some_and(|metadata| metadata.uid() == uid)
    })
}

fn cv4a_routing_active(rules: &str, routes: &str) -> bool {
    rules
        .lines()
        .any(|line| line.starts_with("8900:") || line.starts_with("8901:"))
        && routes.lines().any(|line| line.contains("dev Mihomo"))
}

fn coexistence_rules_active(rules: &str) -> bool {
    rules
        .lines()
        .any(|line| line.starts_with("8888:") && line.contains("iif tun0 goto 10000"))
        && rules
            .lines()
            .any(|line| line.starts_with("8889:") && line.contains("iif lo goto 10000"))
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
    Status,
    ReadCoreVersion,
    ApplyConfig {
        yaml: String,
    },
    UpgradeCore {
        staged_path: String,
        expected_version: String,
    },
    StartCore,
    StopCore,
    RestartCore,
    SetTransparent {
        enable: bool,
    },
    SetVpnCoexistence {
        enable: bool,
    },
    ReadLog {
        lines: usize,
    },
    ClearLog,
    Shutdown,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Response {
    ok: bool,
    error: Option<String>,
    status: Option<Status>,
    log: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    version: &'static str,
    agent_pid: u32,
    core_running: bool,
    core_pid: Option<i32>,
    core_installed: bool,
    config_present: bool,
    geosite_present: bool,
    controller_socket: String,
    transparent_active: bool,
    vpn_coexistence_active: bool,
}

#[derive(Clone, Debug)]
struct Runtime {
    root: PathBuf,
    controller_socket: PathBuf,
    peer_uid: u32,
    socket_context: Option<String>,
    profiles_path: Option<PathBuf>,
    runtime_overrides_path: Option<PathBuf>,
    backup_settings_path: Option<PathBuf>,
    backup_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct PersistedBackupSettings {
    auto_schedule_enabled: bool,
    auto_interval_hours: u64,
    auto_on_change: bool,
    last_auto_backup_at: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistedMobileBackup {
    format_version: u32,
    created_at: u64,
    profiles: Document,
    runtime_overrides: RuntimeOverrides,
}

impl Runtime {
    fn new(
        root: PathBuf,
        controller_socket: PathBuf,
        peer_uid: u32,
        socket_context: Option<String>,
        profiles_path: Option<PathBuf>,
        runtime_overrides_path: Option<PathBuf>,
        backup_settings_path: Option<PathBuf>,
        backup_dir: Option<PathBuf>,
    ) -> Result<Self, String> {
        if !root.is_absolute() {
            return Err("Runtime root must be absolute".into());
        }
        if !controller_socket.is_absolute() {
            return Err("Controller socket must be absolute".into());
        }
        Ok(Self {
            controller_socket,
            root,
            peer_uid,
            socket_context,
            profiles_path,
            runtime_overrides_path,
            backup_settings_path,
            backup_dir,
        })
    }

    fn bin(&self) -> PathBuf {
        self.root.join("bin/mihomo")
    }

    fn config(&self) -> PathBuf {
        self.root.join("config/runtime.yaml")
    }

    fn pid_file(&self) -> PathBuf {
        self.root.join("run/mihomo.pid")
    }

    fn log_file(&self) -> PathBuf {
        self.root.join("logs/mihomo.log")
    }

    fn data_dir(&self) -> PathBuf {
        self.root.join("data")
    }

    fn prepare(&self) -> Result<(), String> {
        for relative in ["bin", "config", "data", "logs", "run"] {
            let path = self.root.join(relative);
            fs::create_dir_all(&path).map_err(|e| format!("create {}: {e}", path.display()))?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                .map_err(|e| format!("chmod {}: {e}", path.display()))?;
        }
        Ok(())
    }

    fn core_pid(&self) -> Option<i32> {
        let raw = fs::read_to_string(self.pid_file()).ok()?;
        let pid = raw.trim().parse::<i32>().ok()?;
        if pid > 1 && process_matches(pid, &self.bin()) {
            Some(pid)
        } else {
            let _ = fs::remove_file(self.pid_file());
            None
        }
    }

    fn transparent_active(&self, core_pid: Option<i32>) -> bool {
        if core_pid.is_none() || !Path::new("/sys/class/net/Mihomo").exists() {
            return false;
        }
        let Ok(rules) = Command::new("ip").args(["rule", "show"]).output() else {
            return false;
        };
        let Ok(routes) = Command::new("ip").args(["route", "show", "table", "3022"]).output() else {
            return false;
        };
        if !rules.status.success() || !routes.status.success() {
            return false;
        }
        let rules = String::from_utf8_lossy(&rules.stdout);
        let routes = String::from_utf8_lossy(&routes.stdout);
        cv4a_routing_active(&rules, &routes)
    }

    fn vpn_coexistence_active(&self) -> bool {
        if !Path::new("/sys/class/net/Mihomo").exists() || !Path::new("/sys/class/net/tun0").exists() {
            return false;
        }
        let Ok(v4) = Command::new("ip").args(["rule", "show"]).output() else {
            return false;
        };
        if !v4.status.success() || !coexistence_rules_active(&String::from_utf8_lossy(&v4.stdout)) {
            return false;
        }
        true
    }

    fn remove_rule_pref(&self, ipv6: bool, pref: &str) {
        for _ in 0..4 {
            let mut command = Command::new("ip");
            if ipv6 {
                command.arg("-6");
            }
            let Ok(status) = command
                .args(["rule", "del", "pref", pref])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
            else {
                break;
            };
            if !status.success() {
                break;
            }
        }
    }

    fn add_rule(&self, ipv6: bool, args: &[&str]) -> Result<(), String> {
        let mut command = Command::new("ip");
        if ipv6 {
            command.arg("-6");
        }
        let output = command.args(args).output().map_err(|e| format!("run ip rule: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("ip rule failed: {}", stderr.trim()));
        }
        Ok(())
    }

    fn android_netd_rule_priority(&self) -> Result<String, String> {
        let output = Command::new("ip")
            .args(["rule", "show"])
            .output()
            .map_err(|e| format!("read Android policy rules: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "read Android policy rules: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let rules = String::from_utf8_lossy(&output.stdout);
        rules
            .lines()
            .find_map(|line| {
                if !line.contains("lookup legacy_system") {
                    return None;
                }
                let (priority, _) = line.split_once(':')?;
                priority.trim().parse::<u32>().ok().filter(|value| *value > 8910)
            })
            .map(|priority| priority.to_string())
            .ok_or_else(|| "Android netd policy entry was not found".into())
    }

    fn clear_core_bypass_rules(&self) {
        for ipv6 in [false, true] {
            self.remove_rule_pref(ipv6, CORE_BYPASS_PREF);
        }
    }

    fn install_core_bypass_rules(&self) -> Result<(), String> {
        self.clear_core_bypass_rules();
        let target = self.android_netd_rule_priority()?;
        self.add_rule(
            false,
            &[
                "rule",
                "add",
                "pref",
                CORE_BYPASS_PREF,
                "fwmark",
                CORE_BYPASS_MARK,
                "goto",
                &target,
            ],
        )?;
        if let Err(error) = self.add_rule(
            true,
            &[
                "rule",
                "add",
                "pref",
                CORE_BYPASS_PREF,
                "fwmark",
                CORE_BYPASS_MARK,
                "goto",
                &target,
            ],
        ) {
            self.clear_core_bypass_rules();
            return Err(error);
        }
        Ok(())
    }

    fn tun0_has_ipv6(&self) -> bool {
        let Ok(text) = fs::read_to_string("/proc/net/if_inet6") else {
            return false;
        };
        text.lines().any(|line| line.split_whitespace().last() == Some("tun0"))
    }

    fn coexistence_marker(&self) -> PathBuf {
        self.root.join("config/vpn-coexistence.enabled")
    }

    fn coexistence_desired(&self) -> bool {
        self.coexistence_marker().is_file()
    }

    fn clear_vpn_coexistence_rules(&self) {
        for ipv6 in [false, true] {
            self.remove_rule_pref(ipv6, "8888");
            self.remove_rule_pref(ipv6, "8889");
        }
    }

    fn reconcile_vpn_coexistence(&self) -> Result<(), String> {
        self.clear_vpn_coexistence_rules();
        if !self.coexistence_desired() {
            return Ok(());
        }
        if !Path::new("/sys/class/net/Mihomo").exists() || !Path::new("/sys/class/net/tun0").exists() {
            return Ok(());
        }
        let target = self.android_netd_rule_priority()?;
        self.add_rule(false, &["rule", "add", "pref", "8888", "iif", "tun0", "goto", &target])?;
        self.add_rule(false, &["rule", "add", "pref", "8889", "iif", "lo", "goto", &target])?;
        if self.tun0_has_ipv6() {
            if let Err(error) = self
                .add_rule(true, &["rule", "add", "pref", "8888", "iif", "tun0", "goto", &target])
                .and_then(|_| self.add_rule(true, &["rule", "add", "pref", "8889", "iif", "lo", "goto", &target]))
            {
                self.remove_rule_pref(false, "8888");
                self.remove_rule_pref(false, "8889");
                self.remove_rule_pref(true, "8888");
                self.remove_rule_pref(true, "8889");
                return Err(error);
            }
        }
        Ok(())
    }

    fn set_vpn_coexistence(&self, enable: bool) -> Result<(), String> {
        self.prepare()?;
        if enable {
            write_private(&self.coexistence_marker(), b"enabled\n")?;
        } else {
            let _ = fs::remove_file(self.coexistence_marker());
        }
        self.reconcile_vpn_coexistence()
    }

    fn status(&self) -> Status {
        let core_pid = self.core_pid();
        let transparent_active = self.transparent_active(core_pid);
        Status {
            version: VERSION,
            agent_pid: std::process::id(),
            core_running: core_pid.is_some(),
            core_pid,
            core_installed: self.bin().is_file(),
            config_present: self.config().is_file(),
            geosite_present: self.data_dir().join("GeoSite.dat").is_file(),
            controller_socket: self.controller_socket.display().to_string(),
            transparent_active,
            vpn_coexistence_active: self.vpn_coexistence_active(),
        }
    }

    fn apply_config(&self, yaml: &str) -> Result<(), String> {
        if yaml.len() > 4 * 1024 * 1024 {
            return Err("Configuration exceeds 4 MiB".into());
        }
        if yaml.trim().is_empty() {
            return Err("Configuration is empty".into());
        }
        self.prepare()?;
        let candidate = self.root.join("config/runtime.yaml.next");
        write_private(&candidate, yaml.as_bytes())?;
        if self.bin().is_file() {
            let output = Command::new(self.bin())
                .args(["-t", "-d"])
                .arg(self.data_dir())
                .arg("-f")
                .arg(&candidate)
                .output()
                .map_err(|e| format!("start mihomo validation: {e}"))?;
            if !output.status.success() {
                let _ = fs::remove_file(&candidate);
                let message = String::from_utf8_lossy(&output.stderr);
                return Err(format!("mihomo rejected configuration: {}", message.trim()));
            }
        }
        fs::rename(&candidate, self.config()).map_err(|e| format!("activate configuration: {e}"))?;
        Ok(())
    }

    fn read_core_version(&self) -> Result<String, String> {
        read_core_version(&self.bin())
    }

    fn validate_upgrade_source(&self, staged_path: &Path) -> Result<PathBuf, String> {
        let profiles = self.profiles_path.as_ref().ok_or("profiles path is not configured")?;
        let app_dir = profiles
            .parent()
            .ok_or("profiles path has no parent")?
            .canonicalize()
            .map_err(|e| format!("canonicalize app data directory: {e}"))?;
        let allowed = app_dir.join("runtime-stage");
        let staged = staged_path
            .canonicalize()
            .map_err(|e| format!("canonicalize staged core: {e}"))?;
        if !staged.starts_with(&allowed) {
            return Err("staged core is outside the app runtime-stage directory".into());
        }
        let metadata = fs::metadata(&staged).map_err(|e| format!("inspect staged core: {e}"))?;
        if !metadata.is_file() || metadata.len() == 0 || metadata.len() > 128 * 1024 * 1024 {
            return Err("staged core has an invalid size or type".into());
        }
        Ok(staged)
    }

    fn upgrade_core(&self, staged_path: &Path, expected_version: &str) -> Result<(), String> {
        if !is_usable_version(expected_version) {
            return Err("invalid expected Mihomo version".into());
        }
        self.prepare()?;
        let source = self.validate_upgrade_source(staged_path)?;
        let target = self.bin();
        let candidate = self.root.join(format!("bin/.mihomo.{}.upgrade", std::process::id()));
        let rollback = self.root.join("bin/.mihomo.rollback");
        let _ = fs::remove_file(&candidate);
        let _ = fs::remove_file(&rollback);

        fs::copy(&source, &candidate).map_err(|e| format!("stage upgraded Mihomo: {e}"))?;
        fs::set_permissions(&candidate, fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("chmod staged Mihomo: {e}"))?;
        let staged_version = read_core_version(&candidate)?;
        if staged_version != expected_version {
            let _ = fs::remove_file(&candidate);
            return Err(format!(
                "staged Mihomo reports {staged_version}, expected {expected_version}"
            ));
        }
        if self.config().is_file() {
            let output = Command::new(&candidate)
                .args(["-t", "-d"])
                .arg(self.data_dir())
                .arg("-f")
                .arg(self.config())
                .output()
                .map_err(|e| format!("validate upgraded Mihomo: {e}"))?;
            if !output.status.success() {
                let _ = fs::remove_file(&candidate);
                return Err(format!(
                    "upgraded Mihomo rejected the runtime configuration: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
        }

        let was_running = self.core_pid().is_some();
        let restorable = target.is_file() && fs::hard_link(&target, &rollback).is_ok();
        if was_running && let Err(error) = self.stop_core() {
            let _ = fs::remove_file(&candidate);
            let _ = fs::remove_file(&rollback);
            return Err(format!("stop current Mihomo before upgrade: {error}"));
        }

        if let Err(error) = fs::rename(&candidate, &target) {
            if was_running {
                let _ = self.start_core();
            }
            let _ = fs::remove_file(&rollback);
            return Err(format!("publish upgraded Mihomo: {error}"));
        }
        let result = if was_running { self.start_core() } else { Ok(()) };
        if let Err(error) = result {
            if restorable {
                let _ = fs::rename(&rollback, &target);
                if was_running {
                    let _ = self.start_core();
                }
            }
            let _ = fs::remove_file(&rollback);
            return Err(format!(
                "upgraded Mihomo failed to start; previous core restored when possible: {error}"
            ));
        }
        let running_version = read_core_version(&target)?;
        if running_version != expected_version {
            if was_running {
                let _ = self.stop_core();
            }
            if restorable {
                let _ = fs::rename(&rollback, &target);
                if was_running {
                    let _ = self.start_core();
                }
            }
            let _ = fs::remove_file(&rollback);
            return Err(format!(
                "published Mihomo reports {running_version}, expected {expected_version}"
            ));
        }
        let _ = fs::remove_file(&rollback);
        Ok(())
    }

    fn start_core(&self) -> Result<(), String> {
        self.prepare()?;
        if self.core_pid().is_some() {
            let source =
                fs::read_to_string(self.config()).map_err(|e| format!("read {}: {e}", self.config().display()))?;
            if runtime_config_tun_enabled(&source) {
                self.install_core_bypass_rules()?;
            } else {
                self.clear_core_bypass_rules();
            }
            return Ok(());
        }
        if !self.bin().is_file() {
            return Err(format!("mihomo is not installed at {}", self.bin().display()));
        }
        if !self.config().is_file() {
            return Err("No runtime configuration has been applied".into());
        }
        let launch_config = self.prepare_launch_config()?;
        let _ = fs::remove_file(&self.controller_socket);
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.log_file())
            .map_err(|e| format!("open mihomo log: {e}"))?;
        let err = log.try_clone().map_err(|e| format!("clone mihomo log: {e}"))?;
        let mut child = Command::new(self.bin())
            .arg("-d")
            .arg(self.data_dir())
            .arg("-f")
            .arg(&launch_config)
            .arg("-ext-ctl-unix")
            .arg(&self.controller_socket)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(err))
            .spawn()
            .map_err(|e| {
                self.clear_core_bypass_rules();
                format!("start mihomo: {e}")
            })?;
        let pid = i32::try_from(child.id()).map_err(|_| "mihomo PID does not fit i32")?;
        write_private(&self.pid_file(), format!("{pid}\n").as_bytes())?;
        let pid_file = self.pid_file();
        let cleanup = self.clone();
        thread::spawn(move || {
            let _ = child.wait();
            let _ = fs::remove_file(pid_file);
            cleanup.clear_core_bypass_rules();
        });
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if !process_alive(pid) {
                self.clear_core_bypass_rules();
                let _ = fs::remove_file(self.pid_file());
                return Err("mihomo exited during startup; inspect logs/mihomo.log".into());
            }
            if self.controller_socket.exists() {
                fs::set_permissions(&self.controller_socket, fs::Permissions::from_mode(0o600))
                    .map_err(|e| format!("chmod controller socket: {e}"))?;
                chown_path(&self.controller_socket, self.peer_uid, self.peer_uid)?;
                if let Some(context) = &self.socket_context {
                    relabel_path(&self.controller_socket, context)?;
                }
                if let Err(error) = self.restore_selected_nodes() {
                    eprintln!("selection restore deferred/failed: {error}");
                }
                if self.coexistence_desired() && Path::new("/sys/class/net/tun0").exists() {
                    let coexist_deadline = Instant::now() + Duration::from_secs(2);
                    while Instant::now() < coexist_deadline {
                        if Path::new("/sys/class/net/Mihomo").exists() {
                            if let Err(error) = self.reconcile_vpn_coexistence() {
                                eprintln!("VPN coexistence restore failed: {error}");
                            }
                            break;
                        }
                        thread::sleep(Duration::from_millis(50));
                    }
                }
                return Ok(());
            }
            thread::sleep(Duration::from_millis(40));
        }
        if process_alive(pid) {
            Ok(())
        } else {
            self.clear_core_bypass_rules();
            let _ = fs::remove_file(self.pid_file());
            Err("mihomo failed to stay running".into())
        }
    }

    fn launch_config(&self) -> PathBuf {
        self.root.join("config/runtime.launch.yaml")
    }

    fn prepare_launch_config(&self) -> Result<PathBuf, String> {
        let source = fs::read_to_string(self.config()).map_err(|e| format!("read {}: {e}", self.config().display()))?;
        if !runtime_config_tun_enabled(&source) {
            self.clear_core_bypass_rules();
            let _ = fs::remove_file(self.launch_config());
            return Ok(self.config());
        }
        self.install_core_bypass_rules()?;
        let _ = fs::remove_file(self.launch_config());
        Ok(self.config())
    }

    fn persisted_selections(&self) -> Result<Vec<clash_verge_mobile::SelectedProxy>, String> {
        let Some(path) = &self.profiles_path else {
            return Ok(Vec::new());
        };
        let document = match Store::open(path.clone()) {
            Ok(store) => store.document(),
            Err(_error) if !path.exists() => return Ok(Vec::new()),
            Err(error) => return Err(format!("read {}: {error}", path.display())),
        };
        let Some(active_id) = document.active_id else {
            return Ok(Vec::new());
        };
        Ok(document
            .profiles
            .into_iter()
            .find(|profile| profile.id == active_id)
            .map(|profile| profile.selected)
            .unwrap_or_default())
    }

    fn restore_selected_nodes(&self) -> Result<(), String> {
        let mut pending = self.persisted_selections()?;
        if pending.is_empty() {
            return Ok(());
        }
        let deadline = Instant::now() + Duration::from_secs(4);
        let mut last_error = None;
        while !pending.is_empty() && Instant::now() < deadline {
            pending.retain(
                |selection| match self.select_proxy(&selection.group, &selection.member) {
                    Ok(()) => false,
                    Err(error) => {
                        last_error = Some(error);
                        true
                    }
                },
            );
            if !pending.is_empty() {
                thread::sleep(Duration::from_millis(100));
            }
        }
        if pending.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "could not restore {} selected proxy group(s): {}",
                pending.len(),
                last_error.unwrap_or_else(|| "unknown controller error".into())
            ))
        }
    }

    fn select_proxy(&self, group: &str, member: &str) -> Result<(), String> {
        if group.is_empty() || member.is_empty() {
            return Err("empty proxy group selection".into());
        }
        let path = format!("/proxies/{}", percent_encode_path_segment(group));
        let body = serde_json::to_vec(&serde_json::json!({ "name": member })).map_err(|e| e.to_string())?;
        let mut stream =
            UnixStream::connect(&self.controller_socket).map_err(|e| format!("connect Mihomo controller: {e}"))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(|e| e.to_string())?;
        write!(
            stream,
            "PUT {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .map_err(|e| format!("write Mihomo request: {e}"))?;
        stream
            .write_all(&body)
            .map_err(|e| format!("write Mihomo request body: {e}"))?;
        stream.flush().map_err(|e| format!("flush Mihomo request: {e}"))?;
        let mut reader = BufReader::new(stream);
        let mut status = String::new();
        reader
            .read_line(&mut status)
            .map_err(|e| format!("read Mihomo response: {e}"))?;
        let code = status
            .split_whitespace()
            .nth(1)
            .and_then(|value| value.parse::<u16>().ok())
            .ok_or_else(|| format!("invalid Mihomo HTTP response: {}", status.trim()))?;
        if (200..300).contains(&code) {
            Ok(())
        } else {
            Err(format!("Mihomo selection returned HTTP {code}"))
        }
    }

    fn controller_json(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let body = match body {
            Some(value) => serde_json::to_vec(value).map_err(|e| e.to_string())?,
            None => Vec::new(),
        };
        let mut stream =
            UnixStream::connect(&self.controller_socket).map_err(|e| format!("connect Mihomo controller: {e}"))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_secs(3)))
            .map_err(|e| e.to_string())?;
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .map_err(|e| format!("write Mihomo request: {e}"))?;
        if !body.is_empty() {
            stream
                .write_all(&body)
                .map_err(|e| format!("write Mihomo request body: {e}"))?;
        }
        stream.flush().map_err(|e| format!("flush Mihomo request: {e}"))?;

        let mut reader = BufReader::new(stream);
        let mut status = String::new();
        reader
            .read_line(&mut status)
            .map_err(|e| format!("read Mihomo response: {e}"))?;
        let code = status
            .split_whitespace()
            .nth(1)
            .and_then(|value| value.parse::<u16>().ok())
            .ok_or_else(|| format!("invalid Mihomo HTTP response: {}", status.trim()))?;
        loop {
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .map_err(|e| format!("read Mihomo response header: {e}"))?;
            if line == "\r\n" || line.is_empty() {
                break;
            }
        }
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .map_err(|e| format!("read Mihomo response body: {e}"))?;
        if !(200..300).contains(&code) {
            return Err(format!(
                "Mihomo controller returned HTTP {code}: {}",
                String::from_utf8_lossy(&bytes).trim()
            ));
        }
        if bytes.is_empty() {
            Ok(serde_json::Value::Null)
        } else {
            serde_json::from_slice(&bytes).map_err(|e| format!("parse Mihomo response: {e}"))
        }
    }

    fn patch_controller_config(&self, patch: serde_json::Value) -> Result<(), String> {
        self.controller_json("PATCH", "/configs", Some(&patch)).map(|_| ())
    }

    fn restore_app_file_metadata(&self, path: &Path) -> Result<(), String> {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("chmod {}: {e}", path.display()))?;
        chown_path(path, self.peer_uid, self.peer_uid)?;
        if let Some(context) = &self.socket_context {
            relabel_path(path, context)?;
        }
        Ok(())
    }

    fn restore_app_dir_metadata(&self, path: &Path) -> Result<(), String> {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("chmod {}: {e}", path.display()))?;
        chown_path(path, self.peer_uid, self.peer_uid)?;
        if let Some(context) = &self.socket_context {
            relabel_path(path, context)?;
        }
        Ok(())
    }

    fn load_backup_settings(&self) -> Option<PersistedBackupSettings> {
        self.backup_settings_path
            .as_ref()
            .and_then(|path| fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<PersistedBackupSettings>(&bytes).ok())
    }

    fn set_last_auto_backup_at(&self, timestamp: u64) -> Result<(), String> {
        let path = self
            .backup_settings_path
            .as_ref()
            .ok_or("backup settings path is not configured")?;
        let bytes = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let mut value = serde_json::from_slice::<serde_json::Value>(&bytes)
            .map_err(|e| format!("parse {}: {e}", path.display()))?;
        let object = value.as_object_mut().ok_or("backup settings JSON must be an object")?;
        object.insert("lastAutoBackupAt".into(), serde_json::Value::from(timestamp));
        let bytes = serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?;
        let temporary = path.with_extension("json.root-agent.tmp");
        write_private(&temporary, &bytes)?;
        fs::rename(&temporary, path).map_err(|e| format!("replace {}: {e}", path.display()))?;
        self.restore_app_file_metadata(path)
    }

    fn cleanup_auto_backups(&self, dir: &Path) -> Result<(), String> {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(format!("read {}: {error}", dir.display())),
        };
        let mut files = Vec::<(PathBuf, SystemTime)>::new();
        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().into_owned();
            if !filename.contains("-auto-") || !filename.ends_with(".json") {
                continue;
            }
            let modified = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(UNIX_EPOCH);
            files.push((entry.path(), modified));
        }
        if files.len() <= AUTO_BACKUP_KEEP {
            return Ok(());
        }
        files.sort_by_key(|(_, modified)| *modified);
        let remove_count = files.len() - AUTO_BACKUP_KEEP;
        for (path, _) in files.into_iter().take(remove_count) {
            fs::remove_file(&path).map_err(|e| format!("remove {}: {e}", path.display()))?;
        }
        Ok(())
    }

    fn record_scheduled_backup(&self) -> Result<(), String> {
        let profiles_path = self.profiles_path.as_ref().ok_or("profiles path is not configured")?;
        let backup_dir = self.backup_dir.as_ref().ok_or("backup directory is not configured")?;
        let profiles = Store::open(profiles_path.clone())?.document();
        let runtime_overrides = self.load_runtime_overrides();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?;
        let backup = PersistedMobileBackup {
            format_version: MOBILE_BACKUP_FORMAT_VERSION,
            created_at: now.as_secs(),
            profiles,
            runtime_overrides,
        };
        let bytes = serde_json::to_vec_pretty(&backup).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_MOBILE_BACKUP_BYTES {
            return Err("automatic backup exceeds 16 MiB".into());
        }
        fs::create_dir_all(backup_dir).map_err(|e| format!("create {}: {e}", backup_dir.display()))?;
        self.restore_app_dir_metadata(backup_dir)?;
        let filename = format!(
            "cv4a-backup-{}-{}-auto-scheduled.json",
            now.as_secs(),
            now.subsec_nanos()
        );
        let path = backup_dir.join(filename);
        write_private(&path, &bytes)?;
        self.restore_app_file_metadata(&path)?;
        self.cleanup_auto_backups(backup_dir)?;
        self.set_last_auto_backup_at(now.as_secs())
    }

    fn auto_backup_loop(self) {
        thread::sleep(Duration::from_secs(30));
        loop {
            if !app_process_running(self.peer_uid)
                && let Some(settings) = self.load_backup_settings()
                && settings.auto_schedule_enabled
            {
                let now = unix_time();
                let interval = settings.auto_interval_hours.clamp(1, 168).saturating_mul(3600);
                if now.saturating_sub(settings.last_auto_backup_at.unwrap_or(0)) >= interval {
                    match self.record_scheduled_backup() {
                        Ok(()) => eprintln!("scheduled automatic backup completed"),
                        Err(error) => eprintln!("scheduled automatic backup failed: {error}"),
                    }
                }
            }
            thread::sleep(Duration::from_secs(60));
        }
    }

    fn download_profile(&self, profile: &clash_verge_mobile::Profile) -> Result<String, String> {
        let source = profile
            .source
            .as_deref()
            .ok_or("profile is not a remote subscription")?;
        let url = reqwest::Url::parse(source).map_err(|e| format!("invalid subscription URL: {e}"))?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err("subscription URL must be HTTP(S) without embedded credentials".into());
        }
        let option = profile.option.clone().normalized()?;
        if option.with_proxy.unwrap_or(false) {
            return Err("system-proxy subscription updates are not available in the root agent".into());
        }
        let timeout = option.timeout_seconds.unwrap_or(25);
        let user_agent = option.user_agent.as_deref().unwrap_or("ClashVerge4Android/0.1.0");

        let mut previous_controller = None;
        let mut proxy_arg = None;
        if option.self_proxy.unwrap_or(false) {
            if self.core_pid().is_none() || !self.controller_socket.exists() {
                return Err("Mihomo must be running to update this subscription through Clash".into());
            }
            let config = self.controller_json("GET", "/configs", None)?;
            let mixed_port = config
                .get("mixed-port")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            let allow_lan = config
                .get("allow-lan")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let bind_address = config
                .get("bind-address")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("*")
                .to_owned();
            previous_controller = Some(serde_json::json!({
                "mixed-port": mixed_port,
                "allow-lan": allow_lan,
                "bind-address": bind_address,
            }));
            let listener =
                TcpListener::bind(("127.0.0.1", 0)).map_err(|e| format!("reserve subscription proxy port: {e}"))?;
            let port = listener
                .local_addr()
                .map_err(|e| format!("read subscription proxy port: {e}"))?
                .port();
            drop(listener);
            self.patch_controller_config(serde_json::json!({
                "mixed-port": port,
                "allow-lan": false,
                "bind-address": "127.0.0.1",
            }))?;
            proxy_arg = Some(format!("http://127.0.0.1:{port}"));
        }

        let result = (|| {
            let client = build_http_client(
                timeout,
                proxy_arg.as_deref(),
                option.danger_accept_invalid_certs.unwrap_or(false),
            )?;
            let mut response = client
                .get(url)
                .header("User-Agent", user_agent)
                .send()
                .map_err(|e| format!("subscription download failed: {}", e.without_url()))?
                .error_for_status()
                .map_err(|e| format!("subscription HTTP error: {}", e.without_url()))?;
            if response
                .content_length()
                .is_some_and(|length| length > MAX_PROFILE_BYTES as u64)
            {
                return Err("subscription response is empty or exceeds 4 MiB".into());
            }
            let mut bytes = Vec::new();
            response
                .by_ref()
                .take((MAX_PROFILE_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|e| format!("read subscription response: {e}"))?;
            if bytes.is_empty() || bytes.len() > MAX_PROFILE_BYTES {
                return Err("subscription response is empty or exceeds 4 MiB".into());
            }
            let yaml = String::from_utf8(bytes).map_err(|_| "subscription response is not UTF-8".to_owned())?;
            clash_verge_mobile::inspect(&yaml)?;
            Ok(yaml)
        })();

        let restore = previous_controller
            .map(|patch| self.patch_controller_config(patch))
            .unwrap_or(Ok(()));
        match (result, restore) {
            (Ok(yaml), Ok(())) => Ok(yaml),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) => Err(format!(
                "subscription downloaded, but temporary Clash proxy settings could not be restored: {error}"
            )),
            (Err(error), Err(restore_error)) => Err(format!(
                "{error}; temporary Clash proxy restore also failed: {restore_error}"
            )),
        }
    }

    fn load_runtime_overrides(&self) -> RuntimeOverrides {
        self.runtime_overrides_path
            .as_ref()
            .and_then(|path| fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<RuntimeOverrides>(&bytes).ok())
            .unwrap_or_default()
    }

    fn set_transparent(&self, enable: bool) -> Result<(), String> {
        let profiles_path = self.profiles_path.as_ref().ok_or("profiles path is not configured")?;
        let document = Store::open(profiles_path.clone())?.document();
        let active_id = document.active_id.as_ref().ok_or("no active profile")?;
        let active = document
            .profiles
            .iter()
            .find(|profile| &profile.id == active_id)
            .ok_or("active profile is missing")?;
        let rendered = render_profile_runtime(&document, active, &self.load_runtime_overrides(), enable)?;
        let previous_runtime = fs::read_to_string(self.config()).ok();
        let was_running = self.core_pid().is_some();

        self.apply_config(&rendered)?;
        let apply_result = (|| {
            if was_running {
                self.stop_core()?;
            }
            self.start_core()
        })();
        if let Err(error) = apply_result {
            if let Some(previous_runtime) = previous_runtime {
                let _ = self.apply_config(&previous_runtime);
                if was_running {
                    let _ = self.start_core();
                }
            }
            return Err(format!("switch transparent mode: {error}"));
        }
        Ok(())
    }

    fn refresh_profile(&self, id: &str) -> Result<(), String> {
        let path = self.profiles_path.as_ref().ok_or("profiles path is not configured")?;
        let initial = Store::open(path.clone())?;
        let initial_document = initial.document();
        let initial_profile = initial_document
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .cloned()
            .ok_or("profile not found")?;
        let yaml = self.download_profile(&initial_profile)?;

        if app_process_running(self.peer_uid) {
            return Ok(());
        }

        let mut store = Store::open(path.clone())?;
        let before = store.document();
        let profile = before
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .cloned()
            .ok_or("profile disappeared during update")?;
        if profile.yaml != initial_profile.yaml {
            return Ok(());
        }
        let updated = store.replace(id, yaml, &profile.yaml)?;
        self.restore_app_file_metadata(path)?;
        if before.active_id.as_deref() != Some(id) {
            return Ok(());
        }

        let active = updated
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .ok_or("updated active profile is missing")?;
        let previous_runtime = fs::read_to_string(self.config()).ok();
        let core_was_running = self.core_pid().is_some();
        let transparent = if core_was_running {
            self.transparent_active(self.core_pid())
        } else {
            previous_runtime.as_deref().is_some_and(runtime_config_tun_enabled)
        };
        let overrides = self.load_runtime_overrides();
        let rendered = render_profile_runtime(&updated, active, &overrides, transparent)?;

        let apply_result = (|| {
            self.apply_config(&rendered)?;
            if core_was_running {
                self.stop_core()?;
                self.start_core()?;
            }
            Ok::<(), String>(())
        })();
        if let Err(error) = apply_result {
            let _ = store.restore(before);
            let _ = self.restore_app_file_metadata(path);
            if let Some(previous_runtime) = previous_runtime {
                let _ = self.apply_config(&previous_runtime);
                if core_was_running {
                    let _ = self.stop_core();
                    let _ = self.start_core();
                }
            }
            return Err(format!("apply refreshed active profile: {error}"));
        }
        Ok(())
    }

    fn auto_update_loop(self) {
        let mut attempts = HashMap::<String, u64>::new();
        thread::sleep(Duration::from_secs(30));
        loop {
            if !app_process_running(self.peer_uid)
                && let Some(path) = &self.profiles_path
                && let Ok(store) = Store::open(path.clone())
            {
                let now = unix_time();
                let due = store
                    .document()
                    .profiles
                    .into_iter()
                    .filter(|profile| profile.source.is_some() && profile.option.auto_update_enabled())
                    .filter(|profile| {
                        profile.option.update_interval.is_some_and(|minutes| {
                            minutes > 0 && profile.updated_at.saturating_add(minutes.saturating_mul(60)) <= now
                        })
                    })
                    .filter(|profile| {
                        attempts
                            .get(&profile.id)
                            .is_none_or(|last| now.saturating_sub(*last) >= 600)
                    })
                    .map(|profile| profile.id)
                    .collect::<Vec<_>>();
                for id in due {
                    attempts.insert(id.clone(), now);
                    match self.refresh_profile(&id) {
                        Ok(()) => {
                            attempts.remove(&id);
                            eprintln!("automatic subscription update completed for {id}");
                        }
                        Err(error) => eprintln!("automatic subscription update failed for {id}: {error}"),
                    }
                    if app_process_running(self.peer_uid) {
                        break;
                    }
                }
            }
            thread::sleep(Duration::from_secs(60));
        }
    }

    fn stop_core(&self) -> Result<(), String> {
        self.clear_vpn_coexistence_rules();
        let Some(pid) = self.core_pid() else {
            self.clear_core_bypass_rules();
            let _ = fs::remove_file(&self.controller_socket);
            let _ = fs::remove_file(self.launch_config());
            return Ok(());
        };
        if unsafe { libc::kill(pid, libc::SIGTERM) } != 0 {
            return Err(format!("SIGTERM mihomo {pid}: {}", std::io::Error::last_os_error()));
        }
        let deadline = Instant::now() + Duration::from_secs(4);
        while Instant::now() < deadline {
            if !process_alive(pid) {
                self.clear_core_bypass_rules();
                let _ = fs::remove_file(self.pid_file());
                let _ = fs::remove_file(&self.controller_socket);
                let _ = fs::remove_file(self.launch_config());
                return Ok(());
            }
            thread::sleep(Duration::from_millis(50));
        }
        Err(format!("mihomo {pid} did not exit after SIGTERM"))
    }

    fn read_log(&self, lines: usize) -> Result<String, String> {
        let lines = lines.clamp(1, 500);
        let path = self.log_file();
        let mut file = match fs::File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
            Err(error) => return Err(format!("open {}: {error}", path.display())),
        };
        let len = file.metadata().map_err(|e| e.to_string())?.len();
        const MAX_BYTES: u64 = 256 * 1024;
        if len > MAX_BYTES {
            file.seek(SeekFrom::Start(len - MAX_BYTES)).map_err(|e| e.to_string())?;
        }
        let mut text = String::new();
        file.read_to_string(&mut text).map_err(|e| e.to_string())?;
        let selected = text.lines().rev().take(lines).collect::<Vec<_>>();
        Ok(selected.into_iter().rev().collect::<Vec<_>>().join("\n"))
    }

    fn clear_log(&self) -> Result<(), String> {
        self.prepare()?;
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(self.log_file())
            .map_err(|e| format!("clear {}: {e}", self.log_file().display()))?;
        Ok(())
    }
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    file.write_all(bytes)
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    file.sync_all().map_err(|e| format!("sync {}: {e}", path.display()))?;
    Ok(())
}

fn process_alive(pid: i32) -> bool {
    if unsafe { libc::kill(pid, 0) } == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

fn is_usable_version(version: &str) -> bool {
    version.starts_with(|c: char| c.is_ascii_alphanumeric())
        && version.len() <= 64
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

fn read_core_version(path: &Path) -> Result<String, String> {
    let output = Command::new(path)
        .arg("-v")
        .output()
        .map_err(|e| format!("run {} -v: {e}", path.display()))?;
    if !output.status.success() {
        return Err(format!("{} -v exited with {}", path.display(), output.status));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let version = stdout
        .split_whitespace()
        .nth(2)
        .ok_or_else(|| format!("unexpected Mihomo version output: {stdout}"))?
        .to_owned();
    if !is_usable_version(&version) {
        return Err(format!("unusable Mihomo version: {version:?}"));
    }
    Ok(version)
}

fn process_matches(pid: i32, expected: &Path) -> bool {
    if !process_alive(pid) {
        return false;
    }
    let Ok(actual) = fs::read_link(format!("/proc/{pid}/exe")) else {
        return false;
    };
    let expected = fs::canonicalize(expected).unwrap_or_else(|_| expected.to_path_buf());
    actual == expected
}

fn chown_path(path: &Path, uid: u32, gid: u32) -> Result<(), String> {
    let path_c = CString::new(path.as_os_str().as_bytes()).map_err(|_| "Path contains NUL")?;
    if unsafe { libc::chown(path_c.as_ptr(), uid, gid) } != 0 {
        return Err(format!("chown {}: {}", path.display(), std::io::Error::last_os_error()));
    }
    Ok(())
}

fn relabel_path(path: &Path, context: &str) -> Result<(), String> {
    if context.len() > 255 || !context.starts_with("u:object_r:") {
        return Err("Invalid SELinux socket context".into());
    }
    let path_c = CString::new(path.as_os_str().as_bytes()).map_err(|_| "Path contains NUL")?;
    let mut value = context.as_bytes().to_vec();
    value.push(0);
    let name = c"security.selinux";
    if unsafe { libc::setxattr(path_c.as_ptr(), name.as_ptr(), value.as_ptr().cast(), value.len(), 0) } != 0 {
        return Err(format!(
            "set SELinux context on {}: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

fn peer_uid(stream: &UnixStream) -> Result<u32, String> {
    let mut cred = libc::ucred { pid: 0, uid: 0, gid: 0 };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut cred as *mut libc::ucred).cast(),
            &mut len,
        )
    };
    if rc != 0 {
        return Err(format!("SO_PEERCRED: {}", std::io::Error::last_os_error()));
    }
    Ok(cred.uid)
}

fn response(ok: bool, error: Option<String>, status: Option<Status>, log: Option<String>) -> Response {
    Response { ok, error, status, log }
}

fn handle(stream: UnixStream, runtime: &Runtime, expected_uid: u32) -> Result<bool, String> {
    let uid = peer_uid(&stream)?;
    if uid != expected_uid && uid != 0 {
        return Err(format!("Denied peer uid {uid}"));
    }
    stream
        .set_read_timeout(Some(Duration::from_secs(45)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(45)))
        .map_err(|e| e.to_string())?;
    let reader_stream = stream.try_clone().map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(reader_stream);
    let mut writer = BufWriter::new(stream);
    let mut line = String::new();
    let bytes = reader.read_line(&mut line).map_err(|e| e.to_string())?;
    if bytes == 0 || bytes > MAX_REQUEST_BYTES {
        return Err("Invalid request size".into());
    }
    let request: Request = serde_json::from_str(&line).map_err(|e| format!("Invalid request: {e}"))?;
    let (result, shutdown) = match request {
        Request::Status => (Ok(None), false),
        Request::ReadCoreVersion => (runtime.read_core_version().map(Some), false),
        Request::ApplyConfig { yaml } => (runtime.apply_config(&yaml).map(|_| None), false),
        Request::UpgradeCore {
            staged_path,
            expected_version,
        } => (
            runtime
                .upgrade_core(Path::new(&staged_path), &expected_version)
                .map(|_| None),
            false,
        ),
        Request::StartCore => (runtime.start_core().map(|_| None), false),
        Request::StopCore => (runtime.stop_core().map(|_| None), false),
        Request::RestartCore => (
            runtime.stop_core().and_then(|_| runtime.start_core()).map(|_| None),
            false,
        ),
        Request::SetTransparent { enable } => (runtime.set_transparent(enable).map(|_| None), false),
        Request::SetVpnCoexistence { enable } => (runtime.set_vpn_coexistence(enable).map(|_| None), false),
        Request::ReadLog { lines } => (runtime.read_log(lines).map(Some), false),
        Request::ClearLog => (runtime.clear_log().map(|_| None), false),
        Request::Shutdown => (runtime.stop_core().map(|_| None), true),
    };
    let reply = match result {
        Ok(log) => response(true, None, Some(runtime.status()), log),
        Err(error) => response(false, Some(error), Some(runtime.status()), None),
    };
    serde_json::to_writer(&mut writer, &reply).map_err(|e| e.to_string())?;
    writer.write_all(b"\n").map_err(|e| e.to_string())?;
    writer.flush().map_err(|e| e.to_string())?;
    Ok(shutdown && reply.ok)
}

fn percent_encode_path_segment(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'.' | b'_' | b'~') {
            output.push(char::from(*byte));
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

fn parse_args() -> Result<
    (
        PathBuf,
        u32,
        PathBuf,
        Option<String>,
        bool,
        Option<PathBuf>,
        Option<PathBuf>,
        Option<PathBuf>,
        Option<PathBuf>,
    ),
    String,
> {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("serve") {
        return Err(
            "Usage: cv4a-root-agent serve --socket PATH --peer-uid UID [--root-dir PATH] [--socket-context CONTEXT] [--profiles PATH] [--runtime-preferences PATH] [--backup-settings PATH] [--backup-dir PATH] [--start-core]"
                .into(),
        );
    }
    let mut socket = None;
    let mut peer = None;
    let mut root = PathBuf::from(DEFAULT_ROOT);
    let mut socket_context = None;
    let mut start_core = false;
    let mut profiles_path = None;
    let mut runtime_overrides_path = None;
    let mut backup_settings_path = None;
    let mut backup_dir = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--socket" => socket = args.next().map(PathBuf::from),
            "--peer-uid" => {
                peer = Some(
                    args.next()
                        .ok_or("Missing --peer-uid value")?
                        .parse::<u32>()
                        .map_err(|_| "Invalid --peer-uid")?,
                )
            }
            "--root-dir" => root = PathBuf::from(args.next().ok_or("Missing --root-dir value")?),
            "--socket-context" => socket_context = Some(args.next().ok_or("Missing --socket-context value")?),
            "--profiles" => profiles_path = Some(PathBuf::from(args.next().ok_or("Missing --profiles value")?)),
            "--runtime-preferences" => {
                runtime_overrides_path = Some(PathBuf::from(args.next().ok_or("Missing --runtime-preferences value")?))
            }
            "--backup-settings" => {
                backup_settings_path = Some(PathBuf::from(args.next().ok_or("Missing --backup-settings value")?))
            }
            "--backup-dir" => backup_dir = Some(PathBuf::from(args.next().ok_or("Missing --backup-dir value")?)),
            "--start-core" => start_core = true,
            other => return Err(format!("Unknown argument: {other}")),
        }
    }
    Ok((
        socket.ok_or("Missing --socket")?,
        peer.ok_or("Missing --peer-uid")?,
        root,
        socket_context,
        start_core,
        profiles_path,
        runtime_overrides_path,
        backup_settings_path,
        backup_dir,
    ))
}

fn run() -> Result<(), String> {
    let (
        socket,
        expected_uid,
        root,
        socket_context,
        start_core,
        profiles_path,
        runtime_overrides_path,
        backup_settings_path,
        backup_dir,
    ) = parse_args()?;
    if unsafe { libc::geteuid() } != 0 {
        return Err("cv4a-root-agent must run as root".into());
    }
    let controller_socket = socket.with_file_name("mihomo.sock");
    let runtime = Runtime::new(
        root,
        controller_socket,
        expected_uid,
        socket_context,
        profiles_path,
        runtime_overrides_path,
        backup_settings_path,
        backup_dir,
    )?;
    runtime.prepare()?;
    if let Some(parent) = socket.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create socket directory: {e}"))?;
    }
    let _ = fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket).map_err(|e| format!("bind {}: {e}", socket.display()))?;
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).map_err(|e| format!("chmod socket: {e}"))?;
    chown_path(&socket, expected_uid, expected_uid)?;
    if let Some(context) = &runtime.socket_context {
        relabel_path(&socket, context)?;
    }
    if start_core {
        runtime.start_core()?;
    }
    if runtime.profiles_path.is_some() {
        let updater = runtime.clone();
        thread::spawn(move || updater.auto_update_loop());
    }
    if runtime.backup_settings_path.is_some() && runtime.backup_dir.is_some() {
        let backup = runtime.clone();
        thread::spawn(move || backup.auto_backup_loop());
    }
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => match handle(stream, &runtime, expected_uid) {
                Ok(true) => break,
                Ok(false) => {}
                Err(error) => eprintln!("request error: {error}"),
            },
            Err(error) => eprintln!("accept error: {error}"),
        }
    }
    let _ = fs::remove_file(socket);
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("cv4a-root-agent: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparent_status_requires_cv4a_route_markers() {
        assert!(cv4a_routing_active(
            "8900:\tfrom all iif Mihomo goto 8910\n8901:\tnot from all iif lo lookup 3022\n",
            "default dev Mihomo\n"
        ));
        assert!(!cv4a_routing_active(
            "9000:\tfrom all iif Mihomo goto 9010\n9001:\tnot from all iif lo lookup 2022\n",
            "default dev Mihomo\n"
        ));
        assert!(!cv4a_routing_active(
            "8900:\tfrom all iif Mihomo goto 8910\n",
            "default dev tun0\n"
        ));
    }

    #[test]
    fn coexistence_status_requires_both_android_vpn_bypass_rules() {
        assert!(coexistence_rules_active(
            "8888:\tfrom all iif tun0 goto 10000\n8889:\tfrom all iif lo goto 10000\n"
        ));
        assert!(!coexistence_rules_active("8888:\tfrom all iif tun0 goto 10000\n"));
        assert!(!coexistence_rules_active("8889:\tfrom all iif lo goto 10000\n"));
    }

    #[test]
    fn proxy_group_names_are_encoded_as_url_path_segments() {
        assert_eq!(percent_encode_path_segment("GLOBAL"), "GLOBAL");
        assert_eq!(percent_encode_path_segment("München Node/1"), "M%C3%BCnchen%20Node%2F1");
    }
}
