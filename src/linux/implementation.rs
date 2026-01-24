pub mod core {
    use crate::models::*;
    use anyhow::Result;
    //返回ok防止爆炸
    pub fn create_live_activity(
        _self: &impl Sized,
        _payload: CreateLiveActivityRequest,
    ) -> Result<()> {
        Ok(())
    }

    pub fn update_live_activity(
        _self: &impl Sized,
        _payload: UpdateLiveActivityRequest,
    ) -> Result<()> {
        Ok(())
    }

    pub fn remove_live_activity(_self: &impl Sized) -> Result<()> {
        Ok(())
    }
}
