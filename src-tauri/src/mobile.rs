use crate::android_vpn::{AndroidVpn, AndroidVpnStart, AndroidVpnStatus};
use clash_verge_mobile::{
    Capabilities, Document, MAX_PROFILE_BYTES, ProfileExtra, ProfileOptions, RuntimeEnhancements, RuntimeOverrides,
    Store, Summary,
};
use cv4a_mihomo_client::Snapshot as CoreSnapshot;
use flate2::read::GzDecoder;
use percent_encoding::percent_decode_str;
use reqwest_dav::list_cmd::{ListEntity, ListMultiStatus};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashMap,
    ffi::CString,
    fs,
    io::{Read, Write},
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    os::unix::{ffi::OsStrExt, net::UnixListener},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager, State};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const ROOT_RUNTIME: &str = "/data/adb/clash-verge4android";
const PACKAGED_MIHOMO_VERSION: &str = "v1.19.31";
const MIHOMO_RELEASE_VERSION_URL: &str = "https://github.com/MetaCubeX/mihomo/releases/latest/download/version.txt";
const MIHOMO_RELEASE_DOWNLOAD_BASE: &str = "https://github.com/MetaCubeX/mihomo/releases/download";
const MIHOMO_ALPHA_VERSION_URL: &str =
    "https://github.com/MetaCubeX/mihomo/releases/download/Prerelease-Alpha/version.txt";
const MIHOMO_ALPHA_DOWNLOAD_BASE: &str = "https://github.com/MetaCubeX/mihomo/releases/download/Prerelease-Alpha";
const APP_RELEASES_URL: &str = "https://api.github.com/repos/AloofBuckle/clash-verge4android/releases?per_page=30";
const APP_RELEASE_ASSET_PREFIX: &str = "https://github.com/AloofBuckle/clash-verge4android/releases/download/";
const MAX_CORE_PACKAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_CORE_BINARY_BYTES: u64 = 128 * 1024 * 1024;
const MAX_APP_PACKAGE_BYTES: u64 = 256 * 1024 * 1024;
const MOBILE_BACKUP_FORMAT_VERSION: u32 = 2;
const MAX_MOBILE_BACKUP_BYTES: u64 = 16 * 1024 * 1024;
const WEBDAV_BACKUP_DIR: &str = "cv4a-backups";
const AUTO_BACKUP_KEEP: usize = 20;
const KSU_MODULE_DIR: &str = "/data/adb/modules/cv4a";
const KSU_MODULE_PROP: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../packaging/kernelsu/cv4a/module.prop"
));
const KSU_SERVICE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../packaging/kernelsu/cv4a/service.sh"
));
const KSU_UNINSTALL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../packaging/kernelsu/cv4a/uninstall.sh"
));

#[cfg(target_arch = "aarch64")]
const ROOT_AGENT_GZ_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../.local-artifacts/runtime/cv4a-root-agent-arm64.gz"
));
#[cfg(target_arch = "aarch64")]
const MIHOMO_GZ_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../.local-artifacts/runtime/mihomo-android-v1.19.31.gz"
));
#[cfg(target_arch = "aarch64")]
const GEOSITE_GZ_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../.local-artifacts/runtime/geosite.dat.gz"
));
#[cfg(not(target_arch = "aarch64"))]
const ROOT_AGENT_GZ_BYTES: &[u8] = &[];
#[cfg(not(target_arch = "aarch64"))]
const MIHOMO_GZ_BYTES: &[u8] = &[];
#[cfg(not(target_arch = "aarch64"))]
const GEOSITE_GZ_BYTES: &[u8] = &[];

