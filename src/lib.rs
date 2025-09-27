use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use models::*;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod error;
mod models;

pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::LiveActivity;
#[cfg(mobile)]
use mobile::LiveActivity;

pub trait LiveActivityExt<R: Runtime> {
    fn live_activity(&self) -> &LiveActivity<R>;
}

impl<R: Runtime, T: Manager<R>> crate::LiveActivityExt<R> for T {
    fn live_activity(&self) -> &LiveActivity<R> {
        self.state::<LiveActivity<R>>().inner()
    }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("live-activity")
        .setup(|app, api| {
            #[cfg(mobile)]
            let live_activity = mobile::init(app, api)?;
            #[cfg(desktop)]
            let live_activity = desktop::init(app, api)?;
            app.manage(live_activity);
            Ok(())
        })
        .build()
}
