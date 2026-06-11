#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use rdev::{listen, Event as RdevEvent, EventType};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Manager, SystemTray, SystemTrayEvent};
use std::sync::atomic::{AtomicPtr, Ordering};

#[cfg(target_os = "windows")]
static MUTEX_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());


struct AppState {
    overlay_windows: Mutex<Vec<String>>,
    // Store timestamp and input type ("mouse" or "keyboard")
    last_activity: Arc<Mutex<(u64, String)>>,
}

#[tauri::command]
fn log_to_file(_msg: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
fn prevent_sleep() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use winapi::um::winbase::SetThreadExecutionState;
        use winapi::um::winnt::{ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED};

        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED);
        }
    }

    Ok(())
}

#[tauri::command]
fn allow_sleep() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use winapi::um::winbase::SetThreadExecutionState;
        use winapi::um::winnt::ES_CONTINUOUS;

        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS);
        }
    }

    Ok(())
}

#[tauri::command]
fn force_sleep(_app: tauri::AppHandle, _state: tauri::State<'_, AppState>) -> Result<(), String> {
    // 1. Allow sleep
    #[cfg(target_os = "windows")]
    {
        use winapi::um::winbase::SetThreadExecutionState;
        use winapi::um::winnt::ES_CONTINUOUS;

        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS);
        }
    }

    // 2. Close any existing overlays (Screen Darken)
    // We can reuse the close_screen_overlay logic or just let frontend handle it.
    // However, specifically NOT opening sleep.html anymore.

    Ok(())
}

#[tauri::command]
fn get_last_activity(state: tauri::State<'_, AppState>) -> Result<(u64, String), String> {
    let activity = state.last_activity.lock().unwrap();
    Ok(activity.clone())
}

#[cfg(target_os = "windows")]
fn is_windows_workstation_locked() -> bool {
    use winapi::um::winuser::{
        CloseDesktop, GetUserObjectInformationW, OpenInputDesktop, DESKTOP_READOBJECTS, UOI_NAME,
    };

    unsafe {
        let desktop = OpenInputDesktop(0, 0, DESKTOP_READOBJECTS);
        if desktop.is_null() {
            return true;
        }

        let mut name = [0u16; 256];
        let mut needed = 0u32;
        let ok = GetUserObjectInformationW(
            desktop as _,
            UOI_NAME as i32,
            name.as_mut_ptr() as _,
            (name.len() * std::mem::size_of::<u16>()) as u32,
            &mut needed,
        );
        CloseDesktop(desktop);

        if ok == 0 || needed < std::mem::size_of::<u16>() as u32 {
            return true;
        }

        let len = (needed as usize / std::mem::size_of::<u16>()).saturating_sub(1);
        let desktop_name = String::from_utf16_lossy(&name[..len]);
        !desktop_name.eq_ignore_ascii_case("Default")
    }
}

#[cfg(not(target_os = "windows"))]
fn is_windows_workstation_locked() -> bool {
    false
}

#[tauri::command]
fn is_workstation_locked() -> Result<bool, String> {
    Ok(is_windows_workstation_locked())
}

#[tauri::command]
async fn show_notification(
    app: tauri::AppHandle,
    title: String,
    body: String,
) -> Result<(), String> {
    use tauri::api::notification::Notification;

    if is_windows_workstation_locked() {
        return Ok(());
    }

    // Try native Tauri notification first
    let result = Notification::new(&app.config().tauri.bundle.identifier)
        .title(&title)
        .body(&body)
        .show();

    // If native fails, fallback to simple PowerShell (MSG command is too intrusive, toast is better)
    if result.is_err() {
        // Fallback or just ignore if user systems are strict
        // Simple fallback to beep?
        // std::print!("\x07");
    }

    Ok(())
}

fn append_log(_msg: &str) {}