struct MobileState {
    store: Mutex<Store>,
    runtime_overrides: Mutex<RuntimeOverrides>,
    runtime_overrides_path: PathBuf,
    mobile_preferences: Mutex<MobilePreferences>,
    mobile_preferences_path: PathBuf,
    backup_settings: Mutex<BackupSettings>,
    backup_settings_path: PathBuf,
    client: reqwest::Client,
    auto_update_attempts: Mutex<HashMap<String, u64>>,
    app_update_progress: Mutex<AppUpdateProgress>,
    app_data_dir: PathBuf,
    agent_socket: PathBuf,
    controller_socket: PathBuf,
    socket_context: Option<String>,
    app_selinux_type: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
struct MobilePreferences {
    auto_close_connection: bool,
    auto_check_update: bool,
    default_latency_test: String,
    default_latency_timeout: u64,
    enable_auto_delay_detection: bool,
    auto_delay_detection_interval_minutes: u64,
    start_page: String,
    language: String,
    theme_mode: String,
    home_cards: MobileHomeCards,
    core_variant: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
struct MobileHomeCards {
    profile: bool,
    proxy: bool,
    network: bool,
    mode: bool,
    traffic: bool,
    clash_info: bool,
    system_info: bool,
    tests: bool,
    ip: bool,
}

impl Default for MobileHomeCards {
    fn default() -> Self {
        Self {
            profile: true,
            proxy: true,
            network: true,
            mode: true,
            traffic: true,
            clash_info: true,
            system_info: true,
            tests: true,
            ip: true,
        }
    }
}

impl Default for MobilePreferences {
    fn default() -> Self {
        Self {
            auto_close_connection: true,
            auto_check_update: true,
            default_latency_test: "http://cp.cloudflare.com/generate_204".into(),
            default_latency_timeout: 10_000,
            enable_auto_delay_detection: false,
            auto_delay_detection_interval_minutes: 5,
            start_page: "home".into(),
            language: "zh".into(),
            theme_mode: "system".into(),
            home_cards: MobileHomeCards::default(),
            core_variant: "stable".into(),
        }
    }
}

impl MobilePreferences {
    fn normalized(mut self) -> Result<Self, String> {
        self.default_latency_test = self.default_latency_test.trim().to_owned();
        if self.default_latency_test.is_empty() {
            self.default_latency_test = "http://cp.cloudflare.com/generate_204".into();
        }
        let url =
            reqwest::Url::parse(&self.default_latency_test).map_err(|e| format!("Invalid latency test URL: {e}"))?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err("Latency test URL must be HTTP(S)".into());
        }
        if !(100..=120_000).contains(&self.default_latency_timeout) {
            return Err("Latency timeout must be between 100 and 120000 ms".into());
        }
        if !(1..=1440).contains(&self.auto_delay_detection_interval_minutes) {
            return Err("Automatic latency interval must be between 1 and 1440 minutes".into());
        }
        if !matches!(
            self.start_page.as_str(),
            "home" | "proxies" | "profiles" | "connections" | "rules" | "logs" | "settings"
        ) {
            return Err("Invalid mobile start page".into());
        }
        self.language = self.language.trim().to_ascii_lowercase().replace('_', "-");
        if self.language == "zh-cn" {
            self.language = "zh".into();
        } else if self.language == "zh-tw" {
            self.language = "zhtw".into();
        }
        if !matches!(
            self.language.as_str(),
            "en" | "ru" | "zh" | "fa" | "tt" | "id" | "ar" | "ko" | "tr" | "de" | "es" | "jp" | "zhtw"
        ) {
            return Err("Invalid mobile language".into());
        }
        if !matches!(self.theme_mode.as_str(), "light" | "dark" | "system") {
            return Err("Invalid mobile theme mode".into());
        }
        self.core_variant = self.core_variant.trim().to_ascii_lowercase();
        if !matches!(self.core_variant.as_str(), "stable" | "alpha") {
            return Err("Invalid Mihomo core variant".into());
        }
        Ok(self)
    }
}

fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

fn next_profile_update_at(profile: &clash_verge_mobile::Profile) -> Option<u64> {
    if profile.source.is_none() || !profile.option.auto_update_enabled() {
        return None;
    }
    let interval = profile.option.update_interval?;
    if interval == 0 {
        return None;
    }
    Some(profile.updated_at.saturating_add(interval.saturating_mul(60)))
}

fn due_profile_ids(state: &MobileState) -> Result<Vec<String>, String> {
    let now = unix_time();
    let document = state.store.lock().map_err(|e| e.to_string())?.document();
    let attempts = state.auto_update_attempts.lock().map_err(|e| e.to_string())?;
    Ok(document
        .profiles
        .iter()
        .filter(|profile| next_profile_update_at(profile).is_some_and(|at| at <= now))
        .filter(|profile| {
            attempts
                .get(&profile.id)
                .is_none_or(|attempted| now.saturating_sub(*attempted) >= 600)
        })
        .map(|profile| profile.id.clone())
        .collect())
}

fn build_http_client(
    timeout_seconds: u64,
    proxy: Option<&str>,
    accept_invalid_certs: bool,
) -> Result<reqwest::Client, String> {
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let tls = rustls::ClientConfig::builder_with_provider(rustls::crypto::aws_lc_rs::default_provider().into())
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_root_certificates(roots)
        .with_no_client_auth();
    let mut builder = reqwest::Client::builder()
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

fn usable_core_version(version: &str) -> bool {
    version.starts_with(|c: char| c.is_ascii_alphanumeric())
        && version.len() <= 64
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

fn release_version_tuple(version: &str) -> Option<(u64, u64, u64)> {
    let raw = version.strip_prefix('v')?;
    let mut parts = raw.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

fn should_install_packaged_core(installed: Option<&str>) -> bool {
    let Some(installed) = installed else {
        return true;
    };
    match (
        release_version_tuple(installed),
        release_version_tuple(PACKAGED_MIHOMO_VERSION),
    ) {
        (Some(installed), Some(packaged)) => installed < packaged,
        // Preserve valid non-release variants or future version formats rather
        // than silently replacing them with the APK-bundled release.
        _ => false,
    }
}

fn persist_runtime_overrides(path: &Path, value: &RuntimeOverrides) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    write_mode(path, &bytes, 0o600)
}

fn persist_mobile_preferences(path: &Path, value: &MobilePreferences) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    write_mode(path, &bytes, 0o600)
}

fn active_profile(state: &MobileState) -> Result<clash_verge_mobile::Profile, String> {
    let document = state.store.lock().map_err(|e| e.to_string())?.document();
    let active = document.active_id.ok_or("No active profile")?;
    document
        .profiles
        .into_iter()
        .find(|profile| profile.id == active)
        .ok_or_else(|| "Active profile is missing".into())
}

fn profile_runtime_overrides(profile: &clash_verge_mobile::Profile, overrides: &RuntimeOverrides) -> RuntimeOverrides {
    let mut effective = overrides.clone();
    if profile.dns_override_enabled.is_some() || profile.dns_override_yaml.is_some() {
        effective.dns_override_enabled = profile.dns_override_enabled;
        effective.dns_override_yaml = profile.dns_override_yaml.clone();
    }
    effective
}

fn render_runtime_config(
    state: &MobileState,
    source: &str,
    overrides: &RuntimeOverrides,
    transparent: bool,
) -> Result<String, String> {
    let document = state.store.lock().map_err(|e| e.to_string())?.document();
    let active_id = document.active_id.as_deref().ok_or("No active profile")?;
    let profile = document
        .profiles
        .iter()
        .find(|profile| profile.id == active_id)
        .ok_or("Active profile is missing")?;
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
    let effective_overrides = profile_runtime_overrides(profile, overrides);
    if transparent {
        clash_verge_mobile::runtime_tun_config_with_enhancements(
            source,
            &effective_overrides,
            enhancements,
            profile.proxy_chain.as_ref(),
        )
    } else {
        clash_verge_mobile::runtime_preview_config_with_enhancements(
            source,
            &effective_overrides,
            enhancements,
            profile.proxy_chain.as_ref(),
        )
    }
}

fn render_android_vpn_config(state: &MobileState) -> Result<(String, HashMap<String, String>), String> {
    let document = state.store.lock().map_err(|e| e.to_string())?.document();
    let active_id = document.active_id.as_deref().ok_or("No active profile")?;
    let profile = document
        .profiles
        .iter()
        .find(|profile| profile.id == active_id)
        .ok_or("Active profile is missing")?;
    let overrides = state.runtime_overrides.lock().map_err(|e| e.to_string())?.clone();
    let effective_overrides = profile_runtime_overrides(profile, &overrides);
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
    let yaml = clash_verge_mobile::runtime_android_vpn_config_with_enhancements(
        &profile.yaml,
        &effective_overrides,
        enhancements,
        profile.proxy_chain.as_ref(),
    )?;
    let selected = profile
        .selected
        .iter()
        .map(|selection| (selection.group.clone(), selection.member.clone()))
        .collect();
    Ok((yaml, selected))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentStatus {
    version: String,
    agent_pid: u32,
    core_running: bool,
    core_pid: Option<i32>,
    core_installed: bool,
    config_present: bool,
    geosite_present: bool,
    controller_socket: String,
    transparent_active: bool,
    #[serde(default)]
    vpn_coexistence_active: bool,
}

#[derive(Debug, Deserialize)]
struct AgentReply {
    ok: bool,
    error: Option<String>,
    status: Option<AgentStatus>,
    log: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeStatus {
    packaged: bool,
    agent_connected: bool,
    core_running: bool,
    core_connected: bool,
    transparent_active: bool,
    agent: Option<AgentStatus>,
    detail: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BootModuleStatus {
    installed: bool,
    detail: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CoreUpgradeReport {
    upgraded: bool,
    from: String,
    to: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CoreVariantStatus {
    variant: String,
    version: String,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppUpdateProgress {
    active: bool,
    downloaded: u64,
    total: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppUpdateInfo {
    current_version: String,
    version: String,
    tag_name: String,
    body: String,
    html_url: String,
    asset_url: String,
    asset_name: String,
    asset_size: u64,
    published_at: String,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    body: Option<String>,
    html_url: String,
    draft: bool,
    published_at: Option<String>,
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MobileBackup {
    format_version: u32,
    created_at: u64,
    profiles: Document,
    runtime_overrides: RuntimeOverrides,
    #[serde(default)]
    mobile_preferences: Option<MobilePreferences>,
    #[serde(default)]
    backup_preferences: Option<BackupPreferencesSnapshot>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct BackupPreferencesSnapshot {
    auto_schedule_enabled: bool,
    auto_interval_hours: u64,
    auto_on_change: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalBackupInfo {
    filename: String,
    created_at: u64,
    profile_count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
struct BackupSettings {
    webdav_url: String,
    webdav_username: String,
    webdav_password: String,
    webdav_accept_invalid_certs: bool,
    auto_schedule_enabled: bool,
    auto_interval_hours: u64,
    auto_on_change: bool,
    last_auto_backup_at: Option<u64>,
}

impl Default for BackupSettings {
    fn default() -> Self {
        Self {
            webdav_url: String::new(),
            webdav_username: String::new(),
            webdav_password: String::new(),
            webdav_accept_invalid_certs: false,
            auto_schedule_enabled: false,
            auto_interval_hours: 24,
            auto_on_change: true,
            last_auto_backup_at: None,
        }
    }
}

impl BackupSettings {
    fn normalized(mut self) -> Result<Self, String> {
        self.webdav_url = self.webdav_url.trim().trim_end_matches('/').to_owned();
        self.webdav_username = self.webdav_username.trim().to_owned();
        if !self.webdav_url.is_empty() {
            let url = reqwest::Url::parse(&self.webdav_url).map_err(|e| format!("Invalid WebDAV URL: {e}"))?;
            if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() || url.username() != "" {
                return Err("WebDAV URL must be an HTTP(S) URL without embedded credentials".into());
            }
            if self.webdav_username.is_empty() || self.webdav_password.is_empty() {
                return Err("WebDAV username and password are required when WebDAV is configured".into());
            }
        }
        if !(1..=168).contains(&self.auto_interval_hours) {
            return Err("Automatic backup interval must be between 1 and 168 hours".into());
        }
        Ok(self)
    }

    fn webdav_configured(&self) -> bool {
        !self.webdav_url.is_empty() && !self.webdav_username.is_empty() && !self.webdav_password.is_empty()
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebDavBackupInfo {
    filename: String,
    last_modified: String,
    content_length: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DnsOverrideSettings {
    enabled: bool,
    yaml: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortSettings {
    allow_lan: bool,
    bind_address: String,
    mixed_port: u16,
    http_port: u16,
    socks_port: u16,
    redir_port: u16,
    tproxy_port: u16,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExternalControllerSettings {
    enabled: bool,
    address: String,
    secret: String,
    allow_private_network: bool,
    allow_origins: Vec<String>,
}

fn backup_dir(state: &MobileState) -> PathBuf {
    state.app_data_dir.join("backups")
}

fn persist_backup_settings(path: &Path, settings: &BackupSettings) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
    write_mode(path, &bytes, 0o600)
}

fn webdav_client(settings: &BackupSettings, timeout_seconds: u64) -> Result<reqwest_dav::Client, String> {
    if !settings.webdav_configured() {
        return Err("WebDAV is not configured".into());
    }
    let agent = build_http_client(timeout_seconds, None, settings.webdav_accept_invalid_certs)?;
    reqwest_dav::ClientBuilder::new()
        .set_agent(agent)
        .set_host(settings.webdav_url.clone())
        .set_auth(reqwest_dav::Auth::Basic(
            settings.webdav_username.clone(),
            settings.webdav_password.clone(),
        ))
        .build()
        .map_err(|e| e.to_string())
}

fn webdav_dir_already_exists(error: &reqwest_dav::Error) -> bool {
    match error {
        reqwest_dav::Error::Decode(reqwest_dav::DecodeError::Server(server)) => {
            server.response_code == 405
                || server.message.to_ascii_lowercase().contains("already exist")
                || server.message.to_ascii_lowercase().contains("already taken")
        }
        reqwest_dav::Error::Decode(reqwest_dav::DecodeError::StatusMismatched(status)) => status.response_code == 405,
        reqwest_dav::Error::Reqwest(error) => error.status().is_some_and(|status| status.as_u16() == 405),
        _ => false,
    }
}

async fn ensure_webdav_backup_dir(client: &reqwest_dav::Client) -> Result<(), String> {
    match client.mkcol(WEBDAV_BACKUP_DIR).await {
        Ok(()) => Ok(()),
        Err(error) if webdav_dir_already_exists(&error) => Ok(()),
        Err(error) => Err(format!("Create WebDAV backup directory: {error}")),
    }
}

fn parse_webdav_backup_list(xml: &str) -> Result<Vec<WebDavBackupInfo>, String> {
    let normalized = xml.replace(" +0000</", " GMT</");
    let multi: ListMultiStatus = reqwest_dav::re_exports::serde_xml_rs::from_str(&normalized)
        .map_err(|e| format!("Parse WebDAV listing: {e}"))?;
    let mut files = Vec::new();
    for response in multi.responses {
        let entity = ListEntity::try_from(response).map_err(|e| e.to_string())?;
        let ListEntity::File(file) = entity else {
            continue;
        };
        let raw_name = file.href.trim_end_matches('/').rsplit('/').next().unwrap_or_default();
        let filename = percent_decode_str(raw_name)
            .decode_utf8()
            .map_err(|_| "WebDAV backup filename is not UTF-8".to_owned())?
            .into_owned();
        if validate_backup_name(&filename).is_err() {
            continue;
        }
        files.push(WebDavBackupInfo {
            filename,
            last_modified: file.last_modified.to_rfc3339(),
            content_length: u64::try_from(file.content_length).unwrap_or(0),
        });
    }
    files.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
    Ok(files)
}

fn validate_backup_name(filename: &str) -> Result<(), String> {
    if filename.is_empty() || filename.contains('/') || filename.contains('\\') || !filename.ends_with(".json") {
        return Err("Invalid backup filename".into());
    }
    Ok(())
}

fn validate_mobile_backup(backup: &MobileBackup) -> Result<(), String> {
    if !(1..=MOBILE_BACKUP_FORMAT_VERSION).contains(&backup.format_version) {
        return Err(format!("Unsupported backup format version {}", backup.format_version));
    }
    if let Some(preferences) = backup.mobile_preferences.clone() {
        preferences.normalized()?;
    }
    if let Some(settings) = backup.backup_preferences.as_ref()
        && !(1..=168).contains(&settings.auto_interval_hours)
    {
        return Err("Automatic backup interval must be between 1 and 168 hours".into());
    }
    if let Some(active_id) = backup.profiles.active_id.as_ref()
        && !backup.profiles.profiles.iter().any(|profile| &profile.id == active_id)
    {
        return Err("Backup active profile is missing".into());
    }
    for profile in &backup.profiles.profiles {
        clash_verge_mobile::inspect(&profile.yaml)?;
        profile.option.clone().normalized()?;
    }
    if let Some(active_id) = backup.profiles.active_id.as_ref()
        && let Some(active) = backup.profiles.profiles.iter().find(|profile| &profile.id == active_id)
    {
        clash_verge_mobile::runtime_tun_config_with_state(
            &active.yaml,
            &backup.runtime_overrides,
            active.proxy_chain.as_ref(),
        )?;
    }
    Ok(())
}

fn read_mobile_backup(state: &MobileState, filename: &str) -> Result<MobileBackup, String> {
    validate_backup_name(filename)?;
    let path = backup_dir(state).join(filename);
    let metadata = fs::metadata(&path).map_err(|e| format!("read backup metadata: {e}"))?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_MOBILE_BACKUP_BYTES {
        return Err("Backup file has an invalid size or type".into());
    }
    let bytes = fs::read(&path).map_err(|e| format!("read backup: {e}"))?;
    let backup = serde_json::from_slice::<MobileBackup>(&bytes).map_err(|e| format!("parse backup: {e}"))?;
    validate_mobile_backup(&backup)?;
    Ok(backup)
}

#[tauri::command]
fn mobile_capabilities() -> Capabilities {
    clash_verge_mobile::capabilities()
}

#[tauri::command]
fn mobile_profiles(state: State<'_, MobileState>) -> Result<Document, String> {
    Ok(state.store.lock().map_err(|e| e.to_string())?.document())
}

#[tauri::command]
fn mobile_inspect(yaml: String) -> Result<Summary, String> {
    clash_verge_mobile::inspect(&yaml)
}

#[tauri::command]
fn mobile_import(name: String, yaml: String, state: State<'_, MobileState>) -> Result<Document, String> {
    state.store.lock().map_err(|e| e.to_string())?.import(name, yaml, None)
}

#[tauri::command]
fn mobile_import_nodes(name: String, links: String, state: State<'_, MobileState>) -> Result<Document, String> {
    let yaml = clash_verge_mobile::parse_links(&links)?;
    state.store.lock().map_err(|e| e.to_string())?.import(name, yaml, None)
}

#[tauri::command]
async fn mobile_activate(id: String, state: State<'_, MobileState>) -> Result<Document, String> {
    let before = state.store.lock().map_err(|e| e.to_string())?.document();
    let previous_id = before.active_id.clone();
    let previous_yaml = previous_id.as_ref().and_then(|active| {
        before
            .profiles
            .iter()
            .find(|profile| &profile.id == active)
            .map(|profile| profile.yaml.clone())
    });
    let (was_running, was_transparent) = running_core_state(&state).await;
    let document = state.store.lock().map_err(|e| e.to_string())?.activate(&id)?;
    if let Err(error) = reload_running_core(&state).await {
        if let Some(previous_id) = previous_id {
            let _ = state.store.lock().map_err(|e| e.to_string())?.activate(&previous_id);
        }
        if was_running {
            if let Some(previous_yaml) = previous_yaml {
                let _ = start_source_core(&state, &previous_yaml, was_transparent).await;
            }
        }
        return Err(format!(
            "Failed to apply selected profile; previous profile restored: {error}"
        ));
    }
    maybe_auto_backup_on_change(&state, "merge");
    Ok(document)
}

#[tauri::command]
fn mobile_delete(id: String, state: State<'_, MobileState>) -> Result<Document, String> {
    state.store.lock().map_err(|e| e.to_string())?.remove(&id)
}

#[tauri::command]
async fn mobile_delete_many(ids: Vec<String>, state: State<'_, MobileState>) -> Result<Document, String> {
    if ids.is_empty() {
        return Ok(state.store.lock().map_err(|e| e.to_string())?.document());
    }
    let before = state.store.lock().map_err(|e| e.to_string())?.document();
    let deleting_active = before
        .active_id
        .as_ref()
        .is_some_and(|active| ids.iter().any(|id| id == active));
    let previous_yaml = before.active_id.as_ref().and_then(|active| {
        before
            .profiles
            .iter()
            .find(|profile| &profile.id == active)
            .map(|profile| profile.yaml.clone())
    });
    let (was_running, was_transparent) = if deleting_active {
        running_core_state(&state).await
    } else {
        (false, false)
    };
    let document = state.store.lock().map_err(|e| e.to_string())?.remove_many(&ids)?;

    if deleting_active && was_running {
        let runtime_result = if let Some(active_id) = document.active_id.as_ref() {
            let source = document
                .profiles
                .iter()
                .find(|profile| &profile.id == active_id)
                .map(|profile| profile.yaml.clone())
                .ok_or("Selected replacement profile is missing")?;
            async {
                start_source_core(&state, &source, was_transparent).await?;
                if was_transparent {
                    wait_for_tun_stable(&state).await?;
                }
                Ok::<(), String>(())
            }
            .await
        } else {
            call_agent(&state.agent_socket, json!({ "op": "stop_core" }))
                .await
                .map(|_| ())
        };

        if let Err(error) = runtime_result {
            let _ = state.store.lock().map_err(|e| e.to_string())?.restore(before.clone());
            if let Some(previous_yaml) = previous_yaml {
                let _ = start_source_core(&state, &previous_yaml, was_transparent).await;
                if was_transparent {
                    let _ = wait_for_tun_stable(&state).await;
                }
            }
            return Err(format!(
                "Batch delete could not activate the replacement profile; original profiles restored: {error}"
            ));
        }
    }
    Ok(document)
}

#[tauri::command]
async fn mobile_update_profile(
    id: String,
    name: String,
    description: Option<String>,
    yaml: String,
    expected_yaml: String,
    source: Option<String>,
    option: Option<ProfileOptions>,
    state: State<'_, MobileState>,
) -> Result<Document, String> {
    let before = state.store.lock().map_err(|e| e.to_string())?.document();
    let profile = before
        .profiles
        .iter()
        .find(|profile| profile.id == id)
        .cloned()
        .ok_or("Profile not found")?;
    let is_active = before.active_id.as_deref() == Some(id.as_str());
    let yaml_changed = profile.yaml != yaml;
    let (was_running, was_transparent) = if is_active && yaml_changed {
        running_core_state(&state).await
    } else {
        (false, false)
    };
    let document = state.store.lock().map_err(|e| e.to_string())?.update_with_metadata(
        &id,
        name,
        yaml.clone(),
        source.or_else(|| profile.source.clone()),
        option.unwrap_or_else(|| profile.option.clone()),
        description,
        &expected_yaml,
    )?;
    if is_active
        && yaml_changed
        && let Err(error) = reload_running_core(&state).await
    {
        let rollback = state.store.lock().map_err(|e| e.to_string())?.update_with_metadata(
            &id,
            profile.name.clone(),
            profile.yaml.clone(),
            profile.source.clone(),
            profile.option.clone(),
            profile.description.clone(),
            &yaml,
        );
        if was_running {
            let _ = start_source_core(&state, &profile.yaml, was_transparent).await;
        }
        return match rollback {
            Ok(_) => Err(format!(
                "Edited profile could not be applied; previous profile restored: {error}"
            )),
            Err(rollback_error) => Err(format!(
                "Edited profile could not be applied ({error}); profile rollback also failed: {rollback_error}"
            )),
        };
    }
    Ok(document)
}

#[tauri::command]
fn mobile_reorder_profile(
    active_id: String,
    over_id: String,
    state: State<'_, MobileState>,
) -> Result<Document, String> {
    state
        .store
        .lock()
        .map_err(|e| e.to_string())?
        .reorder(&active_id, &over_id)
}

#[tauri::command]
async fn mobile_set_global_merge(yaml: Option<String>, state: State<'_, MobileState>) -> Result<Document, String> {
    let before = state.store.lock().map_err(|e| e.to_string())?.document();
    let (was_running, was_transparent) = running_core_state(&state).await;
    let document = state.store.lock().map_err(|e| e.to_string())?.set_global_merge(yaml)?;
    if let Err(error) = reload_running_core(&state).await {
        let _ = state.store.lock().map_err(|e| e.to_string())?.restore(before.clone());
        if was_running && before.active_id.is_some() {
            let _ = start_active_core(&state, was_transparent).await;
            if was_transparent {
                let _ = wait_for_tun_stable(&state).await;
            }
        }
        return Err(format!(
            "Global Merge could not be applied; previous Merge restored: {error}"
        ));
    }
    Ok(document)
}

#[tauri::command]
async fn mobile_set_profile_merge(
    id: String,
    yaml: Option<String>,
    state: State<'_, MobileState>,
) -> Result<Document, String> {
    let before = state.store.lock().map_err(|e| e.to_string())?.document();
    let is_active = before.active_id.as_deref() == Some(id.as_str());
    let (was_running, was_transparent) = if is_active {
        running_core_state(&state).await
    } else {
        (false, false)
    };
    let document = state
        .store
        .lock()
        .map_err(|e| e.to_string())?
        .set_profile_merge(&id, yaml)?;
    if is_active && let Err(error) = reload_running_core(&state).await {
        let _ = state.store.lock().map_err(|e| e.to_string())?.restore(before.clone());
        if was_running {
            let _ = start_active_core(&state, was_transparent).await;
            if was_transparent {
                let _ = wait_for_tun_stable(&state).await;
            }
        }
        return Err(format!(
            "Profile Merge could not be applied; previous Merge restored: {error}"
        ));
    }
    maybe_auto_backup_on_change(&state, "merge");
    Ok(document)
}

#[tauri::command]
async fn mobile_set_profile_sequence(
    id: String,
    kind: String,
    yaml: Option<String>,
    state: State<'_, MobileState>,
) -> Result<Document, String> {
    if !matches!(kind.as_str(), "rules" | "proxies" | "groups") {
        return Err("Unknown sequence enhancement kind".into());
    }
    let before = state.store.lock().map_err(|e| e.to_string())?.document();
    let is_active = before.active_id.as_deref() == Some(id.as_str());
    let (was_running, was_transparent) = if is_active {
        running_core_state(&state).await
    } else {
        (false, false)
    };
    let document = state
        .store
        .lock()
        .map_err(|e| e.to_string())?
        .set_profile_sequence(&id, &kind, yaml)?;
    if is_active && let Err(error) = reload_running_core(&state).await {
        let _ = state.store.lock().map_err(|e| e.to_string())?.restore(before.clone());
        if was_running {
            let _ = start_active_core(&state, was_transparent).await;
            if was_transparent {
                let _ = wait_for_tun_stable(&state).await;
            }
        }
        return Err(format!(
            "{kind} enhancement could not be applied; previous enhancement restored: {error}"
        ));
    }
    Ok(document)
}

#[tauri::command]
async fn mobile_set_global_script(script: Option<String>, state: State<'_, MobileState>) -> Result<Document, String> {
    let before = state.store.lock().map_err(|e| e.to_string())?.document();
    let (was_running, was_transparent) = running_core_state(&state).await;
    let document = state
        .store
        .lock()
        .map_err(|e| e.to_string())?
        .set_global_script(script)?;
    if let Err(error) = reload_running_core(&state).await {
        let _ = state.store.lock().map_err(|e| e.to_string())?.restore(before.clone());
        if was_running && before.active_id.is_some() {
            let _ = start_active_core(&state, was_transparent).await;
            if was_transparent {
                let _ = wait_for_tun_stable(&state).await;
            }
        }
        return Err(format!(
            "Global Script could not be applied; previous Script restored: {error}"
        ));
    }
    maybe_auto_backup_on_change(&state, "script");
    Ok(document)
}

#[tauri::command]
async fn mobile_set_profile_script(
    id: String,
    script: Option<String>,
    state: State<'_, MobileState>,
) -> Result<Document, String> {
    let before = state.store.lock().map_err(|e| e.to_string())?.document();
    let is_active = before.active_id.as_deref() == Some(id.as_str());
    let (was_running, was_transparent) = if is_active {
        running_core_state(&state).await
    } else {
        (false, false)
    };
    let document = state
        .store
        .lock()
        .map_err(|e| e.to_string())?
        .set_profile_script(&id, script)?;
    if is_active && let Err(error) = reload_running_core(&state).await {
        let _ = state.store.lock().map_err(|e| e.to_string())?.restore(before.clone());
        if was_running {
            let _ = start_active_core(&state, was_transparent).await;
            if was_transparent {
                let _ = wait_for_tun_stable(&state).await;
            }
        }
        return Err(format!(
            "Profile Script could not be applied; previous Script restored: {error}"
        ));
    }
    maybe_auto_backup_on_change(&state, "script");
    Ok(document)
}

#[derive(Clone, Debug)]
struct DownloadedSubscription {
    name: String,
    yaml: String,
    extra: Option<ProfileExtra>,
    home: Option<String>,
    update_interval: Option<u64>,
}

fn parse_subscription_userinfo(value: &str) -> ProfileExtra {
    let mut extra = ProfileExtra::default();
    for part in value.split(';') {
        let Some((key, raw)) = part.trim().split_once('=') else {
            continue;
        };
        let Ok(number) = raw.trim().parse::<u64>() else {
            continue;
        };
        match key.trim().to_ascii_lowercase().as_str() {
            "upload" => extra.upload = number,
            "download" => extra.download = number,
            "total" => extra.total = number,
            "expire" => extra.expire = number,
            _ => {}
        }
    }
    extra
}

fn normalize_profile_home_url(value: &str) -> Option<String> {
    let url = reqwest::Url::parse(value.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return None;
    }
    Some(url.to_string())
}

fn subscription_name_from_response(headers: &reqwest::header::HeaderMap, url: &reqwest::Url) -> String {
    let from_disposition = headers
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            let parts = value.split(';').map(str::trim).collect::<Vec<_>>();
            ["filename*", "filename"].into_iter().find_map(|wanted| {
                parts.iter().find_map(|part| {
                    let (key, raw) = part.split_once('=')?;
                    if !key.trim().eq_ignore_ascii_case(wanted) {
                        return None;
                    }
                    let mut raw = raw.trim().trim_matches('"');
                    if wanted == "filename*" {
                        raw = raw.split_once("''").map(|(_, value)| value).unwrap_or(raw);
                    }
                    percent_decode_str(raw)
                        .decode_utf8()
                        .ok()
                        .map(|value| value.trim().to_owned())
                        .filter(|value| !value.is_empty())
                })
            })
        });
    if let Some(name) = from_disposition {
        return name;
    }

    url.path_segments()
        .and_then(|segments| segments.filter(|segment| !segment.is_empty()).next_back())
        .and_then(|segment| percent_decode_str(segment).decode_utf8().ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Remote File".into())
}

async fn download(
    client: &reqwest::Client,
    source: &str,
    user_agent: Option<&str>,
) -> Result<DownloadedSubscription, String> {
    let url = reqwest::Url::parse(source).map_err(|e| e.to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Use an HTTP(S) subscription URL without userinfo".into());
    }
    let mut response = client
        .get(url.clone())
        .header("User-Agent", user_agent.unwrap_or("ClashVerge4Android/0.1.0"))
        .send()
        .await
        .map_err(|e| format!("Subscription download failed: {}", e.without_url()))?
        .error_for_status()
        .map_err(|e| format!("Subscription HTTP error: {}", e.without_url()))?;
    let headers = response.headers().clone();
    let name = subscription_name_from_response(&headers, &url);
    let extra = headers.iter().find_map(|(name, value)| {
        let key = name.as_str().to_ascii_lowercase();
        key.strip_suffix("subscription-userinfo")
            .filter(|prefix| prefix.is_empty() || prefix.ends_with('-'))
            .and_then(|_| value.to_str().ok())
            .map(parse_subscription_userinfo)
    });
    let home = headers
        .get("profile-web-page-url")
        .and_then(|value| value.to_str().ok())
        .and_then(normalize_profile_home_url);
    let update_interval = headers
        .get("profile-update-interval")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .and_then(|hours| hours.checked_mul(60));
    if response
        .content_length()
        .is_some_and(|len| len > MAX_PROFILE_BYTES as u64)
    {
        return Err("Subscription exceeds 4 MiB".into());
    }
    let mut data = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if data.len() + chunk.len() > MAX_PROFILE_BYTES {
            return Err("Subscription exceeds 4 MiB".into());
        }
        data.extend_from_slice(&chunk);
    }
    let yaml = String::from_utf8(data).map_err(|_| "Subscription is not UTF-8".to_owned())?;
    clash_verge_mobile::inspect(&yaml)?;
    Ok(DownloadedSubscription {
        name,
        yaml,
        extra,
        home,
        update_interval,
    })
}

async fn download_subscription(
    state: &MobileState,
    source: &str,
    option: &ProfileOptions,
) -> Result<DownloadedSubscription, String> {
    let option = option.clone().normalized()?;
    if option.with_proxy.unwrap_or(false) {
        return Err("Android system-proxy subscription updates are not enabled in the Root TUN backend".into());
    }

    let timeout = option.timeout_seconds.unwrap_or(25);
    if !option.self_proxy.unwrap_or(false) {
        if timeout == 25 && !option.danger_accept_invalid_certs.unwrap_or(false) {
            return download(&state.client, source, option.user_agent.as_deref()).await;
        }
        let client = build_http_client(timeout, None, option.danger_accept_invalid_certs.unwrap_or(false))?;
        return download(&client, source, option.user_agent.as_deref()).await;
    }

    let controller = core_client(state)?;
    let previous = controller.mixed_port_settings().await?;
    let listener =
        TcpListener::bind(("127.0.0.1", 0)).map_err(|e| format!("reserve local subscription proxy port: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("read local subscription proxy port: {e}"))?
        .port();
    drop(listener);
    let temporary = cv4a_mihomo_client::MixedPortSettings {
        mixed_port: port,
        allow_lan: false,
        bind_address: "127.0.0.1".into(),
    };
    controller.patch_mixed_port_settings(&temporary).await?;

    let result = async {
        let proxy = format!("http://127.0.0.1:{port}");
        let client = build_http_client(
            timeout,
            Some(&proxy),
            option.danger_accept_invalid_certs.unwrap_or(false),
        )?;
        download(&client, source, option.user_agent.as_deref()).await
    }
    .await;
    let restore = controller.patch_mixed_port_settings(&previous).await;
    match (result, restore) {
        (Ok(downloaded), Ok(())) => Ok(downloaded),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(format!(
            "Subscription downloaded, but the temporary Clash proxy listener could not be restored: {error}"
        )),
        (Err(error), Err(restore_error)) => Err(format!(
            "{error}; temporary Clash proxy listener restore also failed: {restore_error}"
        )),
    }
}

async fn download_bytes_limited(client: &reqwest::Client, url: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
    let url = reqwest::Url::parse(url).map_err(|e| e.to_string())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("Core update URL must use HTTP(S)".into());
    }
    let mut response = client
        .get(url)
        .header("User-Agent", "ClashVerge4Android/0.1.0")
        .send()
        .await
        .map_err(|e| format!("Core update download failed: {}", e.without_url()))?
        .error_for_status()
        .map_err(|e| format!("Core update HTTP error: {}", e.without_url()))?;
    if response.content_length().is_some_and(|len| len > max_bytes as u64) {
        return Err(format!("Core update response exceeds {} MiB", max_bytes / 1024 / 1024));
    }
    let mut data = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if data.len() + chunk.len() > max_bytes {
            return Err(format!("Core update response exceeds {} MiB", max_bytes / 1024 / 1024));
        }
        data.extend_from_slice(&chunk);
    }
    Ok(data)
}

async fn download_core_resource(
    state: &MobileState,
    url: &str,
    max_bytes: usize,
    timeout_seconds: u64,
) -> Result<Vec<u8>, String> {
    let direct = build_http_client(timeout_seconds, None, false)?;
    match download_bytes_limited(&direct, url, max_bytes).await {
        Ok(bytes) => Ok(bytes),
        Err(direct_error) => {
            let controller = core_client(state).map_err(|_| direct_error.clone())?;
            let previous = controller
                .mixed_port_settings()
                .await
                .map_err(|_| direct_error.clone())?;
            let listener = TcpListener::bind(("127.0.0.1", 0))
                .map_err(|e| format!("{direct_error}; reserve fallback proxy port: {e}"))?;
            let port = listener
                .local_addr()
                .map_err(|e| format!("{direct_error}; read fallback proxy port: {e}"))?
                .port();
            drop(listener);
            controller
                .patch_mixed_port_settings(&cv4a_mihomo_client::MixedPortSettings {
                    mixed_port: port,
                    allow_lan: false,
                    bind_address: "127.0.0.1".into(),
                })
                .await
                .map_err(|e| format!("{direct_error}; enable Clash fallback proxy: {e}"))?;
            let result = async {
                let proxy = format!("http://127.0.0.1:{port}");
                let proxied = build_http_client(timeout_seconds, Some(&proxy), false)?;
                download_bytes_limited(&proxied, url, max_bytes).await
            }
            .await;
            let restore = controller.patch_mixed_port_settings(&previous).await;
            match (result, restore) {
                (Ok(bytes), Ok(())) => Ok(bytes),
                (Err(proxy_error), Ok(())) => Err(format!(
                    "Direct core download failed ({direct_error}); Clash proxy retry also failed: {proxy_error}"
                )),
                (Ok(_), Err(error)) => Err(format!(
                    "Core downloaded through Clash, but temporary proxy settings could not be restored: {error}"
                )),
                (Err(proxy_error), Err(restore_error)) => Err(format!(
                    "Direct core download failed ({direct_error}); Clash proxy retry failed ({proxy_error}); proxy restore also failed: {restore_error}"
                )),
            }
        }
    }
}

fn stage_online_core(app_data_dir: &Path, package: &[u8]) -> Result<PathBuf, String> {
    let stage_dir = app_data_dir.join("runtime-stage");
    fs::create_dir_all(&stage_dir).map_err(|e| format!("create runtime stage: {e}"))?;
    let staged = stage_dir.join("mihomo-online-upgrade");
    let temporary = stage_dir.join("mihomo-online-upgrade.next");
    let _ = fs::remove_file(&temporary);
    let _ = fs::remove_file(&staged);
    let mut output = fs::File::create(&temporary).map_err(|e| format!("create staged online core: {e}"))?;
    let decoder = GzDecoder::new(package);
    let mut limited = decoder.take(MAX_CORE_BINARY_BYTES + 1);
    let written =
        std::io::copy(&mut limited, &mut output).map_err(|e| format!("decompress online Mihomo package: {e}"))?;
    if written == 0 || written > MAX_CORE_BINARY_BYTES {
        let _ = fs::remove_file(&temporary);
        return Err("Decompressed Mihomo core has an invalid size".into());
    }
    output.sync_all().map_err(|e| format!("sync staged online core: {e}"))?;
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("chmod staged online core: {e}"))?;
    fs::rename(&temporary, &staged).map_err(|e| format!("publish staged online core: {e}"))?;
    Ok(staged)
}

#[tauri::command]
async fn mobile_subscribe(url: String, state: State<'_, MobileState>) -> Result<Document, String> {
    let source = url.trim();
    if source.is_empty() {
        return Err("Subscription URL is required".into());
    }

    let mut option = ProfileOptions::default().normalized()?;
    let downloaded = match download_subscription(&state, source, &option).await {
        Ok(downloaded) => downloaded,
        Err(direct_error) => {
            let mut retry_option = option.clone();
            retry_option.self_proxy = Some(true);
            match download_subscription(&state, source, &retry_option).await {
                Ok(downloaded) => {
                    option = retry_option;
                    downloaded
                }
                Err(_) => return Err(direct_error),
            }
        }
    };
    if option.update_interval.is_none() {
        option.update_interval = downloaded.update_interval;
    }
    state.store.lock().map_err(|e| e.to_string())?.import_with_metadata(
        downloaded.name,
        downloaded.yaml,
        Some(source.to_owned()),
        option,
        None,
        downloaded.home,
        downloaded.extra,
    )
}

async fn refresh_profile_internal(state: &MobileState, id: &str) -> Result<Document, String> {
    let profile = state
        .store
        .lock()
        .map_err(|e| e.to_string())?
        .document()
        .profiles
        .into_iter()
        .find(|p| p.id == id)
        .ok_or("Profile not found")?;
    let url = profile
        .source
        .clone()
        .ok_or("This is a local configuration, not a subscription")?;
    let downloaded = download_subscription(state, &url, &profile.option).await?;
    let yaml = downloaded.yaml;
    let new_yaml = yaml.clone();
    let before = state.store.lock().map_err(|e| e.to_string())?.document();
    let is_active = before.active_id.as_deref() == Some(id);
    let (was_running, was_transparent) = if is_active {
        running_core_state(state).await
    } else {
        (false, false)
    };
    let document = state.store.lock().map_err(|e| e.to_string())?.replace_with_metadata(
        id,
        yaml,
        &profile.yaml,
        downloaded.home,
        downloaded.extra,
    )?;
    if is_active && let Err(error) = reload_running_core(state).await {
        let rollback = state
            .store
            .lock()
            .map_err(|e| e.to_string())?
            .replace(id, profile.yaml.clone(), &new_yaml);
        if was_running {
            let _ = start_source_core(state, &profile.yaml, was_transparent).await;
        }
        return match rollback {
            Ok(_) => Err(format!(
                "Refreshed profile could not be applied; previous profile restored: {error}"
            )),
            Err(rollback_error) => Err(format!(
                "Refreshed profile could not be applied ({error}); profile rollback also failed: {rollback_error}"
            )),
        };
    }
    Ok(document)
}

async fn run_due_profile_updates(state: &MobileState) {
    let Ok(ids) = due_profile_ids(state) else {
        return;
    };
    for id in ids {
        let now = unix_time();
        if let Ok(mut attempts) = state.auto_update_attempts.lock() {
            attempts.insert(id.clone(), now);
        }
        match refresh_profile_internal(state, &id).await {
            Ok(_) => {
                if let Ok(mut attempts) = state.auto_update_attempts.lock() {
                    attempts.remove(&id);
                }
            }
            Err(error) => eprintln!("automatic subscription update failed for {id}: {error}"),
        }
    }
}

#[tauri::command]
async fn mobile_refresh(id: String, state: State<'_, MobileState>) -> Result<Document, String> {
    refresh_profile_internal(&state, &id).await
}

#[tauri::command]
fn mobile_next_update_time(id: String, state: State<'_, MobileState>) -> Result<Option<u64>, String> {
    let document = state.store.lock().map_err(|e| e.to_string())?.document();
    let profile = document
        .profiles
        .iter()
        .find(|profile| profile.id == id)
        .ok_or("Profile not found")?;
    Ok(next_profile_update_at(profile))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RootProbe {
    granted: bool,
    detail: String,
}

#[tauri::command]
async fn mobile_probe_root() -> Result<RootProbe, String> {
    let mut cmd = tokio::process::Command::new("su");
    cmd.args(["-c", "id; printf '\\nselinux='; cat /proc/self/attr/current"])
        .kill_on_drop(true);
    let result = tokio::time::timeout(Duration::from_secs(20), cmd.output()).await;
    match result {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            Ok(RootProbe {
                granted: output.status.success() && stdout.split_whitespace().any(|s| s.starts_with("uid=0(")),
                detail: if output.status.success() {
                    stdout.chars().take(500).collect()
                } else {
                    "Root permission was denied or su is unavailable".into()
                },
            })
        }
        Ok(Err(error)) => Ok(RootProbe {
            granted: false,
            detail: error.to_string(),
        }),
        Err(_) => Ok(RootProbe {
            granted: false,
            detail: "Root authorization timed out; retry after reviewing KernelSU".into(),
        }),
    }
}

fn shell_quote(path: &Path) -> String {
    shell_quote_str(&path.to_string_lossy())
}

fn shell_quote_str(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn selinux_type(context: &str) -> Option<String> {
    let value = context.trim().split(':').nth(2)?;
    if value.is_empty() || !value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some(value.to_owned())
}

fn current_selinux_type() -> Option<String> {
    let context = fs::read_to_string("/proc/self/attr/current").ok()?;
    selinux_type(&context)
}

fn write_mode(path: &Path, bytes: &[u8], mode: u32) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let mut file = fs::File::create(path).map_err(|e| format!("create {}: {e}", path.display()))?;
    file.write_all(bytes)
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    file.sync_all().map_err(|e| format!("sync {}: {e}", path.display()))?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|e| format!("chmod {}: {e}", path.display()))?;
    Ok(())
}

fn write_gzip_mode(path: &Path, package: &[u8], mode: u32) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let mut decoder = GzDecoder::new(package);
    let mut output = fs::File::create(path).map_err(|e| format!("create {}: {e}", path.display()))?;
    std::io::copy(&mut decoder, &mut output).map_err(|e| format!("decompress {}: {e}", path.display()))?;
    output.sync_all().map_err(|e| format!("sync {}: {e}", path.display()))?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|e| format!("chmod {}: {e}", path.display()))?;
    Ok(())
}

fn selinux_context(path: &Path) -> Option<String> {
    let path_c = CString::new(path.as_os_str().as_bytes()).ok()?;
    let name = c"security.selinux";
    let size = unsafe { libc::lgetxattr(path_c.as_ptr(), name.as_ptr(), std::ptr::null_mut(), 0) };
    if size <= 0 || size > 512 {
        return None;
    }
    let mut buffer = vec![0_u8; size as usize];
    let read = unsafe { libc::lgetxattr(path_c.as_ptr(), name.as_ptr(), buffer.as_mut_ptr().cast(), buffer.len()) };
    if read <= 0 {
        return None;
    }
    buffer.truncate(read as usize);
    while buffer.last() == Some(&0) {
        buffer.pop();
    }
    String::from_utf8(buffer).ok()
}

fn probe_socket_context(run_dir: &Path) -> Option<String> {
    fs::create_dir_all(run_dir).ok()?;
    let probe = run_dir.join("context-probe.sock");
    let _ = fs::remove_file(&probe);
    let listener = UnixListener::bind(&probe).ok()?;
    let context = selinux_context(&probe);
    drop(listener);
    let _ = fs::remove_file(probe);
    context
}

fn stage_runtime(app_data_dir: &Path) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    if ROOT_AGENT_GZ_BYTES.is_empty() || MIHOMO_GZ_BYTES.is_empty() || GEOSITE_GZ_BYTES.is_empty() {
        return Err("Root runtime is currently packaged only for arm64-v8a".into());
    }
    let stage = app_data_dir.join("runtime-stage");
    fs::create_dir_all(&stage).map_err(|e| format!("create runtime stage: {e}"))?;
    let agent = stage.join("cv4a-root-agent");
    write_gzip_mode(&agent, ROOT_AGENT_GZ_BYTES, 0o700)?;
    let core = stage.join("mihomo");
    write_gzip_mode(&core, MIHOMO_GZ_BYTES, 0o700)?;
    let geosite = stage.join("GeoSite.dat");
    write_gzip_mode(&geosite, GEOSITE_GZ_BYTES, 0o600)?;
    Ok((agent, core, geosite))
}

async fn call_agent(socket: &Path, request: serde_json::Value) -> Result<AgentReply, String> {
    call_agent_with_timeout(socket, request, Duration::from_secs(5)).await
}

async fn call_agent_with_timeout(
    socket: &Path,
    request: serde_json::Value,
    timeout: Duration,
) -> Result<AgentReply, String> {
    let mut stream = tokio::net::UnixStream::connect(socket)
        .await
        .map_err(|e| format!("connect root-agent: {e}"))?;
    let mut payload = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
    payload.push(b'\n');
    stream
        .write_all(&payload)
        .await
        .map_err(|e| format!("write root-agent request: {e}"))?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    tokio::time::timeout(timeout, reader.read_line(&mut line))
        .await
        .map_err(|_| "root-agent response timed out".to_owned())?
        .map_err(|e| format!("read root-agent response: {e}"))?;
    if line.len() > 128 * 1024 {
        return Err("root-agent response is too large".into());
    }
    let reply: AgentReply = serde_json::from_str(&line).map_err(|e| format!("invalid root-agent response: {e}"))?;
    if reply.ok {
        Ok(reply)
    } else {
        Err(reply.error.unwrap_or_else(|| "root-agent request failed".into()))
    }
}

async fn core_connected(socket: &Path) -> bool {
    if !socket.exists() {
        return false;
    }
    let Ok(client) = reqwest::Client::builder()
        .unix_socket(socket)
        .timeout(Duration::from_secs(2))
        .no_proxy()
        .build()
    else {
        return false;
    };
    client
        .get("http://localhost/version")
        .send()
        .await
        .is_ok_and(|response| response.status().is_success())
}

async fn runtime_status(state: &MobileState) -> RuntimeStatus {
    let agent = call_agent(&state.agent_socket, json!({ "op": "status" })).await.ok();
    let status = agent.as_ref().and_then(|reply| reply.status.clone());
    let agent_connected = status.is_some();
    let core_running = status.as_ref().is_some_and(|value| value.core_running);
    let core_connected = core_running && core_connected(&state.controller_socket).await;
    let transparent_active = status.as_ref().is_some_and(|value| value.transparent_active);
    RuntimeStatus {
        packaged: !ROOT_AGENT_GZ_BYTES.is_empty() && !MIHOMO_GZ_BYTES.is_empty() && !GEOSITE_GZ_BYTES.is_empty(),
        agent_connected,
        core_running,
        core_connected,
        transparent_active,
        agent: status,
        detail: if core_connected {
            if transparent_active {
                "root-agent, Mihomo UDS and the transparent TUN interface are reachable".into()
            } else {
                "root-agent and Mihomo UDS are reachable".into()
            }
        } else if core_running {
            "root-agent reports Mihomo running, but the controller UDS is not reachable".into()
        } else if agent_connected {
            "root-agent is reachable; Mihomo is stopped".into()
        } else if state.agent_socket.exists() {
            "root-agent socket exists but is not reachable".into()
        } else {
            "root runtime is not installed or not running".into()
        },
    }
}

#[tauri::command]
async fn mobile_runtime_status(state: State<'_, MobileState>) -> Result<RuntimeStatus, String> {
    Ok(runtime_status(&state).await)
}

async fn run_su(command: &str, label: &str) -> Result<std::process::Output, String> {
    let mut process = tokio::process::Command::new("su");
    process.args(["-c", command]).kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(30), process.output())
        .await
        .map_err(|_| format!("{label} timed out"))?
        .map_err(|e| format!("start {label}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "{label} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output)
}

#[tauri::command]
async fn mobile_boot_module_status() -> Result<BootModuleStatus, String> {
    let command = format!(
        "if [ -f {module}/module.prop ]; then cat {module}/module.prop; fi",
        module = shell_quote(Path::new(KSU_MODULE_DIR)),
    );
    let output = run_su(&command, "KernelSU module check").await?;
    let text = String::from_utf8_lossy(&output.stdout);
    let installed = text.lines().any(|line| line.trim() == "id=cv4a");
    Ok(BootModuleStatus {
        installed,
        detail: if installed {
            text.lines()
                .find_map(|line| line.strip_prefix("version="))
                .unwrap_or_default()
                .to_owned()
        } else {
            String::new()
        },
    })
}

#[tauri::command]
async fn mobile_runtime_config() -> Result<String, String> {
    let path = Path::new(ROOT_RUNTIME).join("config/runtime.yaml");
    let command = format!("cat {}", shell_quote(&path));
    let output = run_su(&command, "read runtime config").await?;
    let text = String::from_utf8(output.stdout).map_err(|_| "Runtime config is not UTF-8")?;
    if text.len() > MAX_PROFILE_BYTES {
        return Err("Runtime config exceeds 4 MiB".into());
    }
    Ok(text)
}

fn parse_core_version_output(output: &str) -> Result<String, String> {
    let version = output
        .split_whitespace()
        .nth(2)
        .ok_or_else(|| format!("Unexpected Mihomo version output: {output}"))?
        .to_owned();
    if !usable_core_version(&version) {
        return Err(format!("Unusable Mihomo version: {version:?}"));
    }
    Ok(version)
}

async fn installed_core_version(state: &MobileState) -> Result<String, String> {
    if let Ok(reply) = call_agent(&state.agent_socket, json!({ "op": "read_core_version" })).await
        && let Some(version) = reply.log
    {
        return Ok(version);
    }
    let core = Path::new(ROOT_RUNTIME).join("bin/mihomo");
    let command = format!("{} -v", shell_quote(&core));
    let output = run_su(&command, "Mihomo version check").await?;
    parse_core_version_output(&String::from_utf8_lossy(&output.stdout))
}

async fn upgrade_core_variant(variant: &str, force: bool, state: &MobileState) -> Result<CoreUpgradeReport, String> {
    if !matches!(variant, "stable" | "alpha") {
        return Err("Invalid Mihomo core variant".into());
    }
    let installed = installed_core_version(&state).await.unwrap_or_default();
    let version_url = if variant == "alpha" {
        MIHOMO_ALPHA_VERSION_URL
    } else {
        MIHOMO_RELEASE_VERSION_URL
    };
    let version_bytes = download_core_resource(state, version_url, 4096, 20).await?;
    let latest = String::from_utf8(version_bytes)
        .map_err(|_| "Mihomo latest version response is not UTF-8".to_owned())?
        .trim()
        .to_owned();
    if !usable_core_version(&latest) {
        return Err(format!("Mihomo returned an unusable latest version: {latest:?}"));
    }
    if !force {
        if installed == latest {
            return Ok(CoreUpgradeReport {
                upgraded: false,
                from: installed,
                to: latest,
            });
        }
        if variant == "stable"
            && let (Some(current), Some(target)) = (release_version_tuple(&installed), release_version_tuple(&latest))
            && current > target
        {
            return Ok(CoreUpgradeReport {
                upgraded: false,
                from: installed,
                to: latest,
            });
        }
    }

    let url = if variant == "alpha" {
        format!("{MIHOMO_ALPHA_DOWNLOAD_BASE}/mihomo-android-arm64-v8-{latest}.gz")
    } else {
        format!("{MIHOMO_RELEASE_DOWNLOAD_BASE}/{latest}/mihomo-android-arm64-v8-{latest}.gz")
    };
    let package = download_core_resource(state, &url, MAX_CORE_PACKAGE_BYTES, 300).await?;
    let staged = stage_online_core(&state.app_data_dir, &package)?;
    let (was_running, was_transparent) = running_core_state(&state).await;
    let request = json!({
        "op": "upgrade_core",
        "staged_path": staged.to_string_lossy(),
        "expected_version": latest,
    });
    let upgrade = call_agent_with_timeout(&state.agent_socket, request, Duration::from_secs(45)).await;
    let _ = fs::remove_file(&staged);
    upgrade?;
    if was_running && was_transparent {
        wait_for_tun_stable(&state).await?;
    }
    let actual = installed_core_version(&state).await?;
    if actual != latest {
        return Err(format!(
            "Mihomo upgrade completed but installed version is {actual}, expected {latest}"
        ));
    }
    Ok(CoreUpgradeReport {
        upgraded: true,
        from: installed,
        to: latest,
    })
}

#[tauri::command]
async fn mobile_core_upgrade(force: bool, state: State<'_, MobileState>) -> Result<CoreUpgradeReport, String> {
    let variant = state
        .mobile_preferences
        .lock()
        .map_err(|e| e.to_string())?
        .core_variant
        .clone();
    upgrade_core_variant(&variant, force, &state).await
}

#[tauri::command]
async fn mobile_core_variant(state: State<'_, MobileState>) -> Result<CoreVariantStatus, String> {
    let variant = state
        .mobile_preferences
        .lock()
        .map_err(|e| e.to_string())?
        .core_variant
        .clone();
    Ok(CoreVariantStatus {
        variant,
        version: installed_core_version(&state).await.unwrap_or_default(),
    })
}

#[tauri::command]
async fn mobile_set_core_variant(variant: String, state: State<'_, MobileState>) -> Result<CoreVariantStatus, String> {
    let variant = variant.trim().to_ascii_lowercase();
    if !matches!(variant.as_str(), "stable" | "alpha") {
        return Err("Invalid Mihomo core variant".into());
    }
    let current = state
        .mobile_preferences
        .lock()
        .map_err(|e| e.to_string())?
        .core_variant
        .clone();
    if current != variant {
        upgrade_core_variant(&variant, true, &state).await?;
        let mut preferences = state.mobile_preferences.lock().map_err(|e| e.to_string())?.clone();
        preferences.core_variant = variant.clone();
        persist_mobile_preferences(&state.mobile_preferences_path, &preferences)?;
        *state.mobile_preferences.lock().map_err(|e| e.to_string())? = preferences;
    }
    Ok(CoreVariantStatus {
        variant,
        version: installed_core_version(&state).await.unwrap_or_default(),
    })
}

fn android_release_version(tag: &str) -> Option<Version> {
    Version::parse(tag.trim().strip_prefix("android-v")?).ok()
}

#[tauri::command]
fn mobile_app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
async fn mobile_check_app_update(
    app: AppHandle,
    state: State<'_, MobileState>,
) -> Result<Option<AppUpdateInfo>, String> {
    let current_version = app.package_info().version.to_string();
    let current = Version::parse(&current_version)
        .map_err(|e| format!("Invalid installed app version {current_version}: {e}"))?;
    let releases = state
        .client
        .get(APP_RELEASES_URL)
        .header("User-Agent", "ClashVerge4Android-Updater")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("App update check failed: {}", e.without_url()))?
        .error_for_status()
        .map_err(|e| format!("App update HTTP error: {}", e.without_url()))?
        .json::<Vec<GithubRelease>>()
        .await
        .map_err(|e| format!("Invalid app update response: {e}"))?;

    let mut best: Option<(Version, AppUpdateInfo)> = None;
    for release in releases {
        if release.draft {
            continue;
        }
        let Some(version) = android_release_version(&release.tag_name) else {
            continue;
        };
        if version <= current {
            continue;
        }
        let Some(asset) = release
            .assets
            .into_iter()
            .find(|asset| asset.name.to_ascii_lowercase().ends_with(".apk"))
        else {
            continue;
        };
        if asset.size == 0 || asset.size > MAX_APP_PACKAGE_BYTES {
            continue;
        }
        if !asset.browser_download_url.starts_with(APP_RELEASE_ASSET_PREFIX) {
            continue;
        }
        let info = AppUpdateInfo {
            current_version: current_version.clone(),
            version: version.to_string(),
            tag_name: release.tag_name,
            body: release.body.unwrap_or_default(),
            html_url: release.html_url,
            asset_url: asset.browser_download_url,
            asset_name: asset.name,
            asset_size: asset.size,
            published_at: release.published_at.unwrap_or_default(),
        };
        if best.as_ref().is_none_or(|(candidate, _)| version > *candidate) {
            best = Some((version, info));
        }
    }
    Ok(best.map(|(_, info)| info))
}

#[tauri::command]
fn mobile_app_update_progress(state: State<'_, MobileState>) -> Result<AppUpdateProgress, String> {
    Ok(state.app_update_progress.lock().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
async fn mobile_download_app_update(
    asset_url: String,
    expected_size: u64,
    app: AppHandle,
    state: State<'_, MobileState>,
) -> Result<(), String> {
    if !asset_url.starts_with(APP_RELEASE_ASSET_PREFIX) {
        return Err("Invalid app update asset URL".into());
    }
    if expected_size == 0 || expected_size > MAX_APP_PACKAGE_BYTES {
        return Err("Invalid app update package size".into());
    }
    {
        let mut progress = state.app_update_progress.lock().map_err(|e| e.to_string())?;
        if progress.active {
            return Err("App update download is already running".into());
        }
        *progress = AppUpdateProgress {
            active: true,
            downloaded: 0,
            total: expected_size,
        };
    }

    let result = async {
        let mut response = state
            .client
            .get(&asset_url)
            .header("User-Agent", "ClashVerge4Android-Updater")
            .send()
            .await
            .map_err(|e| format!("App update download failed: {}", e.without_url()))?
            .error_for_status()
            .map_err(|e| format!("App update download HTTP error: {}", e.without_url()))?;
        let total = response.content_length().unwrap_or(expected_size);
        if total == 0 || total > MAX_APP_PACKAGE_BYTES {
            return Err("App update package is too large".into());
        }
        let cache_dir = app.path().app_cache_dir().map_err(|e| e.to_string())?;
        fs::create_dir_all(&cache_dir).map_err(|e| format!("create app cache directory: {e}"))?;
        let final_path = cache_dir.join("cv4a-update.apk");
        let temporary = cache_dir.join("cv4a-update.apk.download");
        let _ = fs::remove_file(&temporary);
        let mut file = fs::File::create(&temporary).map_err(|e| format!("create app update package: {e}"))?;
        let mut downloaded = 0u64;
        while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
            downloaded = downloaded
                .checked_add(chunk.len() as u64)
                .ok_or("App update package size overflow")?;
            if downloaded > MAX_APP_PACKAGE_BYTES {
                let _ = fs::remove_file(&temporary);
                return Err("App update package is too large".into());
            }
            file.write_all(&chunk)
                .map_err(|e| format!("write app update package: {e}"))?;
            let mut progress = state.app_update_progress.lock().map_err(|e| e.to_string())?;
            progress.downloaded = downloaded;
            progress.total = total;
        }
        if downloaded == 0 || (expected_size > 0 && downloaded != expected_size) {
            let _ = fs::remove_file(&temporary);
            return Err(format!(
                "App update package size mismatch: downloaded {downloaded}, expected {expected_size}"
            ));
        }
        file.sync_all().map_err(|e| format!("sync app update package: {e}"))?;
        let _ = fs::remove_file(&final_path);
        fs::rename(&temporary, &final_path).map_err(|e| format!("publish app update package: {e}"))?;
        Ok::<(), String>(())
    }
    .await;

    if let Ok(mut progress) = state.app_update_progress.lock() {
        progress.active = false;
        if result.is_err() {
            progress.downloaded = 0;
            progress.total = 0;
        }
    }
    result
}

#[tauri::command]
fn mobile_install_app_update(app: AppHandle, vpn: State<'_, AndroidVpn>) -> Result<(), String> {
    let apk = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("cv4a-update.apk");
    if !apk.is_file() {
        return Err("Downloaded app update package is missing".into());
    }
    vpn.install_apk(apk.to_string_lossy().into_owned())
}

#[tauri::command]
fn mobile_open_external_url(url: String, vpn: State<'_, AndroidVpn>) -> Result<(), String> {
    let parsed = reqwest::Url::parse(url.trim()).map_err(|e| e.to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err("Invalid external URL".into());
    }
    vpn.open_url(parsed.to_string())
}

#[tauri::command]
fn mobile_create_local_backup(state: State<'_, MobileState>) -> Result<LocalBackupInfo, String> {
    create_local_backup_internal(&state, None)
}

fn snapshot_mobile_backup(state: &MobileState) -> Result<MobileBackup, String> {
    let profiles = state.store.lock().map_err(|e| e.to_string())?.document();
    let runtime_overrides = state.runtime_overrides.lock().map_err(|e| e.to_string())?.clone();
    let mobile_preferences = state.mobile_preferences.lock().map_err(|e| e.to_string())?.clone();
    let backup_settings = state.backup_settings.lock().map_err(|e| e.to_string())?.clone();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    let backup = MobileBackup {
        format_version: MOBILE_BACKUP_FORMAT_VERSION,
        created_at: now.as_secs(),
        profiles,
        runtime_overrides,
        mobile_preferences: Some(mobile_preferences),
        backup_preferences: Some(BackupPreferencesSnapshot {
            auto_schedule_enabled: backup_settings.auto_schedule_enabled,
            auto_interval_hours: backup_settings.auto_interval_hours,
            auto_on_change: backup_settings.auto_on_change,
        }),
    };
    validate_mobile_backup(&backup)?;
    Ok(backup)
}

fn create_local_backup_internal(state: &MobileState, suffix: Option<&str>) -> Result<LocalBackupInfo, String> {
    let backup = snapshot_mobile_backup(state)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    let dir = backup_dir(&state);
    fs::create_dir_all(&dir).map_err(|e| format!("create backup directory: {e}"))?;
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).map_err(|e| format!("chmod backup directory: {e}"))?;
    let suffix = suffix.map(|value| format!("-auto-{value}")).unwrap_or_default();
    let filename = format!("cv4a-backup-{}-{}{}.json", now.as_secs(), now.subsec_nanos(), suffix);
    let bytes = serde_json::to_vec_pretty(&backup).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_MOBILE_BACKUP_BYTES {
        return Err("Backup exceeds 16 MiB".into());
    }
    write_mode(&dir.join(&filename), &bytes, 0o600)?;
    Ok(LocalBackupInfo {
        filename,
        created_at: backup.created_at,
        profile_count: backup.profiles.profiles.len(),
    })
}

#[tauri::command]
fn mobile_list_local_backups(state: State<'_, MobileState>) -> Result<Vec<LocalBackupInfo>, String> {
    let dir = backup_dir(&state);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("read backup directory: {error}")),
    };
    let mut backups = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                eprintln!("skip unreadable backup entry: {error}");
                continue;
            }
        };
        let filename = entry.file_name().to_string_lossy().into_owned();
        if validate_backup_name(&filename).is_err() {
            continue;
        }
        match read_mobile_backup(&state, &filename) {
            Ok(backup) => backups.push(LocalBackupInfo {
                filename,
                created_at: backup.created_at,
                profile_count: backup.profiles.profiles.len(),
            }),
            Err(error) => eprintln!("skip invalid backup {filename}: {error}"),
        }
    }
    backups.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.filename.cmp(&a.filename))
    });
    Ok(backups)
}

#[tauri::command]
fn mobile_delete_local_backup(filename: String, state: State<'_, MobileState>) -> Result<(), String> {
    validate_backup_name(&filename)?;
    let path = backup_dir(&state).join(filename);
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("delete backup: {error}")),
    }
}

