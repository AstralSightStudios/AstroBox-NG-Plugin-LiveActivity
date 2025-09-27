pub mod core {
    use crate::models::*;
    use anyhow::{Context, Result};
    use std::sync::{Mutex, OnceLock};

    static CURRENT_TAG: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    fn current_tag() -> &'static Mutex<Option<String>> {
        CURRENT_TAG.get_or_init(|| Mutex::new(None))
    }

    pub fn create_live_activity(
        _self: &impl Sized,
        payload: CreateLiveActivityRequest,
    ) -> Result<()> {
        use win_toast_notify::{CropCircle, Duration, WinToastNotify};

        let (id, title, text, mut state, task_name, task_type, task_icon) =
            match payload.activity_content {
                ActivityContent::TaskQueue(t) => (
                    t.id,
                    t.title,
                    t.text,
                    t.state,
                    t.task_name,
                    t.task_type,
                    t.task_icon,
                ),
            };

        {
            let mut g = current_tag().lock().unwrap();
            *g = Some(id.clone());
        }

        let progress = state
            .remove("progress")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        let progress_text = state
            .remove("percent")
            .map(|p| format!("{}%", p))
            .unwrap_or_else(|| format!("{:.1}%", progress * 100.0));

        WinToastNotify::new()
            .set_duration(Duration::Long)
            .set_title(&title)
            .set_messages(vec![text.as_str()])
            .set_logo(&task_icon, CropCircle::True)
            // set_progress(tag, caption, status, value, value_text)
            .set_progress(&id, &task_name, &task_type, progress, &progress_text)
            .show()
            .map_err(|e| anyhow::anyhow!("{}", e))
            .context("Failed to show toast notification")?;

        Ok(())
    }

    pub fn update_live_activity(
        _self: &impl Sized,
        payload: UpdateLiveActivityRequest,
    ) -> Result<()> {
        use win_toast_notify::WinToastNotify;

        let tag = {
            let g = current_tag().lock().unwrap();
            g.as_ref()
                .cloned()
                .context("No active live activity to update")?
        };

        let mut p = payload
            .state
            .get("progress")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or_else(|| {
                payload
                    .state
                    .get("percent")
                    .and_then(|s| s.parse::<f32>().ok())
                    .map(|x| (x / 100.0) as f32)
                    .unwrap_or(0.0)
            });
        p = p.clamp(0.0, 1.0);

        if (p - 1.0).abs() < f32::EPSILON {
            WinToastNotify::progress_complete(None, &tag, &"传输完成", &"100%")
                .map_err(|e| anyhow::anyhow!("{}", e))
                .context("Failed to complete toast progress")?;
            let mut g = current_tag().lock().unwrap();
            *g = None;
        } else {
            let pct_text = if let Some(s) = payload.state.get("percent") {
                format!("{}%", s)
            } else {
                format!("{:.1}%", p * 100.0)
            };
            WinToastNotify::progress_update(None, &tag, p, &pct_text)
                .map_err(|e| anyhow::anyhow!("{}", e))
                .context("Failed to update toast progress")?;
        }

        Ok(())
    }

    pub fn remove_live_activity(_self: &impl Sized) -> Result<()> {
        use win_toast_notify::WinToastNotify;

        if let Some(tag) = { current_tag().lock().unwrap().clone() } {
            WinToastNotify::progress_complete(None, &tag, &"已结束", &"100%").ok();
            *current_tag().lock().unwrap() = None;
        }
        Ok(())
    }
}
