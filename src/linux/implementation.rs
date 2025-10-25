pub mod core {
    use crate::models::*;
    use anyhow::Result;

    pub fn create_live_activity(
        _self: &impl Sized,
        _payload: CreateLiveActivityRequest,
    ) -> Result<()> {
        corelib::bail_site!("Desktop platforms doesn't support live activity now.")
    }

    pub fn update_live_activity(
        _self: &impl Sized,
        _payload: UpdateLiveActivityRequest,
    ) -> Result<()> {
        corelib::bail_site!("Desktop platforms doesn't support live activity now.")
    }

    pub fn remove_live_activity(_self: &impl Sized) -> Result<()> {
        corelib::bail_site!("Desktop platforms doesn't support live activity now.")
    }
}
