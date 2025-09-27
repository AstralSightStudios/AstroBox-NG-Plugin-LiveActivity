pub mod core {
    use crate::models::*;
    use anyhow::{bail, Context, Result};
    use mac_notification_sys::{send_notification, set_application, Notification};
    use std::sync::{Mutex, OnceLock};

    static CURRENT_TAG: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    static CURRENT_META: OnceLock<Mutex<Option<Meta>>> = OnceLock::new();

    #[derive(Clone)]
    struct Meta {
        title: String,
        text: String,
        icon: Option<String>,
        bundle_id: Option<String>,
    }

    fn current_tag() -> &'static Mutex<Option<String>> {
        CURRENT_TAG.get_or_init(|| Mutex::new(None))
    }
    fn current_meta() -> &'static Mutex<Option<Meta>> {
        CURRENT_META.get_or_init(|| Mutex::new(None))
    }

    pub fn create_live_activity(
        _self: &impl Sized,
        payload: CreateLiveActivityRequest,
    ) -> Result<()> {
        let (id, title, text, mut state, task_name, task_type) = match payload.activity_content {
            ActivityContent::TaskQueue(t) => {
                (t.id, t.title, t.text, t.state, t.task_name, t.task_type)
            }
        };

        let bundle_id = state.remove("bundle_id");
        if let Some(ref bid) = bundle_id {
            set_application(bid).ok();
        }
        let icon = state.remove("logo");
        let progress = state
            .remove("progress")
            .and_then(|s| s.parse::<f32>().ok())
            .or_else(|| {
                state
                    .get("percent")
                    .and_then(|s| s.parse::<f32>().ok())
                    .map(|x| x / 100.0)
            })
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        let progress_text = state
            .remove("percent")
            .map(|p| format!("{}%", p))
            .unwrap_or_else(|| format!("{:.1}%", progress * 100.0));

        {
            *current_tag().lock().unwrap() = Some(id);
        }
        {
            *current_meta().lock().unwrap() = Some(Meta {
                title: title.clone(),
                text: text.clone(),
                icon: icon.clone(),
                bundle_id: bundle_id.clone(),
            });
        }

        let subtitle_opt = Some(task_type);
        let subtitle = subtitle_opt.as_deref();
        let message = format!("{} · {} — {}", text, task_name, progress_text);

        let mut opts = Notification::new();
        if let Some(ref path) = icon {
            opts.app_icon(path);
        }
        send_notification(&title, subtitle, &message, Some(&opts))
            .context("Failed to send macOS notification")?;

        Ok(())
    }

    pub fn update_live_activity(
        _self: &impl Sized,
        payload: UpdateLiveActivityRequest,
    ) -> Result<()> {
        {
            let g = current_tag().lock().unwrap();
            if g.is_none() {
                bail!("No active live activity to update");
            }
        }
        let meta = {
            let m = current_meta().lock().unwrap();
            m.clone().context("No meta to update")?
        };
        if let Some(ref bid) = meta.bundle_id {
            set_application(bid).ok();
        }

        let mut p = payload
            .state
            .get("progress")
            .and_then(|s| s.parse::<f32>().ok())
            .or_else(|| {
                payload
                    .state
                    .get("percent")
                    .and_then(|s| s.parse::<f32>().ok())
                    .map(|x| x / 100.0)
            })
            .unwrap_or(0.0);
        p = p.clamp(0.0, 1.0);

        let mut opts = Notification::new();
        if let Some(ref path) = meta.icon {
            opts.app_icon(path);
        }

        if (p - 1.0).abs() < f32::EPSILON {
            let subtitle = Some("传输完成");
            let message = format!("{} — 100%", meta.text);
            send_notification(&meta.title, subtitle, &message, Some(&opts))
                .context("Failed to send completion notification")?;
            *current_tag().lock().unwrap() = None;
            *current_meta().lock().unwrap() = None;
        } else {
            let pct_text = if let Some(s) = payload.state.get("percent") {
                format!("{}%", s)
            } else {
                format!("{:.1}%", p * 100.0)
            };
            let subtitle = Some("传输中...");
            let message = format!("{} — {}", meta.text, pct_text);
            send_notification(&meta.title, subtitle, &message, Some(&opts))
                .context("Failed to send progress notification")?;
        }

        Ok(())
    }

    pub fn remove_live_activity(_self: &impl Sized) -> Result<()> {
        let exists = { current_tag().lock().unwrap().is_some() };
        if !exists {
            return Ok(());
        }

        if let Some(meta) = { current_meta().lock().unwrap().clone() } {
            if let Some(ref bid) = meta.bundle_id {
                set_application(bid).ok();
            }
            let mut opts = Notification::new();
            if let Some(ref path) = meta.icon {
                opts.app_icon(path);
            }
            let subtitle = Some("已结束");
            let message = meta.text.clone();
            send_notification(&meta.title, subtitle, &message, Some(&opts)).ok();
        }

        *current_tag().lock().unwrap() = None;
        *current_meta().lock().unwrap() = None;
        Ok(())
    }
}