#[tauri::command]
fn mobile_export_local_backup(filename: String, state: State<'_, MobileState>) -> Result<String, String> {
    validate_backup_name(&filename)?;
    let path = backup_dir(&state).join(&filename);
    let metadata = fs::metadata(&path).map_err(|e| format!("read backup metadata: {e}"))?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_MOBILE_BACKUP_BYTES {
        return Err("Backup has an invalid size".into());
    }
    let bytes = fs::read(&path).map_err(|e| format!("read backup: {e}"))?;
    let backup = serde_json::from_slice::<MobileBackup>(&bytes).map_err(|e| format!("parse backup: {e}"))?;
    validate_mobile_backup(&backup)?;
    String::from_utf8(bytes).map_err(|_| "Backup is not UTF-8".into())
}

#[tauri::command]
fn mobile_import_backup_text(text: String, state: State<'_, MobileState>) -> Result<LocalBackupInfo, String> {
    if text.is_empty() || text.len() as u64 > MAX_MOBILE_BACKUP_BYTES {
        return Err("Backup has an invalid size".into());
    }
    let backup = serde_json::from_str::<MobileBackup>(&text).map_err(|e| format!("parse backup: {e}"))?;
    validate_mobile_backup(&backup)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    let dir = backup_dir(&state);
    fs::create_dir_all(&dir).map_err(|e| format!("create backup directory: {e}"))?;
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).map_err(|e| format!("chmod backup directory: {e}"))?;
    let filename = format!("cv4a-backup-{}-{}-import.json", now.as_secs(), now.subsec_nanos());
    let bytes = serde_json::to_vec_pretty(&backup).map_err(|e| e.to_string())?;
    write_mode(&dir.join(&filename), &bytes, 0o600)?;
    Ok(LocalBackupInfo {
        filename,
        created_at: backup.created_at,
        profile_count: backup.profiles.profiles.len(),
    })
}

