pub mod core {
    use crate::models::*;
    use anyhow::{Context, Result};
    use block2::RcBlock;
    use mac_notification_sys::{send_notification, set_application, Notification};
    use objc2::runtime::{AnyObject, Bool};
    use objc2::{class, msg_send};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNAuthorizationStatus, UNNotificationSettings,
        UNUserNotificationCenter,
    };
    use std::process::Command;
    use std::sync::{Mutex, OnceLock};
    use std::time::Duration;

    static CURRENT_TAG: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    static CURRENT_META: OnceLock<Mutex<Option<Meta>>> = OnceLock::new();
    static NOTIFY_SETTINGS_HINT_SHOWN: OnceLock<()> = OnceLock::new();
    static NOTIFY_PERMISSION_CHECKED: OnceLock<()> = OnceLock::new();

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

    fn clear_all_notifications() {
        unsafe {
            let center: *mut AnyObject =
                msg_send![class!(NSUserNotificationCenter), defaultUserNotificationCenter];
            if center.is_null() {
                return;
            }
            let _: () = msg_send![center, removeAllDeliveredNotifications];
        }
    }

    fn open_notification_settings_once() {
        if NOTIFY_SETTINGS_HINT_SHOWN.get().is_some() {
            return;
        }
        let _ = NOTIFY_SETTINGS_HINT_SHOWN.set(());

        // 优先新设置页 URI，失败后回退到旧 URI。
        let _ = Command::new("open")
            .arg("x-apple.systempreferences:com.apple.Notifications-Settings.extension")
            .status()
            .or_else(|_| {
                Command::new("open")
                    .arg("x-apple.systempreferences:com.apple.preference.notifications")
                    .status()
            });
    }

    fn current_notification_auth_status(timeout: Duration) -> Option<UNAuthorizationStatus> {
        let center = UNUserNotificationCenter::currentNotificationCenter();
        let (tx, rx) = std::sync::mpsc::channel();
        let callback = RcBlock::new(move |settings: std::ptr::NonNull<UNNotificationSettings>| {
            // `UNNotificationSettings` 由系统回调持有，读取授权状态后立刻返回。
            let status = unsafe { settings.as_ref().authorizationStatus() };
            let _ = tx.send(status);
        });
        center.getNotificationSettingsWithCompletionHandler(&callback);
        rx.recv_timeout(timeout).ok()
    }

    fn request_notification_auth(timeout: Duration) -> Option<bool> {
        let center = UNUserNotificationCenter::currentNotificationCenter();
        let options = UNAuthorizationOptions::Alert
            | UNAuthorizationOptions::Sound
            | UNAuthorizationOptions::Badge;
        let (tx, rx) = std::sync::mpsc::channel();
        let callback = RcBlock::new(move |granted: Bool, _err| {
            let _ = tx.send(granted.as_bool());
        });
        center.requestAuthorizationWithOptions_completionHandler(options, &callback);
        rx.recv_timeout(timeout).ok()
    }

    fn ensure_notification_permission_once() {
        if NOTIFY_PERMISSION_CHECKED.get().is_some() {
            return;
        }
        let _ = NOTIFY_PERMISSION_CHECKED.set(());

        match current_notification_auth_status(Duration::from_secs(2)) {
            Some(UNAuthorizationStatus::Authorized)
            | Some(UNAuthorizationStatus::Provisional)
            | Some(UNAuthorizationStatus::Ephemeral) => {}
            Some(UNAuthorizationStatus::NotDetermined) => {
                // 主动触发系统授权弹窗；若用户拒绝或系统未返回结果，则引导到设置页。
                if !request_notification_auth(Duration::from_secs(4)).unwrap_or(false) {
                    open_notification_settings_once();
                }
            }
            Some(UNAuthorizationStatus::Denied) | None => {
                open_notification_settings_once();
            }
            Some(_) => {}
        }
    }

    fn send_notification_checked(
        title: &str,
        subtitle: Option<&str>,
        message: &str,
        opts: Option<&Notification>,
        err_ctx: &str,
    ) -> Result<()> {
        ensure_notification_permission_once();
        match send_notification(title, subtitle, message, opts) {
            Ok(_) => Ok(()),
            Err(err) => {
                open_notification_settings_once();
                corelib::bail_site!("{}: {} (请在系统设置>通知中开启 AstroBox)", err_ctx, err);
            }
        }
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
        send_notification_checked(
            &title,
            subtitle,
            &message,
            Some(&opts),
            "Failed to send macOS notification",
        )?;

        Ok(())
    }

    pub fn update_live_activity(
        _self: &impl Sized,
        payload: UpdateLiveActivityRequest,
    ) -> Result<()> {
        {
            let g = current_tag().lock().unwrap();
            if g.is_none() {
                corelib::bail_site!("No active live activity to update");
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
            send_notification_checked(
                &meta.title,
                subtitle,
                &message,
                Some(&opts),
                "Failed to send completion notification",
            )?;

            // 给系统一点时间展示完成通知，然后再清空通知中心，避免“看起来完全没通知”。
            std::thread::spawn(|| {
                std::thread::sleep(Duration::from_secs(3));
                clear_all_notifications();
            });
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
            send_notification_checked(
                &meta.title,
                subtitle,
                &message,
                Some(&opts),
                "Failed to send progress notification",
            )?;
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