fn create_overlay_window(
    app: &tauri::AppHandle,
    state: &tauri::State<'_, AppState>,
    url: &str,
    title: &str,
) -> Result<(), String> {
    let mut overlay_windows = state.overlay_windows.lock().unwrap();
    // Close existing if any
    for label in overlay_windows.iter() {
        if let Some(window) = app.get_window(label) {
            window.close().ok();
        }
    }
    overlay_windows.clear();

    let main_window = app.get_window("main").ok_or("Main window not found")?;
    let monitors = main_window
        .available_monitors()
        .map_err(|e| format!("Failed to get monitors: {}", e))?;

    for (i, monitor) in monitors.iter().enumerate() {
        let safe_title = title
            .replace(" ", "_")
            .replace(|c: char| !c.is_alphanumeric(), "_");
        let label = format!("overlay_{}_{}", safe_title, i); // Unique label
        let _position = monitor.position();
        let _size = monitor.size();

        let window = tauri::WindowBuilder::new(app, &label, tauri::WindowUrl::App(url.into()))
            .title(title)
            .decorations(false)
            .always_on_top(true)
            .resizable(true)
            .fullscreen(true) // Try fullscreen for better coverage
            .visible(false) // Start hidden to avoid flicker
            .build();

        if let Ok(window) = window {
            // Force maximize for Windows 11 compat
            let _ = window.maximize();

            // Use explicit Physical Position/Size to correctly cover monitor regardless of DPI
            let _ = window.set_position(tauri::Position::Physical(monitor.position().clone()));
            let _ = window.set_size(tauri::Size::Physical(monitor.size().clone()));

            // Show it
            let _ = window.show();
            // Focus it
            let _ = window.set_focus();

            overlay_windows.push(label);
        }
    }
    Ok(())
}

#[tauri::command]
async fn create_screen_overlay(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    create_overlay_window(&app, &state, "overlay.html", "Screen Overlay")
}

#[tauri::command]
async fn close_screen_overlay(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut overlay_windows = state.overlay_windows.lock().unwrap();

    for label in overlay_windows.iter() {
        if let Some(window) = app.get_window(label) {
            window.close().ok();
        }
    }

    overlay_windows.clear();
    Ok(())
}

#[tauri::command]
fn set_autostart(enable: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let path = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
        let (key, _disp) = hkcu.create_subkey(path).map_err(|e| e.to_string())?;

        if enable {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let exe_path = exe.to_str().ok_or("Invalid path")?;
            key.set_value("NeverSleepTauri", &exe_path)
                .map_err(|e| e.to_string())?;
        } else {
            key.delete_value("NeverSleepTauri").ok();
        }
    }
    Ok(())
}

#[tauri::command]
fn check_autostart() -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let path = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
        if let Ok(key) = hkcu.open_subkey(path) {
            let val: Result<String, _> = key.get_value("NeverSleepTauri");
            return Ok(val.is_ok());
        }
    }
    Ok(false)
}

#[tauri::command]
fn hide_window(window: tauri::Window) {
    let _ = window.hide();
}

#[tauri::command]
fn minimize_window(window: tauri::Window) {
    let _ = window.minimize();
}

#[tauri::command]
fn focus_main_window(window: tauri::Window) {
    if window.is_minimized().unwrap_or(false) {
        let _ = window.unminimize();
    }
    let _ = window.show();
    let _ = window.set_focus();
    // Force focus hack
    let _ = window.set_always_on_top(true);
    let _ = window.set_always_on_top(false);
}

#[tauri::command]
fn get_settings_path(app: tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let config_dir = app
        .path_resolver()
        .app_config_dir()
        .ok_or("Failed to get config dir")?;
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    }
    Ok(config_dir.join("settings.json"))
}

