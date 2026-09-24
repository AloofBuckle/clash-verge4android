use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{
    AppHandle, Manager, Wry,
    plugin::{Builder, PluginHandle, TauriPlugin},
};

const PLUGIN_IDENTIFIER: &str = "io.github.aloofbuckle.cv4android.vpn";

#[derive(Clone)]
pub struct AndroidVpn(PluginHandle<Wry>);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidVpnStatus {
    pub supported: bool,
    pub permission_granted: bool,
    pub active: bool,
    pub phase: String,
    pub detail: String,
    pub bridge_abi: i32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidVpnStart {
    pub config_path: String,
    pub selected_map: HashMap<String, String>,
    pub stack: String,
    pub mtu: u32,
    pub ipv6: bool,
}

impl AndroidVpn {
    pub fn status(&self) -> Result<AndroidVpnStatus, String> {
        self.0
            .run_mobile_plugin("status", ())
            .map_err(|error| error.to_string())
    }

    pub fn start(&self, args: AndroidVpnStart) -> Result<AndroidVpnStatus, String> {
        self.0
            .run_mobile_plugin("start", args)
            .map_err(|error| error.to_string())
    }

    pub fn stop(&self) -> Result<AndroidVpnStatus, String> {
        self.0.run_mobile_plugin("stop", ()).map_err(|error| error.to_string())
    }
}

pub fn init() -> TauriPlugin<Wry> {
    Builder::new("cv4a-vpn")
        .setup(|app: &AppHandle<Wry>, api| {
            let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "Cv4aVpnPlugin")?;
            app.manage(AndroidVpn(handle));
            Ok(())
        })
        .build()
}
