pub mod core {
    //! Linux implementation of live activities on top of the freedesktop
    //! Notifications D-Bus API, using the pure-Rust `notify-rust` client
    //! (zbus backend, no `notify-send` subprocess).
    //!
    //! Each in-progress activity maps to one persistent notification
    //! carrying the standard `value` int hint, which Dunst and KDE render as
    //! a real progress bar. The percentage is always part of the body text as
    //! well, so daemons without value-hint support (GNOME, ...) still show
    //! meaningful progress. Updates reuse the daemon-side notification id
    //! (`replaces_id`) so the bar updates in place, and every freshly
    //! returned id is written back to the registry to survive daemon restarts.
    //!
    //! Concurrency: each public entry point performs ALL of its registry
    //! reads/writes AND its D-Bus calls inside a single critical section, so
    //! a concurrent remove can never interleave with an update's
    //! read-modify-write (which would resurrect a removed activity).
    //!
    //! Crash safety: progress notifications use a bounded expire timeout that
    //! is refreshed on every replace; conforming daemons reset the timer on
    //! replaces_id updates. If this process dies, the last notification simply
    //! expires instead of lingering forever (the fd.o protocol offers no way
    //! to enumerate or close notifications from a previous session).

    use crate::models::*;
    use anyhow::Result;
    use notify_rust::{Hint, Notification, Timeout};
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    /// Expire timeout for in-progress notifications (ms). Refreshed on every
    /// replace, so it only fires if this process stops updating — turning a
    /// crash into a self-cleaning orphan instead of a stuck permanent bar.
    const PROGRESS_EXPIRE_MS: u32 = 60_000;

    /// Registry of in-progress activities: activity_id -> D-Bus notification id (u32).
    static ACTIVE_NOTIFS: OnceLock<Mutex<HashMap<String, u32>>> = OnceLock::new();

    /// Per-activity metadata captured at create time (mirrors the macOS
    /// `Meta`), so `update_live_activity` can re-render title/body/icon
    /// without the full create payload.
    static ACTIVITY_META: OnceLock<Mutex<HashMap<String, Meta>>> = OnceLock::new();
    #[derive(Clone)]
    struct Meta {
        title: String,
        text: String,
        icon: Option<String>,
    }