#[tauri::command]
fn save_settings(app: tauri::AppHandle, settings: String) -> Result<(), String> {
    let path = get_settings_path(app)?;
    std::fs::write(path, settings).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn load_settings(app: tauri::AppHandle) -> Result<String, String> {
    let path = get_settings_path(app)?;
    if path.exists() {
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        Ok(content)
    } else {
        Ok("{}".to_string())
    }
}

#[tauri::command]
fn play_sys_sound() {
    #[cfg(target_os = "windows")]
    unsafe {
        use winapi::um::winuser::{MessageBeep, MB_ICONASTERISK};
        MessageBeep(MB_ICONASTERISK);
    }
}

#[tauri::command]
fn open_settings(app: tauri::AppHandle, section: Option<String>) {
    let url_str = match &section {
        Some(sec) => format!("settings.html?section={}", sec),
        None => "settings.html".to_string(),
    };

    if let Some(window) = app.get_window("settings") {
        let _ = window.eval("if (window.refreshSettingsFromStorage) window.refreshSettingsFromStorage();");
        if let Some(sec) = &section {
            let _ = window.eval(&format!("if (window.showOnlySection) window.showOnlySection('{}');", sec));
        } else {
            let _ = window.eval("if (window.showAllSections) window.showAllSections();");
        }
        if window.is_minimized().unwrap_or(false) {
            let _ = window.unminimize();
        }
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.set_always_on_top(true);
        let _ = window.set_always_on_top(false);
    } else {
        // Fallback: This should rarely happen with static window, but if it was somehow destroyed
        let _ = tauri::WindowBuilder::new(
            &app,
            "settings",
            tauri::WindowUrl::App(url_str.into()),
        )
        .title("Settings - Never Sleep")
        .inner_size(500.0, 600.0)
        .resizable(false)
        .transparent(true)
        .decorations(true)
        .build();
    }
}

#[tauri::command]
fn close_settings(app: tauri::AppHandle) {
    if let Some(window) = app.get_window("settings") {
        let _ = window.hide();
    }
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn restart_app(app: tauri::AppHandle) {
    #[cfg(target_os = "windows")]
    {
        use winapi::um::handleapi::CloseHandle;

        let handle = MUTEX_HANDLE.swap(std::ptr::null_mut(), Ordering::SeqCst);
        if !handle.is_null() {
            unsafe {
                CloseHandle(handle as *mut _);
            }
        }
        // Small delay to ensure the OS registers the handle release
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    app.restart();
}


#[cfg(target_os = "windows")]
fn embedded_icon_bytes(filename: &str) -> Result<&'static [u8], String> {
    match filename {
        "IMG_6276.png" => Ok(include_bytes!("../../dist/IMG_6276.png") as &[u8]),
        "Stylish 3D Coffee Cup Icon with Steam Effect - Woopicx.png" => Ok(include_bytes!("../../dist/Stylish 3D Coffee Cup Icon with Steam Effect - Woopicx.png") as &[u8]),
        "Stylish 3D Coffee Cup Icon with Steam and Isometric Design.png" => Ok(include_bytes!("../../dist/Stylish 3D Coffee Cup Icon with Steam and Isometric Design.png") as &[u8]),
        "Stylized Coffee Machine Icon with White Cup - Woopicx.png" => Ok(include_bytes!("../../dist/Stylized Coffee Machine Icon with White Cup - Woopicx.png") as &[u8]),
        "A visually striking illustration.png" => Ok(include_bytes!("../../dist/A visually striking illustration.png") as &[u8]),
        "3D Battery Icon with Lightning Bolt in Isometric View.png" => Ok(include_bytes!("../../dist/3D Battery Icon with Lightning Bolt in Isometric View.png") as &[u8]),
        _ => Err(format!("Unknown icon name: {}", filename)),
    }
}

#[cfg(target_os = "windows")]
fn load_normalized_icon_image(filename: &str) -> Result<image::RgbaImage, String> {
    let img = image::load_from_memory(embedded_icon_bytes(filename)?)
        .map_err(|e| format!("Failed to decode embedded image {}: {}", filename, e))?;

    let image = img.to_rgba8();

    let resized = image::imageops::resize(
        &image,
        256,
        256,
        image::imageops::FilterType::Lanczos3,
    );

    Ok(resized)
}



#[cfg(target_os = "windows")]
fn create_multi_res_ico(filename: &str) -> Result<Vec<u8>, String> {
    let raw_bytes = embedded_icon_bytes(filename)?;
    let base_img = image::load_from_memory(raw_bytes)
        .map_err(|e| format!("Failed to decode image: {}", e))?;

    let rgba_base = base_img.to_rgba8();

    let sizes = [16, 24, 32, 48, 64, 128, 256];
    let mut png_images = Vec::new();

    for &sz in &sizes {
        let resized = image::imageops::resize(
            &rgba_base,
            sz,
            sz,
            image::imageops::FilterType::Lanczos3,
        );
        let mut png_bytes = Vec::new();
        image::DynamicImage::ImageRgba8(resized)
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageOutputFormat::Png,
            )
            .map_err(|e| format!("Failed to encode resized PNG: {}", e))?;
        png_images.push(png_bytes);
    }

    let image_count = png_images.len();
    let header_size = 6;
    let dir_entry_size = 16;
    let mut offset = header_size + image_count * dir_entry_size;

    let mut ico = Vec::new();
    ico.extend_from_slice(&0u16.to_le_bytes()); // Reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // Icon type
    ico.extend_from_slice(&(image_count as u16).to_le_bytes()); // Image count

    for (i, &sz) in sizes.iter().enumerate() {
        let w = if sz >= 256 { 0 } else { sz as u8 };
        let h = if sz >= 256 { 0 } else { sz as u8 };
        ico.push(w);
        ico.push(h);
        ico.push(0); // color count
        ico.push(0); // reserved
        ico.extend_from_slice(&1u16.to_le_bytes()); // planes
        ico.extend_from_slice(&32u16.to_le_bytes()); // bit count

        let size_in_bytes = png_images[i].len() as u32;
        ico.extend_from_slice(&size_in_bytes.to_le_bytes());
        ico.extend_from_slice(&(offset as u32).to_le_bytes());

        offset += size_in_bytes as usize;
    }

    for data in png_images {
        ico.extend_from_slice(&data);
    }

    Ok(ico)
}

#[cfg(target_os = "windows")]
fn write_current_icon_file(app: &tauri::AppHandle, filename: &str) -> Result<std::path::PathBuf, String> {
    use std::hash::{Hash, Hasher};

    let icon_dir = app
        .path_resolver()
        .app_config_dir()
        .ok_or("Could not resolve app config directory")?
        .join("icons");
    std::fs::create_dir_all(&icon_dir).map_err(|e| e.to_string())?;

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    filename.hash(&mut hasher);
    let icon_path = icon_dir.join(format!("icon-v2-{:016x}.ico", hasher.finish()));
    let ico_bytes = create_multi_res_ico(filename)?;
    std::fs::write(&icon_path, ico_bytes).map_err(|e| e.to_string())?;
    Ok(icon_path)
}


#[cfg(target_os = "windows")]
fn apply_native_window_icons(app: &tauri::AppHandle, icon_path: &std::path::Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use winapi::shared::minwindef::{LPARAM, WPARAM};
    use winapi::um::winuser::{
        LoadImageW, SendMessageW, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_LOADFROMFILE, WM_SETICON,
    };

    let mut wide_path: Vec<u16> = icon_path.as_os_str().encode_wide().collect();
    wide_path.push(0);

    for label in ["main", "settings"] {
        if let Some(window) = app.get_window(label) {
            if let Ok(hwnd) = window.hwnd() {
                unsafe {
                    let big_icon = LoadImageW(
                        std::ptr::null_mut(),
                        wide_path.as_ptr(),
                        IMAGE_ICON,
                        256,
                        256,
                        LR_LOADFROMFILE,
                    );
                    if !big_icon.is_null() {
                        SendMessageW(
                            hwnd.0 as _,
                            WM_SETICON,
                            ICON_BIG as WPARAM,
                            big_icon as LPARAM,
                        );
                    }

                    let small_icon = LoadImageW(
                        std::ptr::null_mut(),
                        wide_path.as_ptr(),
                        IMAGE_ICON,
                        32,
                        32,
                        LR_LOADFROMFILE,
                    );
                    if !small_icon.is_null() {
                        SendMessageW(
                            hwnd.0 as _,
                            WM_SETICON,
                            ICON_SMALL as WPARAM,
                            small_icon as LPARAM,
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

#[derive(serde::Serialize)]
struct HardIconApplyReport {
    icon_file: String,
    updated_shortcuts: Vec<String>,
    failed_shortcuts: Vec<String>,
    note: String,
}

/// Set the app icon (taskbar + tray) from a filename in the dist/ folder.
/// `filename` should be just the filename, e.g. "icons8_coffee3.png"
#[tauri::command]
fn set_app_icon(app: tauri::AppHandle, filename: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let icon_image = load_normalized_icon_image(&filename)?;
        let (width, height) = icon_image.dimensions();
        let icon = tauri::Icon::Rgba {
            rgba: icon_image.into_raw(),
            width,
            height,
        };

        // Apply to main window (sets both taskbar button icon and title-bar icon)
        if let Some(window) = app.get_window("main") {
            window.set_icon(icon.clone())
                .map_err(|e| format!("Failed to set main window icon: {}", e))?;
        }

        // Also apply to settings window if open
        if let Some(window) = app.get_window("settings") {
            window.set_icon(icon.clone())
                .map_err(|e| format!("Failed to set settings window icon: {}", e))?;
        }

        // Update system tray icon
        app.tray_handle().set_icon(icon)
            .map_err(|e| format!("Failed to set tray icon: {}", e))?;

        let icon_path = write_current_icon_file(&app, &filename)?;
        apply_native_window_icons(&app, &icon_path)?;

        return Ok(());
    }

    #[allow(unreachable_code)]
    Ok(())
}

#[tauri::command]
fn hard_apply_app_icon(
    app: tauri::AppHandle,
    filename: String,
) -> Result<HardIconApplyReport, String> {
    set_app_icon(app.clone(), filename.clone())?;

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        use std::process::Command;

        let icon_path = write_current_icon_file(&app, &filename)?;
        apply_native_window_icons(&app, &icon_path)?;

        let icon_path_string = icon_path.to_string_lossy().to_string();
        let escaped_icon_path = icon_path_string.replace('\'', "''");
        let exe_path_string = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .to_string();
        let escaped_exe_path = exe_path_string.replace('\'', "''");
        let script = format!(
            r#"
$icon = '{}'
$exe = '{}'
$shell = New-Object -ComObject WScript.Shell
$folders = @(
  [Environment]::GetFolderPath('Desktop'),
  [Environment]::GetFolderPath('CommonDesktopDirectory'),
  ([Environment]::GetFolderPath('StartMenu') + '\Programs'),
  ([Environment]::GetFolderPath('CommonStartMenu') + '\Programs'),
  ($env:APPDATA + '\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar')
)
$matched = $false
foreach ($folder in ($folders | Where-Object {{ $_ -and (Test-Path -LiteralPath $_) }} | Select-Object -Unique)) {{
  Get-ChildItem -LiteralPath $folder -Filter '*.lnk' -Recurse -ErrorAction SilentlyContinue |
    ForEach-Object {{
      try {{
        $shortcut = $shell.CreateShortcut($_.FullName)
        $target = $shortcut.TargetPath
        $targetName = if ($target) {{ Split-Path -Leaf $target }} else {{ '' }}
        $isMatch = $_.BaseName -like 'Never Sleep*' -or
          $_.BaseName -like 'NeverSleep*' -or
          $target -ieq $exe -or
          $targetName -like 'Never Sleep*.exe' -or
          $targetName -ieq 'app.exe'
        if ($isMatch) {{
          $matched = $true
          $shortcut.IconLocation = $icon
          $shortcut.Save()
          'UPDATED|' + $_.FullName
        }}
      }} catch {{
        'FAILED|' + $_.FullName + '|' + $_.Exception.Message
      }}
    }}
}}
if (-not $matched) {{ 'NONE|No Never Sleep shortcuts found' }}
try {{
  Start-Process -FilePath "$env:windir\System32\ie4uinit.exe" -ArgumentList '-show' -WindowStyle Hidden -ErrorAction SilentlyContinue
  'REFRESH|Requested Windows icon cache refresh'
}} catch {{
  'REFRESH_FAILED|' + $_.Exception.Message
}}
"#,
            escaped_icon_path,
            escaped_exe_path
        );

        let output = Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output()
            .map_err(|e| e.to_string())?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut updated_shortcuts = Vec::new();
        let mut failed_shortcuts = Vec::new();

        for line in stdout.lines() {
            if let Some(path) = line.strip_prefix("UPDATED|") {
                updated_shortcuts.push(path.to_string());
            } else if let Some(rest) = line.strip_prefix("FAILED|") {
                failed_shortcuts.push(rest.to_string());
            } else if let Some(rest) = line.strip_prefix("REFRESH_FAILED|") {
                failed_shortcuts.push(format!("Icon cache refresh failed: {}", rest));
            }
        }

        if !output.status.success() {
            failed_shortcuts.push(format!("PowerShell exited with status {}", output.status));
        }
        if !stderr.trim().is_empty() {
            failed_shortcuts.push(stderr.trim().to_string());
        }

        let note = if updated_shortcuts.is_empty() {
            "Live window and tray icons were applied. No Never Sleep shortcuts were found to update; pinned taskbar icons may need unpinning and re-pinning.".to_string()
        } else {
            "Live icon applied and matching Windows shortcuts were updated. Pinned taskbar icons may still need an unpin/re-pin if Windows keeps an old cached icon.".to_string()
        };

        return Ok(HardIconApplyReport {
            icon_file: icon_path_string,
            updated_shortcuts,
            failed_shortcuts,
            note,
        });
    }

    #[allow(unreachable_code)]
    Ok(HardIconApplyReport {
        icon_file: String::new(),
        updated_shortcuts: Vec::new(),
        failed_shortcuts: Vec::new(),
        note: "Live icon applied. Hard Windows shortcut updates are only available on Windows."
            .to_string(),
    })
}

#[tauri::command]
fn open_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let allowed_url = "https://github.com/netanel3000fine/NeverSleep-App/releases";
    if url != allowed_url {
        return Err("URL not allowed".to_string());
    }

    if tauri::api::shell::open(&app.shell_scope(), &url, None).is_ok() {
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        use std::process::Command;
        Command::new("cmd")
            .args(["/C", "start", "", &url])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("Failed to open browser".to_string())
}

#[tauri::command]
fn set_pinned(app: tauri::AppHandle, pinned: bool) {
    if let Some(window) = app.get_window("main") {
        let _ = window.set_always_on_top(pinned);
    }
}

#[tauri::command]
fn is_main_visible(app: tauri::AppHandle) -> bool {
    if let Some(w) = app.get_window("main") {
        return w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false);
    }
    false
}

#[tauri::command]
fn set_border_color(app: tauri::AppHandle, color: String) {
    #[cfg(target_os = "windows")]
    {
        use tauri::Manager;
        use winapi::shared::minwindef::{DWORD, LPCVOID};
        use winapi::um::dwmapi::DwmSetWindowAttribute;

        // DWMWA_BORDER_COLOR = 34
        const DWMWA_BORDER_COLOR: DWORD = 34;

        fn hex_to_colorref(hex: &str) -> Option<u32> {
            let hex = hex.trim_start_matches('#');
            if hex.len() != 6 {
                return None;
            }
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some((b as u32) << 16 | (g as u32) << 8 | (r as u32))
        }

        if let Some(cr) = hex_to_colorref(&color) {
            let windows = ["main", "settings"];
            for label in windows {
                if let Some(window) = app.get_window(label) {
                    if let Ok(hwnd) = window.hwnd() {
                        unsafe {
                            let pv_attribute = &cr as *const u32 as LPCVOID;
                            DwmSetWindowAttribute(
                                hwnd.0 as _,
                                DWMWA_BORDER_COLOR,
                                pv_attribute,
                                std::mem::size_of::<u32>() as DWORD,
                            );
                        }
                    }
                }
            }
        }
    }
}

#[tauri::command]
fn set_window_effect(app: tauri::AppHandle, effect: String) {
    #[cfg(target_os = "windows")]
    {
        use window_vibrancy::{apply_acrylic, clear_acrylic};

        let labels = ["main", "settings"];
        for label in labels {
            if let Some(window) = app.get_window(label) {
                match effect.as_str() {
                    "acrylic" => {
                        let _ = apply_acrylic(&window, Some((10, 10, 15, 60)));
                    }
                    "none" => {
                        let _ = clear_acrylic(&window);
                    }
                    _ => {}
                }
            }
        }
    }
}

#[tauri::command]
fn set_window_size(app: tauri::AppHandle, width: u32, height: u32) {
    if let Some(window) = app.get_window("main") {
        let _ = window.set_resizable(true);
        let _ = window.set_min_size(None::<tauri::Size>);
        let _ = window.set_max_size(None::<tauri::Size>);
        let size = tauri::Size::Logical(tauri::LogicalSize {
            width: width as f64,
            height: height as f64,
        });
        let _ = window.set_size(size);
        let _ = window.set_min_size(Some(size));
        let _ = window.set_max_size(Some(size));

        // Ensure transparent border-radius works on Windows
        // Need to remove decorations to allow the CSS border-radius to curve the edges physically
        let _ = window.set_decorations(false);
    }
}

#[tauri::command]
fn set_window_height(app: tauri::AppHandle, height: u32) {
    if let Some(window) = app.get_window("main") {
        // 1. Enable resizing (essential if resizable: false in config)
        let _ = window.set_resizable(true);

        // 2. Clear constraints to avoid conflicts
        let _ = window.set_min_size(None::<tauri::Size>);
        let _ = window.set_max_size(None::<tauri::Size>);

        // 3. Set the size (Use LogicalSize to match tauri.conf.json and handle DPI scaling)
        let size = tauri::Size::Logical(tauri::LogicalSize {
            width: 500.0,
            height: height as f64,
        });
        let _ = window.set_size(size);

        // 4. Lock resizing using constraints instead of set_resizable(false)
        // This keeps the window "resizable" (for Acrylic) but fixed in size (for UX)
        let _ = window.set_min_size(Some(size));
        let _ = window.set_max_size(Some(size));
    }
}

#[tauri::command]
fn set_decorations(app: tauri::AppHandle, decorations: bool) {
    if let Some(window) = app.get_window("main") {
        let _ = window.set_decorations(decorations);
    }
}

#[cfg(target_os = "windows")]
fn get_media_manager() -> Option<&'static windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager> {
    static MEDIA_MANAGER: std::sync::OnceLock<Option<windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager>> = std::sync::OnceLock::new();
    MEDIA_MANAGER.get_or_init(|| {
        use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;
        GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .ok()
            .and_then(|op| op.get().ok())
    }).as_ref()
}

#[tauri::command]
async fn is_media_playing() -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackStatus;

        let mut playing = false;
        if let Some(manager) = get_media_manager() {
            if let Ok(session) = manager.GetCurrentSession() {
                if let Ok(info) = session.GetPlaybackInfo() {
                    if let Ok(status) = info.PlaybackStatus() {
                        if status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing {
                            if let Ok(app_id) = session.SourceAppUserModelId() {
                                let app_id_lower = app_id.to_string().to_lowercase();
                                // Browsers (Chrome, Edge etc) report YouTube as Music, 
                                // so we can't use PlaybackType. Instead we specifically ignore Spotify/iTunes.
                                if !app_id_lower.contains("spotify") 
                                && !app_id_lower.contains("itunes") 
                                && !app_id_lower.contains("apple music") {
                                    playing = true;
                                }
                            } else {
                                // Fallback if no ID is provided
                                playing = true;
                            }
                        }
                    }
                }
            }
        }
        return playing;
    }
    #[allow(unreachable_code)]
    false
}


fn main() {
    // Single Instance Check using WinAPI Mutex
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use winapi::shared::winerror::ERROR_ALREADY_EXISTS;
        use winapi::um::errhandlingapi::GetLastError;
        use winapi::um::synchapi::CreateMutexW;

        let mutex_name: Vec<u16> = OsStr::new("Global\\NeverSleepTauriAppMutex")
            .encode_wide()
            .chain(Some(0))
            .collect();

        unsafe {
            let mutex = CreateMutexW(std::ptr::null_mut(), 1, mutex_name.as_ptr());
            if GetLastError() == ERROR_ALREADY_EXISTS {
                std::process::exit(0);
            }
            MUTEX_HANDLE.store(mutex as *mut _, Ordering::SeqCst);
        }
    }

    let last_activity = Arc::new(Mutex::new((0u64, "mouse".to_string())));
    let activity_clone = Arc::clone(&last_activity);

    // Start global input listener in a separate thread
    std::thread::spawn(move || {
        // Track last update time to debounce high-frequency MouseMove events.
        // This avoids locking the mutex on every pixel of cursor movement,
        // which was causing unnecessary CPU overhead on Windows.
        let mut last_mouse_update_ms: u64 = 0;

        if let Err(error) = listen(move |event: RdevEvent| {
            let is_mouse_move = matches!(event.event_type, EventType::MouseMove { .. });

            // For MouseMove, skip the update if less than 500ms have passed
            if is_mouse_move {
                let now_raw = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
                    .try_into()
                    .unwrap_or(0u64);
                if now_raw.saturating_sub(last_mouse_update_ms) < 500 {
                    return;
                }
                last_mouse_update_ms = now_raw;
            }

            let now: u64 = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis()
                .try_into()
                .unwrap_or(0);

            let input_type = match event.event_type {
                EventType::MouseMove { .. }
                | EventType::Wheel { .. }
                | EventType::ButtonPress { .. }
                | EventType::ButtonRelease { .. } => "mouse",
                _ => "keyboard",
            };

            let mut last = activity_clone.lock().unwrap();
            *last = (now, input_type.to_string());
        }) {
            eprintln!("Error listening to input events: {:?}", error);
        }
    });

    // Tray Menu
    let show = tauri::CustomMenuItem::new("show".to_string(), "Show");
    let quit = tauri::CustomMenuItem::new("quit".to_string(), "Quit");
    let tray_menu = tauri::SystemTrayMenu::new()
        .add_item(show)
        .add_native_item(tauri::SystemTrayMenuItem::Separator)
        .add_item(quit);

    tauri::Builder::default()
        .manage(AppState {
            overlay_windows: Mutex::new(Vec::new()),
            last_activity,
        })
        .invoke_handler(tauri::generate_handler![
            prevent_sleep,
            allow_sleep,
            force_sleep,
            create_screen_overlay,
            close_screen_overlay,
            get_last_activity,
            is_workstation_locked,
            show_notification,
            log_to_file,
            set_autostart,
            check_autostart,
            hide_window,
            focus_main_window,
            play_sys_sound,
            open_settings,
            close_settings,
            is_main_visible,
            save_settings,
            load_settings,
            quit_app,
            restart_app,
            set_pinned,
            set_border_color,
            set_window_effect,
            set_window_height,
            set_window_size,
            set_decorations,
            open_url,
            is_media_playing,
            set_app_icon,
            hard_apply_app_icon
        ])
        .system_tray(SystemTray::new().with_menu(tray_menu))
        .on_system_tray_event(|app, event| match event {
            SystemTrayEvent::LeftClick { .. } => {
                let window = app.get_window("main").unwrap();
                if window.is_minimized().unwrap_or(false) {
                    window.unminimize().unwrap();
                }
                window.show().unwrap();
                window.set_focus().unwrap();
            }
            SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
                "show" => {
                    let window = app.get_window("main").unwrap();
                    window.show().unwrap();
                    window.set_focus().unwrap();
                }
                "quit" => {
                    std::process::exit(0);
                }
                _ => {}
            },
            _ => {}
        })
        .on_window_event(|event| match event.event() {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                let window = event.window();
                if window.label() == "settings" {
                    // Hide instead of close to keep state and avoid crash
                    window.hide().unwrap();
                    api.prevent_close();
                } else if window.label() == "main" {
                    // Quit app on main window close
                    std::process::exit(0);
                }
            }
            _ => {}
        })
        .setup(|app| {
            // Restore saved app icon from settings
            {
                let app_handle = app.handle();
                std::thread::spawn(move || {
                    // Small delay to ensure windows are ready
                    std::thread::sleep(std::time::Duration::from_millis(800));
                    if let Ok(path) = get_settings_path(app_handle.clone()) {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(icon_file) = json.get("appIcon").and_then(|v| v.as_str()) {
                                    let _ = set_app_icon(app_handle, icon_file.to_string());
                                }
                            }
                        }
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
