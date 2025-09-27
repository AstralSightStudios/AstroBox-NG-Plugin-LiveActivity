use anyhow::Result;
use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

#[cfg(target_os = "windows")]
#[path = "./win/implementation.rs"]
pub mod imp;

#[cfg(target_os = "macos")]
#[path = "./macos/implementation.rs"]
pub mod imp;

#[cfg(target_os = "linux")]
#[path = "./linux/implementation.rs"]
pub mod imp;

use imp::core;

use crate::models::*;

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<LiveActivity<R>> {
    Ok(LiveActivity(app.clone()))
}

/// Access to the live-activity APIs.
pub struct LiveActivity<R: Runtime>(AppHandle<R>);

impl<R: Runtime> LiveActivity<R> {
    pub fn create_live_activity(&self, payload: CreateLiveActivityRequest) -> Result<()> {
        core::create_live_activity(self, payload)?;
        Ok(())
    }

    pub fn update_live_activity(&self, payload: UpdateLiveActivityRequest) -> Result<()> {
        core::update_live_activity(self, payload)?;
        Ok(())
    }

    pub fn remove_live_activity(&self) -> Result<()> {
        core::remove_live_activity(self)?;
        Ok(())
    }
}
