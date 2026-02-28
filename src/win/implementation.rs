pub mod core {
    use crate::models::*;
    use anyhow::{Context, Result};
    use std::sync::{Mutex, OnceLock};
    use std::time::Duration;
    use windows::{
        core::{GUID, HSTRING, HRESULT, Interface, PCWSTR, PWSTR, PROPVARIANT},
        Data::Xml::Dom::XmlDocument,
        Foundation::{DateTime, IReference, PropertyValue},
        UI::Notifications::{
            NotificationData, ToastNotification, ToastNotificationManager, ToastNotifier,
        },
        Win32::{
            Foundation::WIN32_ERROR,
            Graphics::Gdi::{
                DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO,
                BITMAPINFOHEADER, DIB_RGB_COLORS,
            },
            System::Com::{
                CoCreateInstance, CoInitializeEx, CoTaskMemAlloc, CoTaskMemFree, IPersistFile,
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED,
            },
            System::Registry::{
                RegCloseKey, RegCreateKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
                HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, KEY_WOW64_64KEY,
                REG_OPTION_NON_VOLATILE, REG_SZ, REG_VALUE_TYPE,
            },
            UI::Shell::{
                ExtractIconExW, IShellLinkW, ShellLink, SHGetKnownFolderPath, FOLDERID_LocalAppData,
                FOLDERID_Programs, KF_FLAG_DEFAULT, SetCurrentProcessExplicitAppUserModelID,
            },
            UI::Shell::PropertiesSystem::{IPropertyStore, PROPERTYKEY},
            UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO},
        },
    };
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::path::PathBuf;

    static CURRENT_TAG: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    static CURRENT_META: OnceLock<Mutex<Option<Meta>>> = OnceLock::new();
    static WINRT_INIT: OnceLock<()> = OnceLock::new();
    const FIXED_AUMID: &str = "moe.astralsight.astrobox";
    fn current_tag() -> &'static Mutex<Option<String>> {
        CURRENT_TAG.get_or_init(|| Mutex::new(None))
    }
    fn current_meta() -> &'static Mutex<Option<Meta>> {
        CURRENT_META.get_or_init(|| Mutex::new(None))
    }

    #[derive(Clone)]
    struct Meta {
        title: String,
        text: String,
        task_name: String,
        task_type: String,
    }

    fn to_wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    fn path_to_file_uri(path: &std::path::Path) -> String {
        let mut s = String::from("file:///");
        s.push_str(&path.to_string_lossy().replace('\\', "/"));
        s
    }

    fn escape_xml(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    fn pwstr_to_string(pwstr: PWSTR) -> String {
        if pwstr.0.is_null() {
            return String::new();
        }
        unsafe {
            let mut len = 0usize;
            while *pwstr.0.add(len) != 0 {
                len += 1;
            }
            let slice = std::slice::from_raw_parts(pwstr.0, len);
            String::from_utf16_lossy(slice)
        }
    }

    fn resolve_app_id() -> Option<String> {
        Some(FIXED_AUMID.to_string())
    }

    fn set_process_app_id(app_id: &str) {
        let app_id_w = to_wide(app_id);
        let _ = unsafe { SetCurrentProcessExplicitAppUserModelID(PCWSTR(app_id_w.as_ptr())) };
    }

    fn extract_exe_icon_png(exe_path: &str) -> Result<Option<String>> {
        let exe = std::path::Path::new(exe_path);
        if !exe.exists() {
            return Ok(None);
        }
        let folder_ptr = unsafe { SHGetKnownFolderPath(&FOLDERID_LocalAppData, KF_FLAG_DEFAULT, None)? };
        let local_dir = pwstr_to_string(folder_ptr);
        unsafe { CoTaskMemFree(Some(folder_ptr.0 as _)) };
        if local_dir.is_empty() {
            return Ok(None);
        }
        let cache_dir = std::path::Path::new(&local_dir).join("AstroBox").join("icons");
        std::fs::create_dir_all(&cache_dir)?;
        let cache_path = cache_dir.join("toast_app_icon.png");
        if cache_path.exists() {
            return Ok(Some(path_to_file_uri(&cache_path)));
        }

        let exe_w = to_wide(exe_path);
        let mut large = HICON::default();
        let count = unsafe { ExtractIconExW(PCWSTR(exe_w.as_ptr()), 0, Some(&mut large), None, 1) };
        if count == 0 || large.0.is_null() {
            return Ok(None);
        }

        let mut info = ICONINFO::default();
        unsafe { GetIconInfo(large, &mut info)? };
        let mut bitmap = BITMAP::default();
        let bytes = unsafe {
            GetObjectW(info.hbmColor, std::mem::size_of::<BITMAP>() as i32, Some((&mut bitmap as *mut BITMAP).cast()))
        };
        if bytes == 0 || bitmap.bmWidth == 0 || bitmap.bmHeight == 0 {
            unsafe { let _ = DeleteObject(info.hbmColor); let _ = DeleteObject(info.hbmMask); let _ = DestroyIcon(large); };
            return Ok(None);
        }

        let width = bitmap.bmWidth as u32;
        let height = bitmap.bmHeight as u32;
        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: bitmap.bmWidth,
            biHeight: -bitmap.bmHeight,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: 0,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        };
        let mut buffer = vec![0u8; (width * height * 4) as usize];
        let hdc = unsafe { GetDC(None) };
        let lines = unsafe {
            GetDIBits(
                hdc,
                info.hbmColor,
                0,
                height,
                Some(buffer.as_mut_ptr().cast()),
                &mut bmi,
                DIB_RGB_COLORS,
            )
        };
        unsafe {
            ReleaseDC(None, hdc);
            let _ = DeleteObject(info.hbmColor);
            let _ = DeleteObject(info.hbmMask);
            let _ = DestroyIcon(large);
        }
        if lines == 0 {
            return Ok(None);
        }

        for chunk in buffer.chunks_exact_mut(4) {
            let b = chunk[0];
            let r = chunk[2];
            chunk[0] = r;
            chunk[2] = b;
        }
        let image = image::RgbaImage::from_raw(width, height, buffer).ok_or_else(|| anyhow::anyhow!("Invalid icon buffer"))?;
        image.save(&cache_path)?;
        Ok(Some(path_to_file_uri(&cache_path)))
    }

    fn reg_set_sz(hkey: HKEY, name: &str, value: &str) -> Result<()> {
        let name_w = to_wide(name);
        let value_w = to_wide(value);
        let bytes = unsafe {
            std::slice::from_raw_parts(value_w.as_ptr() as *const u8, value_w.len() * 2)
        };
        let status = unsafe { RegSetValueExW(hkey, PCWSTR(name_w.as_ptr()), 0, REG_SZ, Some(bytes)) };
        if status != WIN32_ERROR(0) {
            corelib::bail_site!("Registry write failed: {}", status.0);
        }
        Ok(())
    }

    fn reg_get_sz(hkey: HKEY, name: &str) -> Result<Option<String>> {
        let name_w = to_wide(name);
        let mut value_type = REG_VALUE_TYPE(0);
        let mut size: u32 = 0;
        let status = unsafe {
            RegQueryValueExW(
                hkey,
                PCWSTR(name_w.as_ptr()),
                None,
                Some(&mut value_type),
                None,
                Some(&mut size),
            )
        };
        if status != WIN32_ERROR(0) || size == 0 {
            return Ok(None);
        }
        let mut buf = vec![0u8; size as usize];
        let status = unsafe {
            RegQueryValueExW(
                hkey,
                PCWSTR(name_w.as_ptr()),
                None,
                Some(&mut value_type),
                Some(buf.as_mut_ptr()),
                Some(&mut size),
            )
        };
        if status != WIN32_ERROR(0) || value_type != REG_SZ {
            return Ok(None);
        }
        let u16_len = (size as usize / 2).min(buf.len() / 2);
        let u16_slice = unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u16, u16_len) };
        let value = String::from_utf16_lossy(u16_slice).trim_end_matches('\0').to_string();
        Ok(Some(value))
    }

    fn ensure_app_id_registry(app_id: &str, display_name: &str, icon_value: &str) -> Result<()> {
        let subkey = format!("Software\\Classes\\AppUserModelId\\{}", app_id);
        let subkey_w = to_wide(&subkey);
        let mut hkey = HKEY::default();
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey_w.as_ptr()),
                0,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE | KEY_QUERY_VALUE | KEY_WOW64_64KEY,
                None,
                &mut hkey,
                None,
            )
        };
        if status != WIN32_ERROR(0) {
            corelib::bail_site!("Registry create failed: {}", status.0);
        }
        let result = (|| {
            reg_set_sz(hkey, "DisplayName", display_name)?;
            reg_set_sz(hkey, "IconUri", icon_value)?;
            let display = reg_get_sz(hkey, "DisplayName")?.unwrap_or_default();
            let icon = reg_get_sz(hkey, "IconUri")?.unwrap_or_default();
            if display.trim().is_empty() || icon.trim().is_empty() {
                corelib::bail_site!(
                    "Registry verify failed: DisplayName='{}', IconUri='{}'",
                    display,
                    icon
                );
            }
            Ok(())
        })();
        unsafe { let _ = RegCloseKey(hkey); };
        result
    }

    fn ensure_shortcut_inner(app_id: &str) -> Result<()> {
        let exe = std::env::current_exe()?;
        let exe_dir = exe.parent().unwrap_or_else(|| std::path::Path::new("")).to_path_buf();
        let exe_str = exe.to_string_lossy().to_string();
        let exe_dir_str = exe_dir.to_string_lossy().to_string();
        let exe_w = to_wide(&exe_str);
        let exe_dir_w = to_wide(&exe_dir_str);

        let folder_ptr = unsafe { SHGetKnownFolderPath(&FOLDERID_Programs, KF_FLAG_DEFAULT, None)? };
        let programs_dir = pwstr_to_string(folder_ptr);
        unsafe { CoTaskMemFree(Some(folder_ptr.0 as _)) };
        if programs_dir.is_empty() {
            corelib::bail_site!("Failed to resolve Start Menu programs path");
        }

        let shortcut_name = "AstroBox".to_string();
        let shortcut_path = PathBuf::from(programs_dir).join(format!("{}.lnk", shortcut_name));
        let shortcut_w = to_wide(&shortcut_path.to_string_lossy());

        let shell_link: IShellLinkW =
            unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)? };
        unsafe {
            shell_link.SetPath(PCWSTR(exe_w.as_ptr()))?;
            shell_link.SetWorkingDirectory(PCWSTR(exe_dir_w.as_ptr()))?;
            shell_link.SetIconLocation(PCWSTR(exe_w.as_ptr()), 0)?;
        }

        let store: IPropertyStore = shell_link.cast()?;
        let app_id_w = to_wide(app_id);
        let pv = propvariant_from_string(&app_id_w)?;
        let app_id_key = PROPERTYKEY {
            fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
            pid: 5,
        };
        unsafe {
            store.SetValue(&app_id_key, &pv)?;
            store.Commit()?;
        }

        let persist: IPersistFile = shell_link.cast()?;
        unsafe {
            persist.Save(PCWSTR(shortcut_w.as_ptr()), true)?;
        }

        Ok(())
    }

    fn ensure_shortcut(app_id: &str) -> Result<()> {
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if hr.is_ok() {
            ensure_shortcut_inner(app_id)
        } else if hr == HRESULT(0x80010106u32 as i32) {
            let app_id = app_id.to_string();
            let handle = std::thread::spawn(move || {
                let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
                ensure_shortcut_inner(&app_id)
            });
            match handle.join() {
                Ok(result) => result,
                Err(_) => corelib::bail_site!("Failed to create shortcut"),
            }
        } else {
            Err(windows::core::Error::from(hr).into())
        }
    }

    fn propvariant_from_string(value: &[u16]) -> Result<PROPVARIANT> {
        let bytes = value.len() * 2;
        let mem = unsafe { CoTaskMemAlloc(bytes) } as *mut u16;
        if mem.is_null() {
            corelib::bail_site!("Failed to allocate memory for AppUserModelID");
        }
        unsafe {
            std::ptr::copy_nonoverlapping(value.as_ptr(), mem, value.len());
        }
        let mut raw: windows::core::imp::PROPVARIANT = unsafe { std::mem::zeroed() };
        raw.Anonymous.Anonymous.vt = 31;
        raw.Anonymous.Anonymous.Anonymous.pwszVal = mem;
        Ok(unsafe { PROPVARIANT::from_raw(raw) })
    }

    fn create_notifier() -> Result<ToastNotifier> {
        if WINRT_INIT.get().is_none() {
            let _ = WINRT_INIT.set(());
            let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        }
        if let Some(app_id) = resolve_app_id() {
            set_process_app_id(&app_id);
            if let Ok(exe) = std::env::current_exe() {
                let exe_str = exe.to_string_lossy().to_string();
                let icon_value = extract_exe_icon_png(&exe_str)
                    .unwrap_or(None)
                    .unwrap_or_else(|| format!("{},0", exe_str));
                ensure_app_id_registry(&app_id, "AstroBox", &icon_value)?;
            }
            let _ = ensure_shortcut(&app_id);
            let id_h = HSTRING::from(&app_id);
            if let Ok(notifier) = ToastNotificationManager::CreateToastNotifierWithId(&id_h) {
                Ok(notifier)
            } else {
                Ok(ToastNotificationManager::CreateToastNotifier()?)
            }
        } else {
            Ok(ToastNotificationManager::CreateToastNotifier()?)
        }
    }

    fn remove_history(tag: &str, app_id: Option<String>) -> Result<()> {
        let history = ToastNotificationManager::History()?;
        let tag_h = HSTRING::from(tag);
        if let Some(id) = app_id {
            let id_h = HSTRING::from(id);
            let group_h = HSTRING::from("");
            let _ = history.RemoveGroupedTagWithId(&tag_h, &group_h, &id_h);
            let _ = history.Remove(&tag_h);
        } else {
            let _ = history.Remove(&tag_h);
        }
        Ok(())
    }

    fn schedule_remove_history(tag: String) {
        let app_id = resolve_app_id();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(2));
            let _ = remove_history(&tag, app_id);
        });
    }

    fn expiration_after(delay: Duration) -> DateTime {
        let since_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let base_100ns = (since_unix.as_secs() + 11_644_473_600) * 10_000_000
            + (since_unix.subsec_nanos() as u64 / 100);
        let extra_100ns = (delay.as_nanos() / 100) as u64;
        DateTime {
            UniversalTime: (base_100ns + extra_100ns) as i64,
        }
    }

    pub fn create_live_activity(
        _self: &impl Sized,
        payload: CreateLiveActivityRequest,
    ) -> Result<()> {
        let (id, title, text, mut state, task_name, task_type, _task_icon) =
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

        let unique_tag = {
            let since = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            use std::hash::{Hash, Hasher};
            id.hash(&mut hasher);
            since.as_millis().hash(&mut hasher);
            format!("{:x}", hasher.finish())
        };
        {
            let mut g = current_tag().lock().unwrap();
            *g = Some(unique_tag.clone());
        }
        {
            let mut m = current_meta().lock().unwrap();
            *m = Some(Meta {
                title: title.clone(),
                text: text.clone(),
                task_name: task_name.clone(),
                task_type: task_type.clone(),
            });
        }
        let progress_value = state
            .remove("progress")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        let progress_text = state
            .remove("percent")
            .map(|p| format!("{}%", p))
            .unwrap_or_else(|| format!("{:.1}%", progress_value * 100.0));

        let image_xml = String::new();

        let xml = format!(
            "<toast>\
                <visual>\
                    <binding template=\"ToastGeneric\">\
                        <text>{}</text>\
                        <text>{} · {}</text>\
                        {}\
                        <progress title=\"{}\" status=\"{}\" value=\"{{progressValue}}\" valueStringOverride=\"{{progressText}}\"/>\
                    </binding>\
                </visual>\
            </toast>",
            escape_xml(&title),
            escape_xml(&text),
            escape_xml(&task_name),
            image_xml,
            escape_xml(&task_name),
            escape_xml(&task_type)
        );

        let doc = XmlDocument::new()?;
        let xml_h = HSTRING::from(xml);
        doc.LoadXml(&xml_h).context("Toast XML load failed")?;

        let toast = ToastNotification::CreateToastNotification(&doc).context("Create toast failed")?;
        let tag_h = HSTRING::from(&unique_tag);
        toast.SetTag(&tag_h).context("Set toast tag failed")?;

        let notifier = create_notifier()?;
        let data = NotificationData::new()?;
        let values = data.Values()?;
        let key_value = HSTRING::from("progressValue");
        let value_value = HSTRING::from(format!("{:.3}", progress_value));
        values.Insert(&key_value, &value_value)?;
        let key_text = HSTRING::from("progressText");
        let value_text = HSTRING::from(progress_text);
        values.Insert(&key_text, &value_text)?;
        data.SetSequenceNumber(1)?;
        toast.SetData(&data).context("Set toast data failed")?;
        if (progress_value - 1.0).abs() < f32::EPSILON {
            let expire = expiration_after(Duration::from_secs(2));
            let expire_ref: IReference<DateTime> =
                PropertyValue::CreateDateTime(expire)?.cast()?;
            toast
                .SetExpirationTime(&expire_ref)
                .context("Set expiration failed")?;
        }
        notifier.Show(&toast).context("Show toast failed")?;
        if (progress_value - 1.0).abs() < f32::EPSILON {
            schedule_remove_history(unique_tag.clone());
            let mut g = current_tag().lock().unwrap();
            *g = None;
            *current_meta().lock().unwrap() = None;
        }

        Ok(())
    }

    pub fn update_live_activity(
        _self: &impl Sized,
        payload: UpdateLiveActivityRequest,
    ) -> Result<()> {
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
            let meta = {
                let m = current_meta().lock().unwrap();
                m.clone().context("No meta to update")?
            };
            let progress_text = "100%".to_string();
            let image_xml = String::new();
            let xml = format!(
                "<toast>\
                    <visual>\
                        <binding template=\"ToastGeneric\">\
                            <text>{}</text>\
                            <text>{} · {}</text>\
                            {}\
                            <progress title=\"{}\" status=\"{}\" value=\"{{progressValue}}\" valueStringOverride=\"{{progressText}}\"/>\
                        </binding>\
                    </visual>\
                </toast>",
                escape_xml(&meta.title),
                escape_xml(&meta.text),
                escape_xml(&meta.task_name),
                image_xml,
                escape_xml(&meta.task_name),
                escape_xml(&meta.task_type)
            );
            let doc = XmlDocument::new()?;
            let xml_h = HSTRING::from(xml);
            doc.LoadXml(&xml_h).context("Toast XML load failed")?;
            let toast = ToastNotification::CreateToastNotification(&doc).context("Create toast failed")?;
            let tag_h = HSTRING::from(&tag);
            toast.SetTag(&tag_h).context("Set toast tag failed")?;
            let expire = expiration_after(Duration::from_secs(2));
            let expire_ref: IReference<DateTime> =
                PropertyValue::CreateDateTime(expire)?.cast()?;
            toast
                .SetExpirationTime(&expire_ref)
                .context("Set expiration failed")?;
            let data = NotificationData::new()?;
            let values = data.Values()?;
            let key_value = HSTRING::from("progressValue");
            let value_value = HSTRING::from("1");
            values.Insert(&key_value, &value_value)?;
            let key_text = HSTRING::from("progressText");
            let value_text = HSTRING::from(progress_text);
            values.Insert(&key_text, &value_text)?;
            data.SetSequenceNumber(9999)?;
            toast.SetData(&data).context("Set toast data failed")?;
            let notifier = create_notifier()?;
            notifier.Show(&toast).context("Show toast failed")?;
            schedule_remove_history(tag.clone());
            let mut g = current_tag().lock().unwrap();
            *g = None;
            *current_meta().lock().unwrap() = None;
        } else {
            let pct_text = if let Some(s) = payload.state.get("percent") {
                format!("{}%", s)
            } else {
                format!("{:.1}%", p * 100.0)
            };
            let notifier = create_notifier()?;
            let data = NotificationData::new()?;
            let values = data.Values()?;
            let key_value = HSTRING::from("progressValue");
            let value_value = HSTRING::from(format!("{:.3}", p));
            values.Insert(&key_value, &value_value)?;
            let key_text = HSTRING::from("progressText");
            let value_text = HSTRING::from(pct_text);
            values.Insert(&key_text, &value_text)?;
            data.SetSequenceNumber(2)?;
            let tag_h = HSTRING::from(&tag);
            notifier.UpdateWithTag(&data, &tag_h)?;
        }

        Ok(())
    }

    pub fn remove_live_activity(_self: &impl Sized) -> Result<()> {
        if let Some(tag) = { current_tag().lock().unwrap().clone() } {
            let notifier = create_notifier()?;
            let data = NotificationData::new()?;
            let values = data.Values()?;
            let key_value = HSTRING::from("progressValue");
            let value_value = HSTRING::from("1");
            values.Insert(&key_value, &value_value)?;
            let key_text = HSTRING::from("progressText");
            let value_text = HSTRING::from("100%");
            values.Insert(&key_text, &value_text)?;
            data.SetSequenceNumber(9999)?;
            let tag_h = HSTRING::from(&tag);
            let group_h = HSTRING::from("");
            let _ = notifier.UpdateWithTagAndGroup(&data, &tag_h, &group_h);
            schedule_remove_history(tag);
            *current_tag().lock().unwrap() = None;
        }
        Ok(())
    }
}