    fn active_notifs() -> &'static Mutex<HashMap<String, u32>> {
        ACTIVE_NOTIFS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    fn activity_meta() -> &'static Mutex<HashMap<String, Meta>> {
        ACTIVITY_META.get_or_init(|| Mutex::new(HashMap::new()))
    }

    /// Post (or replace) the persistent in-progress notification.
    ///
    /// - `subtitle`: informational only; notify-rust drops it on XDG targets
    ///   (not part of the freedesktop Notify call), so `message` must carry
    ///   the state the user needs to see.
    /// - `message`: body text; must already contain the percentage ("45%")
    ///   as fallback for daemons without value-hint support.
    /// - `replaces_id`: daemon-side id to refresh in place, or 0 to post anew.
    ///
    /// Returns the notification id handed out by the daemon (always store it
    /// back into [`ACTIVE_NOTIFS`]).
    fn show_progress(
        title: &str,
        icon: Option<&str>,
        subtitle: &str,
        progress: f32,
        message: &str,
        replaces_id: u32,
    ) -> notify_rust::error::Result<u32> {
        let percent = (progress.clamp(0.0, 1.0) * 100.0).round() as i32;

        let mut notification = Notification::new();
        notification
            .summary(title)
            .subtitle(subtitle)
            .body(message)
            // Standard "value" hint (int32): rendered as a progress bar by
            // Dunst/KDE; ignored gracefully elsewhere.
            .hint(Hint::CustomInt("value".to_owned(), percent))
            .timeout(Timeout::Milliseconds(PROGRESS_EXPIRE_MS));
        if let Some(path) = icon {
            // app_icon accepts absolute file paths per the fd.o spec.
            notification.icon(path);
        }
        if replaces_id != 0 {
            // Replace the known notification instead of stacking a new one.
            // Conforming daemons also reset the expire timer on replace, so
            // the bounded lifetime above only fires once updates stop.
            notification.id(replaces_id);
        }

        Ok(notification.show()?.id())
    }

    /// Post a transient completion notification that auto-dismisses (~5s),
    /// first closing the previous persistent notification. Closing errors are
    /// ignored: a stale id (daemon restart) simply means there is nothing to
    /// close.
    fn finish_progress(
        title: &str,
        icon: Option<&str>,
        message: &str,
        previous_notif_id: Option<u32>,
    ) -> notify_rust::error::Result<u32> {
        if let Some(prev_id) = previous_notif_id.filter(|id| *id != 0) {
            close_notification_ignoring_errors(prev_id);
        }

        let mut notification = Notification::new();
        notification
            .summary(title)
            .body(message)
            // Transient: bypass server persistence (history/tray).
            .hint(Hint::Transient(true))
            .timeout(Timeout::Milliseconds(5000));
        if let Some(path) = icon {
            notification.icon(path);
        }

        Ok(notification.show()?.id())
    }

    /// Close a notification by daemon-side id, ignoring all errors.
    ///
    /// notify-rust 4.x exposes no free `close_notification(id)` function —
    /// closing is normally done through the `NotificationHandle`, which we do
    /// not keep around. Instead we post a blank notification replacing the
    /// old one and immediately close it again, which removes the original on
    /// conforming daemons while leaving nothing persistent behind.
    fn close_notification_ignoring_errors(id: u32) {
        let _ = Notification::new()
            .summary("")
            .body("")
            .id(id)
            // Self-dismissing in case CloseNotification fails.
            .timeout(Timeout::Milliseconds(1))
            .show()
            .map(|handle| handle.close());
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

        // bundle_id is a macOS-only concept; irrelevant on Linux.
        let _ = state.remove("bundle_id");
        let icon = state.remove("logo");
        // `progress` (0.0–1.0 float string) takes precedence over `percent`
        // (0–100 numeric string); clamp to [0, 1]. Mirrors the macOS impl.
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
            .map(|p| format!("{p}%"))
            .unwrap_or_else(|| format!("{:.1}%", progress * 100.0));

        let replaces_id = {
            let registry = active_notifs().lock().unwrap();
            registry.get(&id).copied().unwrap_or(0)
        };

        // Remember metadata so later update_live_activity(activity_id) calls
        // can rebuild the notification without the full create payload.
        //
        // Lock ordering note: the two maps are only ever locked notifs-first,
        // and every public entry point holds them across its whole read +
        // D-Bus call + write sequence, so a concurrent remove can never
        // interleave with an update's read-modify-write (which would let a
        // removed activity be resurrected by a late id write-back).
        {
            let mut metas = activity_meta().lock().unwrap();
            metas.insert(
                id.clone(),
                Meta {
                    title: title.clone(),
                    text: text.clone(),
                    icon: icon.clone(),
                },
            );
        }

        let message = format!("{text} · {task_name} — {progress_text}");
        let mut registry = active_notifs().lock().unwrap();
        let notif_id = match show_progress(
            &title,
            icon.as_deref(),
            &task_type,
            progress,
            &message,
            replaces_id,
        ) {
            Ok(notif_id) => notif_id,
            Err(err) => {
                // Don't leave orphaned metadata behind a failed post.
                drop(registry);
                activity_meta().lock().unwrap().remove(&id);
                return Err(corelib::anyhow_site!(
                    "Failed to show Linux notification: {err} \
                     (请检查是否运行着通知守护进程，如 Dunst/KStatusNotifier/GNOME Shell)"
                ));
            }
        };

        // Every Notify() returns a fresh u32 — always write it back.
        registry.insert(id, notif_id);

        Ok(())
    }

    pub fn update_live_activity(
        _self: &impl Sized,
        payload: UpdateLiveActivityRequest,
    ) -> Result<()> {
        let previous_notif_id = {
            let registry = active_notifs().lock().unwrap();
            match registry.get(&payload.activity_id).copied() {
                Some(id) => id,
                None => {
                    // Unknown id: nothing to update, stay silent-but-logged.
                    log::info!(
                        "live-activity(linux): update for unknown activity_id '{}', ignoring",
                        payload.activity_id
                    );
                    return Ok(());
                }
            }
        };

        let meta = {
            let metas = activity_meta().lock().unwrap();
            match metas.get(&payload.activity_id) {
                Some(meta) => meta.clone(),
                None => {
                    // Registry entry without metadata should not happen;
                    // degrade gracefully instead of failing the update.
                    log::warn!(
                        "live-activity(linux): no stored meta for activity_id '{}', using fallback",
                        payload.activity_id
                    );
                    Meta {
                        title: payload.activity_id.clone(),
                        text: String::new(),
                        icon: None,
                    }
                }
            }
        };

        // Same precedence rules as create/macOS: `progress` beats `percent`.
        let progress = payload
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
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);

        // Hold the notifs lock across the whole D-Bus call so a concurrent
        // remove cannot slip between our read and write-back (TOCTOU).
        let mut registry = active_notifs().lock().unwrap();
        if (progress - 1.0).abs() < f32::EPSILON {
            // Completion: transient toast, persistent bar goes away.
            let message = format!("{} — 100%", meta.text);
            finish_progress(
                &meta.title,
                meta.icon.as_deref(),
                &message,
                Some(previous_notif_id),
            )
            .map_err(|err| {
                corelib::anyhow_site!("Failed to send Linux completion notification: {err}")
            })?;

            registry.remove(&payload.activity_id);
            drop(registry);
            activity_meta().lock().unwrap().remove(&payload.activity_id);
        } else {
            let pct_text = if let Some(p) = payload.state.get("percent") {
                format!("{p}%")
            } else {
                format!("{:.1}%", progress * 100.0)
            };
            let message = format!("{} — {}", meta.text, pct_text);
            let notif_id = show_progress(
                &meta.title,
                meta.icon.as_deref(),
                "传输中...",
                progress,
                &message,
                previous_notif_id,
            )
            .map_err(|err| {
                corelib::anyhow_site!("Failed to update Linux progress notification: {err}")
            })?;

            // Store the freshly returned id: heals stale ids after a
            // notification-daemon restart (unknown replaces_id comes back as
            // a brand-new id).
            registry.insert(payload.activity_id, notif_id);
        }

        Ok(())
    }

    pub fn remove_live_activity(
        _self: &impl Sized,
        payload: RemoveLiveActivityRequest,
    ) -> Result<()> {
        // Remove under the lock, then close while still holding it: a
        // concurrent update therefore either sees the entry gone (no-op) or
        // completes fully before our close — never in between.
        let previous_notif_id = {
            let mut registry = active_notifs().lock().unwrap();
            match registry.remove(&payload.activity_id) {
                Some(id) => {
                    activity_meta().lock().unwrap().remove(&payload.activity_id);
                    id
                }
                None => {
                    log::info!(
                        "live-activity(linux): remove for unknown activity_id '{}', ignoring",
                        payload.activity_id
                    );
                    return Ok(());
                }
            }
        };

        close_notification_ignoring_errors(previous_notif_id);

        Ok(())
    }
}