#[tauri::command]
fn mobile_backup_settings(state: State<'_, MobileState>) -> Result<BackupSettings, String> {
    Ok(state.backup_settings.lock().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
fn mobile_set_backup_settings(
    settings: BackupSettings,
    state: State<'_, MobileState>,
) -> Result<BackupSettings, String> {
    let mut next = settings.normalized()?;
    let before = state.backup_settings.lock().map_err(|e| e.to_string())?.clone();
    if next.auto_schedule_enabled && (!before.auto_schedule_enabled || next.last_auto_backup_at.is_none()) {
        next.last_auto_backup_at = Some(unix_time());
    }
    persist_backup_settings(&state.backup_settings_path, &next)?;
    *state.backup_settings.lock().map_err(|e| e.to_string())? = next.clone();
    Ok(next)
}

async fn current_webdav_client(state: &MobileState, timeout: u64) -> Result<reqwest_dav::Client, String> {
    let settings = state.backup_settings.lock().map_err(|e| e.to_string())?.clone();
    let client = webdav_client(&settings, timeout)?;
    ensure_webdav_backup_dir(&client).await?;
    Ok(client)
}

#[tauri::command]
async fn mobile_test_webdav(state: State<'_, MobileState>) -> Result<(), String> {
    let client = current_webdav_client(&state, 30).await?;
    let path = format!("{WEBDAV_BACKUP_DIR}/");
    let response = client
        .list_raw(&path, reqwest_dav::Depth::Number(1))
        .await
        .map_err(|e| format!("WebDAV test failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("WebDAV test returned HTTP {}", response.status()));
    }
    Ok(())
}

#[tauri::command]
async fn mobile_create_webdav_backup(state: State<'_, MobileState>) -> Result<WebDavBackupInfo, String> {
    let backup = snapshot_mobile_backup(&state)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    let filename = format!("cv4a-backup-{}-{}.json", now.as_secs(), now.subsec_nanos());
    let bytes = serde_json::to_vec_pretty(&backup).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_MOBILE_BACKUP_BYTES {
        return Err("Backup exceeds 16 MiB".into());
    }
    let client = current_webdav_client(&state, 300).await?;
    let path = format!("{WEBDAV_BACKUP_DIR}/{filename}");
    client
        .put(&path, bytes.clone())
        .await
        .map_err(|e| format!("Upload WebDAV backup: {e}"))?;
    Ok(WebDavBackupInfo {
        filename,
        last_modified: chrono::Utc::now().to_rfc3339(),
        content_length: bytes.len() as u64,
    })
}

#[tauri::command]
async fn mobile_list_webdav_backups(state: State<'_, MobileState>) -> Result<Vec<WebDavBackupInfo>, String> {
    let client = current_webdav_client(&state, 30).await?;
    let path = format!("{WEBDAV_BACKUP_DIR}/");
    let response = client
        .list_raw(&path, reqwest_dav::Depth::Number(1))
        .await
        .map_err(|e| format!("List WebDAV backups: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("List WebDAV backups returned HTTP {}", response.status()));
    }
    let text = response.text().await.map_err(|e| e.to_string())?;
    parse_webdav_backup_list(&text)
}

#[tauri::command]
async fn mobile_delete_webdav_backup(filename: String, state: State<'_, MobileState>) -> Result<(), String> {
    validate_backup_name(&filename)?;
    let client = current_webdav_client(&state, 30).await?;
    let path = format!("{WEBDAV_BACKUP_DIR}/{filename}");
    client
        .delete(&path)
        .await
        .map_err(|e| format!("Delete WebDAV backup: {e}"))
}

#[tauri::command]
async fn mobile_restore_webdav_backup(filename: String, state: State<'_, MobileState>) -> Result<Document, String> {
    validate_backup_name(&filename)?;
    let client = current_webdav_client(&state, 300).await?;
    let path = format!("{WEBDAV_BACKUP_DIR}/{filename}");
    let response = client
        .get(&path)
        .await
        .map_err(|e| format!("Download WebDAV backup: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("Download WebDAV backup returned HTTP {}", response.status()));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_MOBILE_BACKUP_BYTES)
    {
        return Err("WebDAV backup exceeds 16 MiB".into());
    }
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_MOBILE_BACKUP_BYTES {
        return Err("WebDAV backup has an invalid size".into());
    }
    let backup = serde_json::from_slice::<MobileBackup>(&bytes).map_err(|e| format!("parse WebDAV backup: {e}"))?;
    apply_mobile_backup(&state, backup).await
}

fn cleanup_auto_backups(state: &MobileState) -> Result<(), String> {
    let dir = backup_dir(state);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("read backup directory: {error}")),
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
        fs::remove_file(&path).map_err(|e| format!("remove old automatic backup {}: {e}", path.display()))?;
    }
    Ok(())
}

fn record_auto_backup(state: &MobileState, trigger: &str) -> Result<LocalBackupInfo, String> {
    let backup = create_local_backup_internal(state, Some(trigger))?;
    cleanup_auto_backups(state)?;
    let mut settings = state.backup_settings.lock().map_err(|e| e.to_string())?.clone();
    settings.last_auto_backup_at = Some(unix_time());
    persist_backup_settings(&state.backup_settings_path, &settings)?;
    *state.backup_settings.lock().map_err(|e| e.to_string())? = settings;
    Ok(backup)
}

async fn run_due_auto_backup(state: &MobileState) {
    let settings = match state.backup_settings.lock() {
        Ok(settings) => settings.clone(),
        Err(_) => return,
    };
    if !settings.auto_schedule_enabled {
        return;
    }
    let interval = settings.auto_interval_hours.clamp(1, 168).saturating_mul(3600);
    let last = settings.last_auto_backup_at.unwrap_or_else(unix_time);
    if unix_time().saturating_sub(last) < interval {
        return;
    }
    if let Err(error) = record_auto_backup(state, "scheduled") {
        eprintln!("scheduled automatic backup failed: {error}");
    }
}

fn maybe_auto_backup_on_change(state: &MobileState, trigger: &str) {
    let enabled = state
        .backup_settings
        .lock()
        .map(|settings| settings.auto_on_change)
        .unwrap_or(false);
    if enabled && let Err(error) = record_auto_backup(state, trigger) {
        eprintln!("automatic backup after {trigger} change failed: {error}");
    }
}

#[tauri::command]
async fn mobile_restore_local_backup(filename: String, state: State<'_, MobileState>) -> Result<Document, String> {
    let backup = read_mobile_backup(&state, &filename)?;
    apply_mobile_backup(&state, backup).await
}

async fn apply_mobile_backup(state: &MobileState, backup: MobileBackup) -> Result<Document, String> {
    validate_mobile_backup(&backup)?;
    let before_profiles = state.store.lock().map_err(|e| e.to_string())?.document();
    let before_overrides = state.runtime_overrides.lock().map_err(|e| e.to_string())?.clone();
    let before_mobile_preferences = state.mobile_preferences.lock().map_err(|e| e.to_string())?.clone();
    let before_backup_settings = state.backup_settings.lock().map_err(|e| e.to_string())?.clone();
    let (was_running, was_transparent) = running_core_state(&state).await;

    state
        .store
        .lock()
        .map_err(|e| e.to_string())?
        .restore(backup.profiles.clone())?;
    if let Err(error) = persist_runtime_overrides(&state.runtime_overrides_path, &backup.runtime_overrides) {
        let _ = state
            .store
            .lock()
            .map_err(|e| e.to_string())?
            .restore(before_profiles.clone());
        return Err(format!("restore runtime preferences: {error}"));
    }
    *state.runtime_overrides.lock().map_err(|e| e.to_string())? = backup.runtime_overrides.clone();
    if let Some(preferences) = backup.mobile_preferences.clone() {
        let preferences = preferences.normalized()?;
        if let Err(error) = persist_mobile_preferences(&state.mobile_preferences_path, &preferences) {
            let _ = state
                .store
                .lock()
                .map_err(|e| e.to_string())?
                .restore(before_profiles.clone());
            let _ = persist_runtime_overrides(&state.runtime_overrides_path, &before_overrides);
            *state.runtime_overrides.lock().map_err(|e| e.to_string())? = before_overrides.clone();
            return Err(format!("restore mobile preferences: {error}"));
        }
        *state.mobile_preferences.lock().map_err(|e| e.to_string())? = preferences;
    }
    if let Some(snapshot) = backup.backup_preferences.as_ref() {
        let mut settings = before_backup_settings.clone();
        settings.auto_schedule_enabled = snapshot.auto_schedule_enabled;
        settings.auto_interval_hours = snapshot.auto_interval_hours;
        settings.auto_on_change = snapshot.auto_on_change;
        if let Err(error) = persist_backup_settings(&state.backup_settings_path, &settings) {
            let _ = persist_mobile_preferences(&state.mobile_preferences_path, &before_mobile_preferences);
            *state.mobile_preferences.lock().map_err(|e| e.to_string())? = before_mobile_preferences.clone();
            let _ = state
                .store
                .lock()
                .map_err(|e| e.to_string())?
                .restore(before_profiles.clone());
            let _ = persist_runtime_overrides(&state.runtime_overrides_path, &before_overrides);
            *state.runtime_overrides.lock().map_err(|e| e.to_string())? = before_overrides.clone();
            return Err(format!("restore backup preferences: {error}"));
        }
        *state.backup_settings.lock().map_err(|e| e.to_string())? = settings;
    }

    let apply_result = async {
        if was_running {
            if backup.profiles.active_id.is_some() {
                start_active_core(&state, was_transparent).await?;
                if was_transparent {
                    wait_for_tun_stable(&state).await?;
                }
            } else {
                call_agent(&state.agent_socket, json!({ "op": "stop_core" })).await?;
            }
        } else if backup.profiles.active_id.is_some()
            && call_agent(&state.agent_socket, json!({ "op": "status" })).await.is_ok()
        {
            let source = active_yaml(&state)?;
            let overrides = state.runtime_overrides.lock().map_err(|e| e.to_string())?.clone();
            let yaml = render_runtime_config(&state, &source, &overrides, true)?;
            call_agent(&state.agent_socket, json!({ "op": "apply_config", "yaml": yaml })).await?;
        }
        Ok::<(), String>(())
    }
    .await;

    if let Err(error) = apply_result {
        let _ = state
            .store
            .lock()
            .map_err(|e| e.to_string())?
            .restore(before_profiles.clone());
        let _ = persist_runtime_overrides(&state.runtime_overrides_path, &before_overrides);
        *state.runtime_overrides.lock().map_err(|e| e.to_string())? = before_overrides;
        let _ = persist_mobile_preferences(&state.mobile_preferences_path, &before_mobile_preferences);
        *state.mobile_preferences.lock().map_err(|e| e.to_string())? = before_mobile_preferences;
        let _ = persist_backup_settings(&state.backup_settings_path, &before_backup_settings);
        *state.backup_settings.lock().map_err(|e| e.to_string())? = before_backup_settings;
        if was_running && before_profiles.active_id.is_some() {
            let _ = start_active_core(&state, was_transparent).await;
            if was_transparent {
                let _ = wait_for_tun_stable(&state).await;
            }
        }
        return Err(format!("Backup could not be applied; previous state restored: {error}"));
    }
    if let Ok(mut attempts) = state.auto_update_attempts.lock() {
        attempts.clear();
    }
    Ok(state.store.lock().map_err(|e| e.to_string())?.document())
}

#[tauri::command]
async fn mobile_set_boot_module(enable: bool, state: State<'_, MobileState>) -> Result<BootModuleStatus, String> {
    if enable {
        let stage = state.app_data_dir.join("runtime-stage/ksu-module");
        let module_prop = stage.join("module.prop");
        let service = stage.join("service.sh");
        let uninstall = stage.join("uninstall.sh");
        write_mode(&module_prop, KSU_MODULE_PROP.as_bytes(), 0o600)?;
        write_mode(&service, KSU_SERVICE.as_bytes(), 0o700)?;
        write_mode(&uninstall, KSU_UNINSTALL.as_bytes(), 0o700)?;
        let module = Path::new(KSU_MODULE_DIR);
        let command = format!(
            "mkdir -p {module}; cp {prop} {module}/module.prop; cp {service} {module}/service.sh; cp {uninstall} {module}/uninstall.sh; chmod 644 {module}/module.prop; chmod 755 {module}/service.sh {module}/uninstall.sh; rm -f {module}/disable {module}/remove",
            module = shell_quote(module),
            prop = shell_quote(&module_prop),
            service = shell_quote(&service),
            uninstall = shell_quote(&uninstall),
        );
        run_su(&command, "KernelSU module install").await?;
    } else {
        let command = format!("rm -rf {}", shell_quote(Path::new(KSU_MODULE_DIR)));
        run_su(&command, "KernelSU module removal").await?;
    }
    mobile_boot_module_status().await
}

#[tauri::command]
async fn mobile_bootstrap_runtime(
    state: State<'_, MobileState>,
    vpn: State<'_, AndroidVpn>,
) -> Result<RuntimeStatus, String> {
    // Preserve a core that was upgraded online to a newer release. Bootstrap
    // should refresh the agent/geodata and repair an absent/older core, but it
    // must not silently downgrade a newer Mihomo back to the APK-bundled one.
    let installed_core_version = installed_core_version(&state).await.ok();
    let install_packaged_core = should_install_packaged_core(installed_core_version.as_deref());
    if call_agent(&state.agent_socket, json!({ "op": "status" })).await.is_ok() {
        let _ = call_agent(&state.agent_socket, json!({ "op": "shutdown" })).await;
        for _ in 0..30 {
            if !state.agent_socket.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
    let (agent, core, geosite) = stage_runtime(&state.app_data_dir)?;
    if let Some(parent) = state.agent_socket.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create runtime socket directory: {e}"))?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("chmod runtime socket directory: {e}"))?;
    }
    let _ = fs::remove_file(&state.agent_socket);
    let uid = unsafe { libc::getuid() };
    let root = Path::new(ROOT_RUNTIME);
    let root_agent = root.join("bin/cv4a-root-agent");
    let root_core = root.join("bin/mihomo");
    let root_geosite = root.join("data/GeoSite.dat");
    let context_arg = state
        .socket_context
        .as_ref()
        .map(|context| format!(" --socket-context {}", shell_quote_str(context)))
        .unwrap_or_default();
    let policy_patch = state
        .app_selinux_type
        .as_ref()
        .map(|app_type| {
            format!(
                "root_type=$(cut -d: -f3 /proc/self/attr/current 2>/dev/null || true); ksud_bin=$(command -v ksud 2>/dev/null || true); if [ -z \"$ksud_bin\" ] && [ -x /data/adb/ksu/bin/ksud ]; then ksud_bin=/data/adb/ksu/bin/ksud; fi; if [ -n \"$ksud_bin\" ] && [ -n \"$root_type\" ]; then \"$ksud_bin\" sepolicy patch \"allow {app_type} $root_type unix_stream_socket connectto\" >/dev/null 2>&1 || true; fi; "
            )
        })
        .unwrap_or_default();
    let profiles_path = state.app_data_dir.join("profiles.json");
    let runtime_preferences_path = state.app_data_dir.join("runtime-preferences.json");
    let backup_settings_path = state.app_data_dir.join("backup-settings.json");
    let backup_dir = state.app_data_dir.join("backups");
    let core_install = if install_packaged_core {
        format!(
            "cp {core} {root_core}.next; chmod 700 {root_core}.next; mv -f {root_core}.next {root_core}; ",
            core = shell_quote(&core),
            root_core = shell_quote(&root_core),
        )
    } else {
        String::new()
    };
    let command = format!(
        "umask 077; {policy_patch}mkdir -p {root}/bin {root}/config {root}/data {root}/logs {root}/run; cp {agent} {root_agent}.next; cp {geosite} {root_geosite}.next; chmod 700 {root_agent}.next; chmod 600 {root_geosite}.next; mv -f {root_agent}.next {root_agent}; {core_install}mv -f {root_geosite}.next {root_geosite}; nohup {root_agent} serve --socket {socket} --peer-uid {uid} --profiles {profiles} --runtime-preferences {runtime_preferences} --backup-settings {backup_settings} --backup-dir {backup_dir}{context_arg} >/data/adb/clash-verge4android/logs/agent.log 2>&1 </dev/null &",
        policy_patch = policy_patch,
        root = shell_quote(root),
        agent = shell_quote(&agent),
        geosite = shell_quote(&geosite),
        root_agent = shell_quote(&root_agent),
        root_geosite = shell_quote(&root_geosite),
        core_install = core_install,
        socket = shell_quote(&state.agent_socket),
        profiles = shell_quote(&profiles_path),
        runtime_preferences = shell_quote(&runtime_preferences_path),
        backup_settings = shell_quote(&backup_settings_path),
        backup_dir = shell_quote(&backup_dir),
        context_arg = context_arg,
    );
    let mut process = tokio::process::Command::new("su");
    process.args(["-c", &command]).kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(30), process.output())
        .await
        .map_err(|_| "KernelSU bootstrap timed out".to_owned())?
        .map_err(|e| format!("start KernelSU bootstrap: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "KernelSU bootstrap failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    for _ in 0..50 {
        if call_agent(&state.agent_socket, json!({ "op": "status" })).await.is_ok() {
            let vpn = vpn.inner().clone();
            run_vpn_plugin(move || vpn.set_tun_tile_registered(true)).await?;
            return Ok(runtime_status(&state).await);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err("root-agent did not become reachable after bootstrap".into())
}

fn active_yaml(state: &MobileState) -> Result<String, String> {
    Ok(active_profile(state)?.yaml)
}

fn core_client(state: &MobileState) -> Result<cv4a_mihomo_client::Client, String> {
    cv4a_mihomo_client::Client::new(&state.controller_socket)
}

async fn wait_for_core(state: &MobileState, expected_rules: usize) -> Result<CoreSnapshot, String> {
    let client = core_client(state)?;
    let mut last_error = "Mihomo controller did not become ready".to_owned();
    for _ in 0..60 {
        match client.snapshot().await {
            Ok(snapshot) if snapshot.rules.len() >= expected_rules => return Ok(snapshot),
            Ok(snapshot) => {
                last_error = format!(
                    "controller is reachable but only {}/{} rules are loaded",
                    snapshot.rules.len(),
                    expected_rules
                );
            }
            Err(error) => last_error = error,
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err(format!("Mihomo controller did not become ready: {last_error}"))
}

async fn start_source_core(state: &MobileState, source: &str, transparent: bool) -> Result<CoreSnapshot, String> {
    let expected_rules = clash_verge_mobile::inspect(&source)?.rules.len();
    let overrides = state.runtime_overrides.lock().map_err(|e| e.to_string())?.clone();
    let yaml = render_runtime_config(state, source, &overrides, transparent)?;
    call_agent(&state.agent_socket, json!({ "op": "apply_config", "yaml": yaml })).await?;
    call_agent(&state.agent_socket, json!({ "op": "stop_core" })).await?;
    call_agent(&state.agent_socket, json!({ "op": "start_core" })).await?;
    wait_for_core(state, expected_rules).await
}

#[tauri::command]
async fn mobile_runtime_preferences(
    state: State<'_, MobileState>,
) -> Result<cv4a_mihomo_client::RuntimePreferences, String> {
    if let Ok(client) = core_client(&state)
        && let Ok(preferences) = client.runtime_preferences().await
    {
        return Ok(preferences);
    }
    let overrides = state.runtime_overrides.lock().map_err(|e| e.to_string())?.clone();
    Ok(cv4a_mihomo_client::RuntimePreferences {
        ipv6: overrides.ipv6.unwrap_or(true),
        unified_delay: overrides.unified_delay.unwrap_or(false),
        log_level: overrides.log_level.unwrap_or_else(|| "info".into()),
        tcp_concurrent: overrides.tcp_concurrent.unwrap_or(false),
        find_process_mode: overrides.find_process_mode.unwrap_or_else(|| "strict".into()),
    })
}

#[tauri::command]
fn mobile_preferences(state: State<'_, MobileState>) -> Result<MobilePreferences, String> {
    Ok(state.mobile_preferences.lock().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
fn mobile_set_preferences(
    preferences: MobilePreferences,
    state: State<'_, MobileState>,
) -> Result<MobilePreferences, String> {
    let next = preferences.normalized()?;
    persist_mobile_preferences(&state.mobile_preferences_path, &next)?;
    *state.mobile_preferences.lock().map_err(|e| e.to_string())? = next.clone();
    Ok(next)
}

#[tauri::command]
async fn mobile_set_runtime_preferences(
    preferences: cv4a_mihomo_client::RuntimePreferences,
    state: State<'_, MobileState>,
) -> Result<cv4a_mihomo_client::RuntimePreferences, String> {
    if !matches!(
        preferences.log_level.as_str(),
        "debug" | "info" | "warning" | "warn" | "error" | "silent"
    ) {
        return Err("Invalid Mihomo log level".into());
    }
    if !matches!(preferences.find_process_mode.as_str(), "off" | "strict" | "always") {
        return Err("Invalid Mihomo find-process-mode".into());
    }
    let before = state.runtime_overrides.lock().map_err(|e| e.to_string())?.clone();
    let previous_live = match core_client(&state) {
        Ok(client) => client.runtime_preferences().await.ok(),
        Err(_) => None,
    };
    let source = active_yaml(&state)?;
    let (core_running, transparent) = running_core_state(&state).await;
    let mut next = before.clone();
    next.ipv6 = Some(preferences.ipv6);
    next.unified_delay = Some(preferences.unified_delay);
    next.log_level = Some(preferences.log_level.clone());
    next.tcp_concurrent = Some(preferences.tcp_concurrent);
    next.find_process_mode = Some(preferences.find_process_mode.clone());
    let yaml = render_runtime_config(&state, &source, &next, transparent || !core_running)?;
    persist_runtime_overrides(&state.runtime_overrides_path, &next)?;
    *state.runtime_overrides.lock().map_err(|e| e.to_string())? = next.clone();
    let update_result = async {
        call_agent(&state.agent_socket, json!({ "op": "apply_config", "yaml": yaml })).await?;
        if core_running {
            core_client(&state)?.patch_runtime_preferences(&preferences).await?;
        }
        Ok::<(), String>(())
    }
    .await;

    if let Err(error) = update_result {
        let _ = persist_runtime_overrides(&state.runtime_overrides_path, &before);
        *state.runtime_overrides.lock().map_err(|e| e.to_string())? = before.clone();
        let rollback_yaml = render_runtime_config(&state, &source, &before, transparent || !core_running);
        if let Ok(rollback_yaml) = rollback_yaml {
            let _ = call_agent(
                &state.agent_socket,
                json!({ "op": "apply_config", "yaml": rollback_yaml }),
            )
            .await;
        }
        if let Some(previous_live) = previous_live
            && let Ok(client) = core_client(&state)
        {
            let _ = client.patch_runtime_preferences(&previous_live).await;
        }
        return Err(format!(
            "Failed to apply Mihomo settings; previous values restored: {error}"
        ));
    }
    mobile_runtime_preferences(state).await
}

fn validate_dns_override_settings(settings: &DnsOverrideSettings) -> Result<(), String> {
    if !settings.enabled {
        return Ok(());
    }
    let yaml = settings.yaml.trim();
    if yaml.is_empty() {
        return Err("DNS override is enabled but empty".into());
    }
    if yaml.len() > 256 * 1024 {
        return Err("DNS override exceeds 256 KiB".into());
    }
    let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(yaml).map_err(|e| format!("DNS override YAML: {e}"))?;
    let mapping = value.as_mapping().ok_or("DNS override must be a YAML mapping")?;
    for (key, value) in mapping {
        let Some(key) = key.as_str() else {
            return Err("DNS override keys must be strings".into());
        };
        if !matches!(key, "dns" | "hosts") {
            return Err(format!("DNS override may only contain dns/hosts, found: {key}"));
        }
        if !value.is_mapping() {
            return Err(format!("DNS override {key} must be a mapping"));
        }
    }
    Ok(())
}

#[tauri::command]
fn mobile_dns_override(
    profile_id: Option<String>,
    state: State<'_, MobileState>,
) -> Result<DnsOverrideSettings, String> {
    let overrides = state.runtime_overrides.lock().map_err(|e| e.to_string())?.clone();
    if let Some(profile_id) = profile_id {
        let document = state.store.lock().map_err(|e| e.to_string())?.document();
        let profile = document
            .profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .ok_or("Profile not found")?;
        if profile.dns_override_enabled.is_some() || profile.dns_override_yaml.is_some() {
            return Ok(DnsOverrideSettings {
                enabled: profile.dns_override_enabled.unwrap_or(false),
                yaml: profile.dns_override_yaml.clone().unwrap_or_default(),
            });
        }
    }
    Ok(DnsOverrideSettings {
        enabled: overrides.dns_override_enabled.unwrap_or(false),
        yaml: overrides.dns_override_yaml.unwrap_or_default(),
    })
}

#[tauri::command]
async fn mobile_set_dns_override(
    profile_id: Option<String>,
    settings: DnsOverrideSettings,
    state: State<'_, MobileState>,
) -> Result<DnsOverrideSettings, String> {
    validate_dns_override_settings(&settings)?;
    if let Some(profile_id) = profile_id {
        let before_document = state.store.lock().map_err(|e| e.to_string())?.document();
        let is_active = before_document.active_id.as_deref() == Some(profile_id.as_str());
        state
            .store
            .lock()
            .map_err(|e| e.to_string())?
            .set_profile_dns_override(&profile_id, Some(settings.enabled), Some(settings.yaml.clone()))?;
        if !is_active {
            return Ok(settings);
        }
        let source = active_yaml(&state)?;
        let (was_running, was_transparent) = running_core_state(&state).await;
        let overrides = state.runtime_overrides.lock().map_err(|e| e.to_string())?.clone();
        let preview_yaml = match render_runtime_config(&state, &source, &overrides, was_transparent || was_running) {
            Ok(yaml) => yaml,
            Err(error) => {
                let _ = state.store.lock().map_err(|e| e.to_string())?.restore(before_document);
                return Err(error);
            }
        };
        let apply_result = async {
            if was_running {
                start_source_core(&state, &source, was_transparent).await?;
                if was_transparent {
                    wait_for_tun_stable(&state).await?;
                }
            } else if call_agent(&state.agent_socket, json!({ "op": "status" })).await.is_ok() {
                call_agent(
                    &state.agent_socket,
                    json!({ "op": "apply_config", "yaml": preview_yaml }),
                )
                .await?;
            }
            Ok::<(), String>(())
        }
        .await;
        if let Err(error) = apply_result {
            let _ = state.store.lock().map_err(|e| e.to_string())?.restore(before_document);
            if was_running {
                let _ = start_source_core(&state, &source, was_transparent).await;
            }
            return Err(format!(
                "Failed to apply DNS override; previous DNS settings restored: {error}"
            ));
        }
        return Ok(settings);
    }
    let before = state.runtime_overrides.lock().map_err(|e| e.to_string())?.clone();
    let mut next = before.clone();
    next.dns_override_enabled = Some(settings.enabled);
    next.dns_override_yaml = Some(settings.yaml.clone());

    let source = active_yaml(&state)?;
    let (was_running, was_transparent) = running_core_state(&state).await;
    let preview_yaml = render_runtime_config(&state, &source, &next, was_transparent || was_running)?;

    persist_runtime_overrides(&state.runtime_overrides_path, &next)?;
    *state.runtime_overrides.lock().map_err(|e| e.to_string())? = next.clone();

    let apply_result = async {
        if was_running {
            start_source_core(&state, &source, was_transparent).await?;
            if was_transparent {
                wait_for_tun_stable(&state).await?;
            }
        } else if call_agent(&state.agent_socket, json!({ "op": "status" })).await.is_ok() {
            call_agent(
                &state.agent_socket,
                json!({ "op": "apply_config", "yaml": preview_yaml }),
            )
            .await?;
        }
        Ok::<(), String>(())
    }
    .await;

    if let Err(error) = apply_result {
        let _ = persist_runtime_overrides(&state.runtime_overrides_path, &before);
        *state.runtime_overrides.lock().map_err(|e| e.to_string())? = before.clone();
        if was_running {
            let _ = start_source_core(&state, &source, was_transparent).await;
            if was_transparent {
                let _ = wait_for_tun_stable(&state).await;
            }
        }
        return Err(format!(
            "Failed to apply DNS override; previous DNS settings restored: {error}"
        ));
    }

    Ok(DnsOverrideSettings {
        enabled: next.dns_override_enabled.unwrap_or(false),
        yaml: next.dns_override_yaml.unwrap_or_default(),
    })
}

fn port_settings_from_overrides(overrides: &RuntimeOverrides) -> PortSettings {
    PortSettings {
        allow_lan: overrides.allow_lan.unwrap_or(false),
        bind_address: overrides.bind_address.clone().unwrap_or_else(|| "127.0.0.1".into()),
        mixed_port: overrides.mixed_port.unwrap_or(0),
        http_port: overrides.http_port.unwrap_or(0),
        socks_port: overrides.socks_port.unwrap_or(0),
        redir_port: overrides.redir_port.unwrap_or(0),
        tproxy_port: overrides.tproxy_port.unwrap_or(0),
    }
}

#[tauri::command]
fn mobile_port_settings(state: State<'_, MobileState>) -> Result<PortSettings, String> {
    let overrides = state.runtime_overrides.lock().map_err(|e| e.to_string())?;
    Ok(port_settings_from_overrides(&overrides))
}

#[tauri::command]
async fn mobile_set_port_settings(
    settings: PortSettings,
    state: State<'_, MobileState>,
) -> Result<PortSettings, String> {
    let before = state.runtime_overrides.lock().map_err(|e| e.to_string())?.clone();
    let mut next = before.clone();
    next.allow_lan = Some(settings.allow_lan);
    next.bind_address = Some(settings.bind_address.trim().to_owned());
    next.mixed_port = Some(settings.mixed_port);
    next.http_port = Some(settings.http_port);
    next.socks_port = Some(settings.socks_port);
    next.redir_port = Some(settings.redir_port);
    next.tproxy_port = Some(settings.tproxy_port);

    let source = active_yaml(&state)?;
    let (was_running, was_transparent) = running_core_state(&state).await;
    let rendered = render_runtime_config(&state, &source, &next, was_transparent || was_running)?;

    persist_runtime_overrides(&state.runtime_overrides_path, &next)?;
    *state.runtime_overrides.lock().map_err(|e| e.to_string())? = next.clone();

    let apply_result = async {
        if was_running {
            start_source_core(&state, &source, was_transparent).await?;
            if was_transparent {
                wait_for_tun_stable(&state).await?;
            }
        } else if call_agent(&state.agent_socket, json!({ "op": "status" })).await.is_ok() {
            call_agent(&state.agent_socket, json!({ "op": "apply_config", "yaml": rendered })).await?;
        }
        Ok::<(), String>(())
    }
    .await;

    if let Err(error) = apply_result {
        let _ = persist_runtime_overrides(&state.runtime_overrides_path, &before);
        *state.runtime_overrides.lock().map_err(|e| e.to_string())? = before.clone();
        if was_running {
            let _ = start_source_core(&state, &source, was_transparent).await;
            if was_transparent {
                let _ = wait_for_tun_stable(&state).await;
            }
        }
        return Err(format!(
            "Failed to apply proxy listener settings; previous values restored: {error}"
        ));
    }
    Ok(port_settings_from_overrides(&next))
}

fn external_controller_settings_from_overrides(overrides: &RuntimeOverrides) -> ExternalControllerSettings {
    ExternalControllerSettings {
        enabled: overrides.external_controller_enabled.unwrap_or(false),
        address: overrides
            .external_controller
            .clone()
            .unwrap_or_else(|| "127.0.0.1:9090".into()),
        secret: overrides.external_controller_secret.clone().unwrap_or_default(),
        allow_private_network: overrides.external_controller_allow_private_network.unwrap_or(true),
        allow_origins: overrides.external_controller_allow_origins.clone().unwrap_or_default(),
    }
}

#[tauri::command]
fn mobile_external_controller_settings(state: State<'_, MobileState>) -> Result<ExternalControllerSettings, String> {
    let overrides = state.runtime_overrides.lock().map_err(|e| e.to_string())?;
    Ok(external_controller_settings_from_overrides(&overrides))
}

#[tauri::command]
fn mobile_network_interfaces() -> Result<Vec<network_interface::NetworkInterface>, String> {
    use network_interface::{NetworkInterface, NetworkInterfaceConfig as _};

    let mut interfaces = NetworkInterface::show().map_err(|e| e.to_string())?;
    interfaces.sort_by(|a, b| {
        a.internal
            .cmp(&b.internal)
            .then_with(|| a.index.cmp(&b.index))
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(interfaces)
}

#[tauri::command]
async fn mobile_set_external_controller_settings(
    mut settings: ExternalControllerSettings,
    state: State<'_, MobileState>,
) -> Result<ExternalControllerSettings, String> {
    settings.address = settings.address.trim().to_owned();
    settings.secret = settings.secret.trim().to_owned();
    settings.allow_origins = settings
        .allow_origins
        .into_iter()
        .map(|origin| origin.trim().to_owned())
        .filter(|origin| !origin.is_empty())
        .collect();
    let mut seen = std::collections::HashSet::new();
    settings.allow_origins.retain(|origin| seen.insert(origin.clone()));

    let before = state.runtime_overrides.lock().map_err(|e| e.to_string())?.clone();
    let mut next = before.clone();
    next.external_controller_enabled = Some(settings.enabled);
    next.external_controller = Some(settings.address.clone());
    next.external_controller_secret = Some(settings.secret.clone());
    next.external_controller_allow_private_network = Some(settings.allow_private_network);
    next.external_controller_allow_origins = Some(settings.allow_origins.clone());

    let source = active_yaml(&state)?;
    let (was_running, was_transparent) = running_core_state(&state).await;
    let rendered = render_runtime_config(&state, &source, &next, was_transparent || was_running)?;

    persist_runtime_overrides(&state.runtime_overrides_path, &next)?;
    *state.runtime_overrides.lock().map_err(|e| e.to_string())? = next.clone();

    let apply_result = async {
        if was_running {
            start_source_core(&state, &source, was_transparent).await?;
            if was_transparent {
                wait_for_tun_stable(&state).await?;
            }
        } else if call_agent(&state.agent_socket, json!({ "op": "status" })).await.is_ok() {
            call_agent(&state.agent_socket, json!({ "op": "apply_config", "yaml": rendered })).await?;
        }
        Ok::<(), String>(())
    }
    .await;

    if let Err(error) = apply_result {
        let _ = persist_runtime_overrides(&state.runtime_overrides_path, &before);
        *state.runtime_overrides.lock().map_err(|e| e.to_string())? = before.clone();
        if was_running {
            let _ = start_source_core(&state, &source, was_transparent).await;
            if was_transparent {
                let _ = wait_for_tun_stable(&state).await;
            }
        }
        return Err(format!(
            "Failed to apply external controller settings; previous values restored: {error}"
        ));
    }
    Ok(external_controller_settings_from_overrides(&next))
}

fn tun_preferences_from_overrides(overrides: &RuntimeOverrides) -> cv4a_mihomo_client::TunPreferences {
    cv4a_mihomo_client::TunPreferences {
        stack: overrides.tun_stack.clone().unwrap_or_else(|| "mixed".into()),
        strict_route: overrides.tun_strict_route.unwrap_or(true),
        // Android root TUN must keep interface auto-detection disabled: Mihomo can
        // otherwise choose the app's VpnService tun0 as its own outbound path.
        auto_detect_interface: false,
        dns_hijack: overrides
            .tun_dns_hijack
            .clone()
            .unwrap_or_else(|| vec!["any:53".into(), "tcp://any:53".into()]),
        mtu: overrides.tun_mtu.unwrap_or(1500),
    }
}

#[tauri::command]
fn mobile_tun_preferences(state: State<'_, MobileState>) -> Result<cv4a_mihomo_client::TunPreferences, String> {
    let overrides = state.runtime_overrides.lock().map_err(|e| e.to_string())?;
    Ok(tun_preferences_from_overrides(&overrides))
}

#[tauri::command]
async fn mobile_set_tun_preferences(
    mut preferences: cv4a_mihomo_client::TunPreferences,
    state: State<'_, MobileState>,
) -> Result<cv4a_mihomo_client::TunPreferences, String> {
    preferences.auto_detect_interface = false;
    preferences.stack = preferences.stack.trim().to_ascii_lowercase();
    if !matches!(preferences.stack.as_str(), "system" | "gvisor" | "mixed" | "mips") {
        return Err("Invalid Mihomo TUN stack".into());
    }
    if !(576..=65_535).contains(&preferences.mtu) {
        return Err("TUN MTU must be between 576 and 65535".into());
    }
    preferences.dns_hijack = preferences
        .dns_hijack
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect();

    let before = state.runtime_overrides.lock().map_err(|e| e.to_string())?.clone();
    let mut next = before.clone();
    next.tun_stack = Some(preferences.stack.clone());
    next.tun_strict_route = Some(preferences.strict_route);
    next.tun_auto_detect_interface = Some(false);
    next.tun_dns_hijack = Some(preferences.dns_hijack.clone());
    next.tun_mtu = Some(preferences.mtu);

    let source = active_yaml(&state)?;
    let yaml = render_runtime_config(&state, &source, &next, true)?;
    let rollback_yaml = render_runtime_config(&state, &source, &before, true)?;
    let (was_running, was_transparent) = running_core_state(&state).await;

    persist_runtime_overrides(&state.runtime_overrides_path, &next)?;
    *state.runtime_overrides.lock().map_err(|e| e.to_string())? = next.clone();

    let apply_result = async {
        call_agent(&state.agent_socket, json!({ "op": "apply_config", "yaml": yaml })).await?;
        if was_running && was_transparent {
            call_agent(&state.agent_socket, json!({ "op": "stop_core" })).await?;
            call_agent(&state.agent_socket, json!({ "op": "start_core" })).await?;
            wait_for_tun_stable(&state).await?;
        }
        Ok::<(), String>(())
    }
    .await;

    if let Err(error) = apply_result {
        let _ = persist_runtime_overrides(&state.runtime_overrides_path, &before);
        *state.runtime_overrides.lock().map_err(|e| e.to_string())? = before.clone();
        let _ = call_agent(
            &state.agent_socket,
            json!({ "op": "apply_config", "yaml": rollback_yaml }),
        )
        .await;
        if was_running && was_transparent {
            let _ = call_agent(&state.agent_socket, json!({ "op": "stop_core" })).await;
            let _ = call_agent(&state.agent_socket, json!({ "op": "start_core" })).await;
            let _ = wait_for_tun_stable(&state).await;
        }
        return Err(format!(
            "Failed to apply TUN settings; previous values restored: {error}"
        ));
    }

    Ok(tun_preferences_from_overrides(&next))
}

async fn start_active_core(state: &MobileState, transparent: bool) -> Result<CoreSnapshot, String> {
    let source = active_yaml(state)?;
    start_source_core(state, &source, transparent).await
}

async fn running_core_state(state: &MobileState) -> (bool, bool) {
    let Ok(reply) = call_agent(&state.agent_socket, json!({ "op": "status" })).await else {
        return (false, false);
    };
    let Some(status) = reply.status else {
        return (false, false);
    };
    (status.core_running, status.transparent_active)
}

async fn wait_for_tun_stable(state: &MobileState) -> Result<(), String> {
    let mut stable_checks = 0_u8;
    for _ in 0..80 {
        let status = runtime_status(state).await;
        if status.core_running && status.core_connected && status.transparent_active {
            stable_checks += 1;
            if stable_checks >= 8 {
                return Ok(());
            }
        } else {
            stable_checks = 0;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let _ = call_agent(&state.agent_socket, json!({ "op": "stop_core" })).await;
    Err("Mihomo started but the Android TUN interface did not remain active".into())
}

async fn reload_running_core(state: &MobileState) -> Result<(), String> {
    let Ok(reply) = call_agent(&state.agent_socket, json!({ "op": "status" })).await else {
        return Ok(());
    };
    let Some(status) = reply.status else {
        return Ok(());
    };
    if !status.core_running {
        return Ok(());
    }
    let transparent = status.transparent_active;
    start_active_core(state, transparent).await?;
    if transparent {
        wait_for_tun_stable(state).await?;
    }
    Ok(())
}

#[tauri::command]
async fn mobile_start_preview_core(state: State<'_, MobileState>) -> Result<CoreSnapshot, String> {
    start_active_core(&state, false).await
}

#[tauri::command]
async fn mobile_stop_preview_core(state: State<'_, MobileState>) -> Result<RuntimeStatus, String> {
    call_agent(&state.agent_socket, json!({ "op": "stop_core" })).await?;
    Ok(runtime_status(&state).await)
}

#[tauri::command]
async fn mobile_restart_core(state: State<'_, MobileState>) -> Result<CoreSnapshot, String> {
    let (running, transparent) = running_core_state(&state).await;
    if !running {
        return Err("Mihomo is not running".into());
    }
    let snapshot = start_active_core(&state, transparent).await?;
    if transparent {
        wait_for_tun_stable(&state).await?;
    }
    Ok(snapshot)
}

#[tauri::command]
async fn mobile_core_snapshot(state: State<'_, MobileState>) -> Result<CoreSnapshot, String> {
    let snapshot = core_client(&state)?.snapshot().await?;
    let active_id = state.store.lock().map_err(|e| e.to_string())?.document().active_id;
    if let Some(active_id) = active_id {
        let selections = snapshot
            .groups
            .iter()
            .filter(|group| group.selectable)
            .filter_map(|group| {
                group.selected.as_ref().map(|member| clash_verge_mobile::SelectedProxy {
                    group: group.name.clone(),
                    member: member.clone(),
                })
            })
            .collect::<Vec<_>>();
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        store.record_selections(&active_id, selections)?;
    }
    Ok(snapshot)
}

#[tauri::command]
async fn mobile_select_proxy(
    group: String,
    member: String,
    state: State<'_, MobileState>,
) -> Result<CoreSnapshot, String> {
    let client = core_client(&state)?;
    let before_snapshot = client.snapshot().await?;
    let previous = before_snapshot
        .groups
        .iter()
        .find(|candidate| candidate.name == group)
        .and_then(|candidate| candidate.selected.clone());
    client.select(&group, &member).await?;
    let active_id = state
        .store
        .lock()
        .map_err(|e| e.to_string())?
        .document()
        .active_id
        .ok_or("No active profile")?;
    let persist_result = {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        store.record_selection(&active_id, group.clone(), member.clone())
    };
    if let Err(error) = persist_result {
        if let Some(previous) = previous {
            let _ = client.select(&group, &previous).await;
        }
        return Err(format!(
            "Proxy changed but selection could not be persisted; live selection restored: {error}"
        ));
    }
    let auto_close = state
        .mobile_preferences
        .lock()
        .map(|preferences| preferences.auto_close_connection)
        .unwrap_or(false);
    if auto_close
        && let Some(previous) = previous.as_deref()
        && previous != member
        && let Ok(connections) = client.connections().await
    {
        for connection in connections
            .connections
            .iter()
            .filter(|connection| connection.chains.iter().any(|chain| chain == previous))
        {
            let _ = client.close_connection(&connection.id).await;
        }
    }
    client.snapshot().await
}

#[tauri::command]
async fn mobile_set_proxy_chain(
    target_group: String,
    nodes: Vec<String>,
    state: State<'_, MobileState>,
) -> Result<CoreSnapshot, String> {
    let client = core_client(&state)?;
    let snapshot = client.snapshot().await?;
    let target = snapshot
        .groups
        .iter()
        .find(|group| group.name == target_group)
        .ok_or("Proxy chain target group is not loaded")?;
    if !target.selectable {
        return Err("Proxy chain target group is not selectable".into());
    }
    let exit = nodes.last().ok_or("Proxy chain requires at least two nodes")?;
    if !target.members.iter().any(|member| member == exit) {
        return Err("Proxy chain exit node must be a member of the target group".into());
    }
    let (running, transparent) = running_core_state(&state).await;
    if !running {
        return Err("Start Mihomo before configuring a proxy chain".into());
    }
    let before = state.store.lock().map_err(|e| e.to_string())?.document();
    let active_id = before.active_id.clone().ok_or("No active profile")?;
    state
        .store
        .lock()
        .map_err(|e| e.to_string())?
        .set_proxy_chain(&active_id, target_group, nodes)?;
    if let Err(error) = start_active_core(&state, transparent).await {
        let _ = state.store.lock().map_err(|e| e.to_string())?.restore(before);
        let _ = start_active_core(&state, transparent).await;
        return Err(format!(
            "Failed to apply proxy chain; previous runtime restored: {error}"
        ));
    }
    if transparent {
        wait_for_tun_stable(&state).await?;
    }
    let _ = core_client(&state)?.close_all_connections().await;
    core_client(&state)?.snapshot().await
}

#[tauri::command]
async fn mobile_clear_proxy_chain(state: State<'_, MobileState>) -> Result<CoreSnapshot, String> {
    let before = state.store.lock().map_err(|e| e.to_string())?.document();
    let active_id = before.active_id.clone().ok_or("No active profile")?;
    let profile = before
        .profiles
        .iter()
        .find(|profile| profile.id == active_id)
        .ok_or("Active profile is missing")?;
    profile.proxy_chain.as_ref().ok_or("No proxy chain is active")?;
    let (running, transparent) = running_core_state(&state).await;
    if !running {
        return Err("Start Mihomo before disconnecting a proxy chain".into());
    }
    state
        .store
        .lock()
        .map_err(|e| e.to_string())?
        .clear_proxy_chain(&active_id)?;
    if let Err(error) = start_active_core(&state, transparent).await {
        let _ = state.store.lock().map_err(|e| e.to_string())?.restore(before);
        let _ = start_active_core(&state, transparent).await;
        return Err(format!(
            "Failed to remove proxy chain; previous runtime restored: {error}"
        ));
    }
    if transparent {
        wait_for_tun_stable(&state).await?;
    }
    let _ = core_client(&state)?.close_all_connections().await;
    core_client(&state)?.snapshot().await
}

#[tauri::command]
async fn mobile_set_mode(mode: String, state: State<'_, MobileState>) -> Result<CoreSnapshot, String> {
    let mode = mode.to_ascii_lowercase();
    if !matches!(mode.as_str(), "rule" | "global" | "direct") {
        return Err("Unsupported Mihomo mode".into());
    }
    let client = core_client(&state)?;
    let previous_mode = client.snapshot().await?.mode;
    let before = state.runtime_overrides.lock().map_err(|e| e.to_string())?.clone();
    let mut next = before.clone();
    next.mode = Some(mode.clone());
    let source = active_yaml(&state)?;
    let (_, transparent) = running_core_state(&state).await;
    let yaml = render_runtime_config(&state, &source, &next, transparent)?;

    client.set_mode(&mode).await?;
    if let Err(error) = call_agent(&state.agent_socket, json!({ "op": "apply_config", "yaml": yaml })).await {
        let _ = client.set_mode(&previous_mode).await;
        return Err(format!("Failed to persist Mihomo mode: {error}"));
    }
    if let Err(error) = persist_runtime_overrides(&state.runtime_overrides_path, &next) {
        let rollback_yaml = render_runtime_config(&state, &source, &before, transparent);
        if let Ok(rollback_yaml) = rollback_yaml {
            let _ = call_agent(
                &state.agent_socket,
                json!({ "op": "apply_config", "yaml": rollback_yaml }),
            )
            .await;
        }
        let _ = client.set_mode(&previous_mode).await;
        return Err(format!("Failed to save Mihomo mode: {error}"));
    }
    *state.runtime_overrides.lock().map_err(|e| e.to_string())? = next;
    if state
        .mobile_preferences
        .lock()
        .map(|preferences| preferences.auto_close_connection)
        .unwrap_or(false)
    {
        let _ = client.close_all_connections().await;
    }
    client.snapshot().await
}

#[tauri::command]
async fn mobile_connections(state: State<'_, MobileState>) -> Result<cv4a_mihomo_client::Connections, String> {
    core_client(&state)?.connections().await
}

#[tauri::command]
async fn mobile_close_connection(
    id: String,
    state: State<'_, MobileState>,
) -> Result<cv4a_mihomo_client::Connections, String> {
    let client = core_client(&state)?;
    client.close_connection(&id).await?;
    client.connections().await
}

#[tauri::command]
async fn mobile_close_all_connections(
    state: State<'_, MobileState>,
) -> Result<cv4a_mihomo_client::Connections, String> {
    let client = core_client(&state)?;
    client.close_all_connections().await?;
    client.connections().await
}

#[tauri::command]
async fn mobile_proxy_delay(proxy: String, state: State<'_, MobileState>) -> Result<u64, String> {
    let preferences = state.mobile_preferences.lock().map_err(|e| e.to_string())?.clone();
    core_client(&state)?
        .delay(
            &proxy,
            &preferences.default_latency_test,
            preferences.default_latency_timeout,
        )
        .await
}

#[tauri::command]
async fn mobile_proxy_providers(
    state: State<'_, MobileState>,
) -> Result<Vec<cv4a_mihomo_client::ProxyProviderSummary>, String> {
    core_client(&state)?.proxy_providers().await
}

#[tauri::command]
async fn mobile_update_proxy_provider(
    name: String,
    state: State<'_, MobileState>,
) -> Result<Vec<cv4a_mihomo_client::ProxyProviderSummary>, String> {
    let client = core_client(&state)?;
    client.update_proxy_provider(&name).await?;
    client.proxy_providers().await
}

#[tauri::command]
async fn mobile_rule_providers(
    state: State<'_, MobileState>,
) -> Result<Vec<cv4a_mihomo_client::RuleProviderSummary>, String> {
    core_client(&state)?.rule_providers().await
}

#[tauri::command]
async fn mobile_update_rule_provider(
    name: String,
    state: State<'_, MobileState>,
) -> Result<Vec<cv4a_mihomo_client::RuleProviderSummary>, String> {
    let client = core_client(&state)?;
    client.update_rule_provider(&name).await?;
    client.rule_providers().await
}

#[tauri::command]
async fn mobile_flush_fakeip(state: State<'_, MobileState>) -> Result<(), String> {
    core_client(&state)?.flush_fakeip().await
}

#[tauri::command]
async fn mobile_flush_dns(state: State<'_, MobileState>) -> Result<(), String> {
    core_client(&state)?.flush_dns().await
}

#[tauri::command]
async fn mobile_update_geo(state: State<'_, MobileState>) -> Result<(), String> {
    core_client(&state)?.update_geo().await
}

#[tauri::command]
async fn mobile_diagnostics(state: State<'_, MobileState>) -> Result<String, String> {
    let module_version = KSU_MODULE_PROP
        .lines()
        .find_map(|line| line.strip_prefix("version="))
        .unwrap_or("unknown");
    let runtime = runtime_status(&state).await;
    let installed_core = installed_core_version(&state)
        .await
        .unwrap_or_else(|error| format!("unavailable ({error})"));
    let document = state.store.lock().map_err(|e| e.to_string())?.document();
    let active = document
        .active_id
        .as_ref()
        .and_then(|active_id| document.profiles.iter().find(|profile| &profile.id == active_id));
    let summary = active.and_then(|profile| clash_verge_mobile::inspect(&profile.yaml).ok());
    let preferences = state.mobile_preferences.lock().map_err(|e| e.to_string())?.clone();
    let runtime_preferences = mobile_runtime_preferences(state.clone()).await.ok();
    let tun_preferences = state
        .runtime_overrides
        .lock()
        .map(|overrides| tun_preferences_from_overrides(&overrides))
        .ok();
    let port_settings = state
        .runtime_overrides
        .lock()
        .map(|overrides| port_settings_from_overrides(&overrides))
        .ok();
    let backup_settings = state.backup_settings.lock().map_err(|e| e.to_string())?.clone();
    let network_interfaces = mobile_network_interfaces().unwrap_or_default();

    let (core_mode, core_groups, core_rules, connections, memory) = if runtime.core_connected {
        let client = core_client(&state)?;
        let snapshot = client.snapshot().await.ok();
        let connections = client.connections().await.ok();
        (
            snapshot.as_ref().map(|snapshot| snapshot.mode.clone()),
            snapshot.as_ref().map(|snapshot| snapshot.groups.len()),
            snapshot.as_ref().map(|snapshot| snapshot.rules.len()),
            connections.as_ref().map(|connections| connections.connections.len()),
            connections.as_ref().map(|connections| connections.memory),
        )
    } else {
        (None, None, None, None, None)
    };

    let mut report = String::new();
    use std::fmt::Write as _;
    writeln!(report, "Clash Verge for Android diagnostics").ok();
    writeln!(report, "Generated: {}", chrono::Utc::now().to_rfc3339()).ok();
    writeln!(report, "Mobile version: {module_version}").ok();
    writeln!(report, "Backend version: {}", env!("CARGO_PKG_VERSION")).ok();
    writeln!(report, "Packaged Mihomo: {PACKAGED_MIHOMO_VERSION}").ok();
    writeln!(report, "Installed Mihomo: {installed_core}").ok();
    writeln!(report).ok();
    writeln!(report, "[Runtime]").ok();
    writeln!(report, "agentConnected={}", runtime.agent_connected).ok();
    writeln!(report, "coreRunning={}", runtime.core_running).ok();
    writeln!(report, "coreConnected={}", runtime.core_connected).ok();
    writeln!(report, "transparentActive={}", runtime.transparent_active).ok();
    writeln!(report, "mode={}", core_mode.as_deref().unwrap_or("unknown")).ok();
    writeln!(
        report,
        "loadedGroups={}",
        core_groups.map_or_else(|| "unknown".into(), |v| v.to_string())
    )
    .ok();
    writeln!(
        report,
        "loadedRules={}",
        core_rules.map_or_else(|| "unknown".into(), |v| v.to_string())
    )
    .ok();
    writeln!(
        report,
        "activeConnections={}",
        connections.map_or_else(|| "unknown".into(), |v| v.to_string())
    )
    .ok();
    writeln!(
        report,
        "mihomoMemoryBytes={}",
        memory.map_or_else(|| "unknown".into(), |v| v.to_string())
    )
    .ok();

    writeln!(report).ok();
    writeln!(report, "[Profile]").ok();
    writeln!(report, "profileCount={}", document.profiles.len()).ok();
    writeln!(
        report,
        "activeProfile={}",
        active.map(|profile| profile.name.as_str()).unwrap_or("none")
    )
    .ok();
    if let Some(profile) = active {
        writeln!(report, "savedSelections={}", profile.selected.len()).ok();
        writeln!(report, "proxyChain={}", profile.proxy_chain.is_some()).ok();
        writeln!(report, "profileMerge={}", profile.merge_yaml.is_some()).ok();
        writeln!(report, "profileScript={}", profile.script_js.is_some()).ok();
    }
    if let Some(summary) = summary {
        writeln!(report, "sourceNodes={}", summary.nodes.len()).ok();
        writeln!(report, "sourceGroups={}", summary.groups.len()).ok();
        writeln!(report, "sourceRules={}", summary.rules.len()).ok();
        writeln!(report, "providerCount={}", summary.provider_count).ok();
    }

    writeln!(report).ok();
    writeln!(report, "[Preferences]").ok();
    if let Some(preferences_runtime) = runtime_preferences {
        writeln!(report, "ipv6={}", preferences_runtime.ipv6).ok();
        writeln!(report, "unifiedDelay={}", preferences_runtime.unified_delay).ok();
        writeln!(report, "tcpConcurrent={}", preferences_runtime.tcp_concurrent).ok();
        writeln!(report, "findProcessMode={}", preferences_runtime.find_process_mode).ok();
        writeln!(report, "logLevel={}", preferences_runtime.log_level).ok();
    }
    if let Some(tun) = tun_preferences {
        writeln!(report, "tunStack={}", tun.stack).ok();
        writeln!(report, "tunStrictRoute={}", tun.strict_route).ok();
        writeln!(report, "tunAutoDetectInterface={}", tun.auto_detect_interface).ok();
        writeln!(report, "tunMtu={}", tun.mtu).ok();
    }
    writeln!(report, "autoCloseConnection={}", preferences.auto_close_connection).ok();
    writeln!(report, "autoDelayDetection={}", preferences.enable_auto_delay_detection).ok();
    writeln!(report, "latencyTimeoutMs={}", preferences.default_latency_timeout).ok();
    writeln!(report, "startPage={}", preferences.start_page).ok();
    if let Some(ports) = port_settings {
        writeln!(report, "allowLan={}", ports.allow_lan).ok();
        writeln!(report, "mixedPort={}", ports.mixed_port).ok();
        writeln!(report, "httpPort={}", ports.http_port).ok();
        writeln!(report, "socksPort={}", ports.socks_port).ok();
    }

    writeln!(report).ok();
    writeln!(report, "[Backup]").ok();
    writeln!(report, "webdavConfigured={}", backup_settings.webdav_configured()).ok();
    writeln!(report, "autoBackupSchedule={}", backup_settings.auto_schedule_enabled).ok();
    writeln!(report, "autoBackupOnChange={}", backup_settings.auto_on_change).ok();

    writeln!(report).ok();
    writeln!(report, "[Network]").ok();
    writeln!(
        report,
        "interfaces={}",
        network_interfaces
            .iter()
            .map(|interface| interface.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    )
    .ok();
    writeln!(report).ok();
    writeln!(
        report,
        "Sensitive configuration, URLs, credentials, IP addresses and raw logs are intentionally omitted."
    )
    .ok();
    Ok(report)
}

#[tauri::command]
async fn mobile_logs(lines: usize, state: State<'_, MobileState>) -> Result<String, String> {
    Ok(
        call_agent(&state.agent_socket, json!({ "op": "read_log", "lines": lines }))
            .await?
            .log
            .unwrap_or_default(),
    )
}

#[tauri::command]
async fn mobile_clear_logs(state: State<'_, MobileState>) -> Result<(), String> {
    call_agent(&state.agent_socket, json!({ "op": "clear_log" })).await?;
    Ok(())
}

async fn run_vpn_plugin<T, F>(operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| format!("Android VPN bridge task failed: {error}"))?
}

async fn vpn_status(vpn: AndroidVpn) -> Result<AndroidVpnStatus, String> {
    run_vpn_plugin(move || vpn.status()).await
}

async fn set_agent_vpn_coexistence(state: &MobileState, enable: bool) -> Result<(), String> {
    call_agent(
        &state.agent_socket,
        json!({ "op": "set_vpn_coexistence", "enable": enable }),
    )
    .await
    .map(|_| ())
}

#[tauri::command]
async fn mobile_system_proxy_status(vpn: State<'_, AndroidVpn>) -> Result<AndroidVpnStatus, String> {
    vpn_status(vpn.inner().clone()).await
}

#[tauri::command]
async fn mobile_set_system_proxy(
    enable: bool,
    state: State<'_, MobileState>,
    vpn: State<'_, AndroidVpn>,
) -> Result<AndroidVpnStatus, String> {
    let vpn = vpn.inner().clone();
    if !enable {
        let _ = set_agent_vpn_coexistence(&state, false).await;
        let stop = vpn.clone();
        let _ = run_vpn_plugin(move || stop.stop()).await?;
        for _ in 0..50 {
            let status = vpn_status(vpn.clone()).await?;
            if !status.active && status.phase != "starting" && status.phase != "stopping" {
                return Ok(status);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        return Err("Android VPN did not stop in time".into());
    }

    let (yaml, selected_map) = render_android_vpn_config(&state)?;
    let overrides = state.runtime_overrides.lock().map_err(|e| e.to_string())?.clone();
    let vpn_dir = state.app_data_dir.join("vpn-runtime");
    fs::create_dir_all(&vpn_dir).map_err(|e| format!("create Android VPN runtime directory: {e}"))?;
    fs::set_permissions(&vpn_dir, fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("chmod Android VPN runtime directory: {e}"))?;
    let config_path = vpn_dir.join("config.yaml");
    write_mode(&config_path, yaml.as_bytes(), 0o600)?;
    if !GEOSITE_GZ_BYTES.is_empty() {
        write_gzip_mode(&vpn_dir.join("GeoSite.dat"), GEOSITE_GZ_BYTES, 0o600)?;
    }

    let stack = match overrides.tun_stack.as_deref().unwrap_or("mixed") {
        "system" => "system",
        "gvisor" => "gvisor",
        "mixed" => "mixed",
        // The Android bridge currently exposes mihomo's system/gvisor/mixed
        // stacks. Keep the persisted root-only experimental stack from
        // breaking the non-root VPN path.
        _ => "mixed",
    }
    .to_owned();
    let mtu = overrides.tun_mtu.unwrap_or(1500).clamp(576, 65_535);
    let ipv6 = overrides.ipv6.unwrap_or(true);

    let args = AndroidVpnStart {
        config_path: config_path.to_string_lossy().into_owned(),
        selected_map,
        stack,
        mtu,
        ipv6,
    };
    let start = vpn.clone();
    if let Err(error) = run_vpn_plugin(move || start.start(args)).await {
        return Err(error);
    }

    let mut last = None;
    for _ in 0..100 {
        let status = vpn_status(vpn.clone()).await?;
        if status.active && status.phase == "running" {
            if state.agent_socket.exists()
                && let Err(error) = set_agent_vpn_coexistence(&state, true).await
            {
                let stop = vpn.clone();
                let _ = run_vpn_plugin(move || stop.stop()).await;
                let _ = set_agent_vpn_coexistence(&state, false).await;
                return Err(format!("Android VPN started but coexistence routing failed: {error}"));
            }
            return Ok(status);
        }
        if status.phase == "error" {
            return Err(status.detail);
        }
        last = Some(status);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let stop = vpn.clone();
    let _ = run_vpn_plugin(move || stop.stop()).await;
    let _ = set_agent_vpn_coexistence(&state, false).await;
    Err(format!(
        "Android VPN did not become ready: {}",
        last.map(|status| status.detail)
            .unwrap_or_else(|| "no status returned".into())
    ))
}

#[tauri::command]
async fn mobile_enable_tun(state: State<'_, MobileState>, vpn: State<'_, AndroidVpn>) -> Result<CoreSnapshot, String> {
    let vpn = vpn.inner().clone();
    let set_tun = vpn.clone();
    let switched = run_vpn_plugin(move || set_tun.set_tun(true)).await?;
    if !switched.active {
        return Err("Root TUN controller did not report an active TUN".into());
    }
    wait_for_tun_stable(&state).await?;
    if vpn_status(vpn.clone()).await.is_ok_and(|status| status.active)
        && let Err(error) = set_agent_vpn_coexistence(&state, true).await
    {
        let rollback = vpn.clone();
        let _ = run_vpn_plugin(move || rollback.set_tun(false)).await;
        return Err(format!("Root TUN started but VPN coexistence routing failed: {error}"));
    }
    core_client(&state)?.snapshot().await
}

#[tauri::command]
async fn mobile_disable_tun(
    state: State<'_, MobileState>,
    vpn: State<'_, AndroidVpn>,
) -> Result<RuntimeStatus, String> {
    let set_tun = vpn.inner().clone();
    let switched = run_vpn_plugin(move || set_tun.set_tun(false)).await?;
    if switched.active {
        return Err("Root TUN controller still reports an active TUN".into());
    }
    for _ in 0..30 {
        let status = runtime_status(&state).await;
        if status.core_running && status.core_connected && !status.transparent_active {
            return Ok(status);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err("Mihomo left transparent mode but the preview core did not become ready".into())
}

#[tauri::mobile_entry_point]
pub fn run() {
    tauri::Builder::default()
        .plugin(crate::android_vpn::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_http::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&app_data_dir)?;
            let run_dir = app_data_dir.join("run");
            fs::create_dir_all(&run_dir)?;
            fs::set_permissions(&run_dir, fs::Permissions::from_mode(0o700))?;
            let socket_context = probe_socket_context(&run_dir);
            let app_selinux_type = current_selinux_type();
            let path = app_data_dir.join("profiles.json");
            let store = Store::open(path).map_err(std::io::Error::other)?;
            let runtime_overrides_path = app_data_dir.join("runtime-preferences.json");
            let runtime_overrides = match fs::read(&runtime_overrides_path) {
                Ok(bytes) => serde_json::from_slice::<RuntimeOverrides>(&bytes).map_err(std::io::Error::other)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => RuntimeOverrides::default(),
                Err(error) => return Err(error.into()),
            };
            let mobile_preferences_path = app_data_dir.join("mobile-preferences.json");
            let mobile_preferences = match fs::read(&mobile_preferences_path) {
                Ok(bytes) => serde_json::from_slice::<MobilePreferences>(&bytes)
                    .map_err(std::io::Error::other)?
                    .normalized()
                    .map_err(std::io::Error::other)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => MobilePreferences::default(),
                Err(error) => return Err(error.into()),
            };
            let backup_settings_path = app_data_dir.join("backup-settings.json");
            let backup_settings = match fs::read(&backup_settings_path) {
                Ok(bytes) => serde_json::from_slice::<BackupSettings>(&bytes)
                    .map_err(std::io::Error::other)?
                    .normalized()
                    .map_err(std::io::Error::other)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => BackupSettings::default(),
                Err(error) => return Err(error.into()),
            };
            let client = build_http_client(25, None, false).map_err(std::io::Error::other)?;
            app.manage(MobileState {
                store: Mutex::new(store),
                runtime_overrides: Mutex::new(runtime_overrides),
                runtime_overrides_path,
                mobile_preferences: Mutex::new(mobile_preferences),
                mobile_preferences_path,
                backup_settings: Mutex::new(backup_settings),
                backup_settings_path,
                client,
                auto_update_attempts: Mutex::new(HashMap::new()),
                app_update_progress: Mutex::new(AppUpdateProgress::default()),
                agent_socket: run_dir.join("root-agent.sock"),
                controller_socket: run_dir.join("mihomo.sock"),
                socket_context,
                app_selinux_type,
                app_data_dir,
            });
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_secs(15)).await;
                loop {
                    let state = handle.state::<MobileState>();
                    run_due_profile_updates(&state).await;
                    run_due_auto_backup(&state).await;
                    drop(state);
                    tokio::time::sleep(Duration::from_secs(60)).await;
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            mobile_capabilities,
            mobile_profiles,
            mobile_inspect,
            mobile_import,
            mobile_import_nodes,
            mobile_activate,
            mobile_delete,
            mobile_delete_many,
            mobile_update_profile,
            mobile_reorder_profile,
            mobile_set_global_merge,
            mobile_set_profile_merge,
            mobile_set_profile_sequence,
            mobile_set_global_script,
            mobile_set_profile_script,
            mobile_subscribe,
            mobile_refresh,
            mobile_next_update_time,
            mobile_probe_root,
            mobile_runtime_status,
            mobile_boot_module_status,
            mobile_set_boot_module,
            mobile_runtime_config,
            mobile_bootstrap_runtime,
            mobile_start_preview_core,
            mobile_stop_preview_core,
            mobile_restart_core,
            mobile_core_upgrade,
            mobile_core_variant,
            mobile_set_core_variant,
            mobile_app_version,
            mobile_check_app_update,
            mobile_app_update_progress,
            mobile_download_app_update,
            mobile_install_app_update,
            mobile_open_external_url,
            mobile_create_local_backup,
            mobile_list_local_backups,
            mobile_restore_local_backup,
            mobile_delete_local_backup,
            mobile_export_local_backup,
            mobile_import_backup_text,
            mobile_backup_settings,
            mobile_set_backup_settings,
            mobile_test_webdav,
            mobile_create_webdav_backup,
            mobile_list_webdav_backups,
            mobile_restore_webdav_backup,
            mobile_delete_webdav_backup,
            mobile_core_snapshot,
            mobile_runtime_preferences,
            mobile_set_runtime_preferences,
            mobile_preferences,
            mobile_set_preferences,
            mobile_dns_override,
            mobile_set_dns_override,
            mobile_port_settings,
            mobile_set_port_settings,
            mobile_external_controller_settings,
            mobile_set_external_controller_settings,
            mobile_network_interfaces,
            mobile_tun_preferences,
            mobile_set_tun_preferences,
            mobile_select_proxy,
            mobile_set_proxy_chain,
            mobile_clear_proxy_chain,
            mobile_set_mode,
            mobile_connections,
            mobile_close_connection,
            mobile_close_all_connections,
            mobile_proxy_delay,
            mobile_proxy_providers,
            mobile_update_proxy_provider,
            mobile_rule_providers,
            mobile_update_rule_provider,
            mobile_flush_fakeip,
            mobile_flush_dns,
            mobile_update_geo,
            mobile_diagnostics,
            mobile_logs,
            mobile_clear_logs,
            mobile_system_proxy_status,
            mobile_set_system_proxy,
            mobile_enable_tun,
            mobile_disable_tun
        ])
        .run(tauri::generate_context!())
        .expect("Unable to start Clash Verge for Android");
}
