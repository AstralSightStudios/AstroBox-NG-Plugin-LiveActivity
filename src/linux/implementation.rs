pub mod core {
    use crate::models::*;
    use anyhow::{bail, Result};

    pub fn create_live_activity(
        _self: &impl Sized,
        _payload: CreateLiveActivityRequest,
    ) -> Result<()> {
        bail!("Desktop platforms doesn't support live activity now.")
    }

    pub fn update_live_activity(
        _self: &impl Sized,
        _payload: UpdateLiveActivityRequest,
    ) -> Result<()> {
        bail!("Desktop platforms doesn't support live activity now.")
    }

    pub fn remove_live_activity(_self: &impl Sized) -> Result<()> {
        bail!("Desktop platforms doesn't support live activity now.")
    }
}
