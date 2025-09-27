use anyhow::Result;
use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

use crate::models::*;

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_live_activity);

// initializes the Kotlin or Swift plugin classes
pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<LiveActivity<R>> {
    #[cfg(target_os = "android")]
    let handle = api.register_android_plugin(
        "com.astralsight.astrobox.plugin.live_activity",
        "ExamplePlugin",
    )?;
    #[cfg(target_os = "ios")]
    let handle = api.register_ios_plugin(init_plugin_live_activity)?;
    Ok(LiveActivity(handle))
}

/// Access to the live-activity APIs.
pub struct LiveActivity<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> LiveActivity<R> {
    pub fn create_live_activity(&self, payload: CreateLiveActivityRequest) -> Result<()> {
        self.0
            .run_mobile_plugin("createLiveActivity", payload)
            .map_err(Into::into)
    }

    pub fn update_live_activity(&self, payload: UpdateLiveActivityRequest) -> Result<()> {
        self.0
            .run_mobile_plugin("updateLiveActivity", payload)
            .map_err(Into::into)
    }

    pub fn remove_live_activity(&self) -> Result<()> {
        self.0
            .run_mobile_plugin("removeLiveActivity", ())
            .map_err(Into::into)
    }
}
