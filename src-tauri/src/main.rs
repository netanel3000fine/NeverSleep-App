#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use rdev::{listen, Event as RdevEvent, EventType};
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{
    CustomMenuItem, Manager, SystemTray, SystemTrayEvent, SystemTrayMenu, SystemTrayMenuItem,
    SystemTraySubmenu,
};

#[cfg(target_os = "windows")]
static MUTEX_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

static MAIN_WINDOW_PINNED: AtomicBool = AtomicBool::new(false);

// Global HWND storage for the Win32 darken window (isize is the raw HWND value)
#[cfg(target_os = "windows")]
static WIN32_DARKEN_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

// Global: previous brightness level before we dimmed it (for restore)
#[cfg(target_os = "windows")]
static PREV_BRIGHTNESS: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

// Global: unix-ms timestamp when the tray window was last shown (for the 600ms grace period)
static TRAY_SHOWN_AT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

// Extension trait to prevent crashes from poisoned mutexes
trait SafeLock<T> {
    fn safe_lock(&self) -> std::sync::MutexGuard<'_, T>;
}

impl<T> SafeLock<T> for Mutex<T> {
    fn safe_lock(&self) -> std::sync::MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

struct AppState {
    overlay_windows: Mutex<Vec<String>>,
    // Store timestamp and input type ("mouse" or "keyboard")
    last_activity: Arc<Mutex<(u64, String)>>,
    debug_log: Mutex<DebugLogState>,
    // Keep the Windows power request handle alive while the app prevents idle lock/sleep.
    power_request: Mutex<Option<isize>>,
}

struct DebugLogState {
    enabled: bool,
    desktop_dir: Option<std::path::PathBuf>,
    last_error: Option<String>,
}

#[tauri::command]
fn log_to_file(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    msg: String,
) -> Result<(), String> {
    append_debug_log(&app, &state, &msg)
}

#[cfg(not(target_os = "windows"))]
fn utc_timestamp() -> (String, String) {
    let milliseconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let seconds = milliseconds / 1_000;
    let days = (seconds / 86_400) as i64;
    let day_seconds = seconds % 86_400;

    // Convert the UTC day count to a Gregorian calendar date without another crate.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    let hour = day_seconds / 3_600;
    let minute = (day_seconds % 3_600) / 60;
    let second = day_seconds % 60;
    let millisecond = milliseconds % 1_000;
    (
        format!("{:04}-{:02}-{:02}", year, month, day),
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03} UTC",
            year, month, day, hour, minute, second, millisecond
        ),
    )
}

#[cfg(target_os = "windows")]
fn local_timestamp() -> (String, String) {
    use winapi::um::minwinbase::SYSTEMTIME;
    use winapi::um::sysinfoapi::GetLocalTime;

    let mut time: SYSTEMTIME = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut time) };
    let date = format!("{:04}-{:02}-{:02}", time.wYear, time.wMonth, time.wDay);
    let timestamp = format!(
        "{} {:02}:{:02}:{:02}.{:03}",
        date, time.wHour, time.wMinute, time.wSecond, time.wMilliseconds
    );
    (date, timestamp)
}

#[cfg(not(target_os = "windows"))]
fn local_timestamp() -> (String, String) {
    utc_timestamp()
}

fn write_debug_log_line(logger: &mut DebugLogState, message: &str) -> Result<(), String> {
    let desktop = logger
        .desktop_dir
        .as_ref()
        .ok_or_else(|| "Could not locate the Desktop folder".to_string())?;
    let (date, timestamp) = local_timestamp();
    let path = desktop.join(format!("NeverSleep-debug-{}.log", date));
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("Could not open {}: {}", path.display(), error))?;
    use std::io::Write;
    writeln!(file, "[{}] {}", timestamp, message)
        .map_err(|error| format!("Could not write {}: {}", path.display(), error))?;
    Ok(())
}

fn publish_debug_log_status(app: &tauri::AppHandle, status: String) {
    let _ = app.emit_all("debug-log-status", status);
}

fn append_debug_log(app: &tauri::AppHandle, state: &AppState, message: &str) -> Result<(), String> {
    let mut logger = state
        .debug_log
        .lock()
        .map_err(|_| "Debug logger lock is unavailable".to_string())?;
    if !logger.enabled {
        return Ok(());
    }
    let previous_error = logger.last_error.clone();
    let result = write_debug_log_line(&mut logger, message);
    if let Err(error) = &result {
        logger.last_error = Some(error.clone());
    } else {
        logger.last_error = None;
    }
    let status = logger
        .last_error
        .clone()
        .map(|error| format!("Error: {}", error))
        .unwrap_or_else(|| {
            logger
                .desktop_dir
                .as_ref()
                .map(|path| format!("Logging to {}", path.display()))
                .unwrap_or_else(|| "Debug logging is enabled".to_string())
        });
    drop(logger);
    if result.is_err() || previous_error.is_some() {
        publish_debug_log_status(app, status);
    }
    result
}

fn set_debug_logging(app: &tauri::AppHandle, state: &AppState, enabled: bool) -> String {
    let mut logger = match state.debug_log.lock() {
        Ok(logger) => logger,
        Err(_) => return "Error: Debug logger lock is unavailable".to_string(),
    };
    let was_enabled = logger.enabled;
    if enabled {
        logger.desktop_dir = dirs_next::desktop_dir();
        logger.enabled = true;
        if !was_enabled {
            if let Err(error) = write_debug_log_line(&mut logger, "Debug logging enabled") {
                logger.last_error = Some(error);
            } else {
                logger.last_error = None;
            }
        }
    } else if was_enabled {
        let final_write = write_debug_log_line(&mut logger, "Debug logging disabled");
        logger.enabled = false;
        logger.last_error = final_write.err();
    }
    let status = logger
        .last_error
        .clone()
        .map(|error| format!("Error: {}", error))
        .unwrap_or_else(|| {
            if logger.enabled {
                logger
                    .desktop_dir
                    .as_ref()
                    .map(|path| format!("Logging to {}", path.display()))
                    .unwrap_or_else(|| "Debug logging is enabled".to_string())
            } else {
                "Debug logging is off".to_string()
            }
        });
    drop(logger);
    publish_debug_log_status(app, status.clone());
    status
}

#[tauri::command]
fn get_debug_log_status(state: tauri::State<'_, AppState>) -> String {
    let logger = match state.debug_log.lock() {
        Ok(logger) => logger,
        Err(_) => return "Error: Debug logger lock is unavailable".to_string(),
    };
    logger
        .last_error
        .clone()
        .map(|error| format!("Error: {}", error))
        .unwrap_or_else(|| {
            if logger.enabled {
                logger
                    .desktop_dir
                    .as_ref()
                    .map(|path| format!("Logging to {}", path.display()))
                    .unwrap_or_else(|| "Debug logging is enabled".to_string())
            } else {
                "Debug logging is off".to_string()
            }
        })
}

#[tauri::command]
fn prevent_sleep(state: tauri::State<'_, AppState>) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        set_windows_power_request(&state, true)?;
    }

    Ok(())
}

#[tauri::command]
fn allow_sleep(state: tauri::State<'_, AppState>) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        set_windows_power_request(&state, false)?;
    }

    Ok(())
}

#[tauri::command]
fn get_ac_power_status() -> Option<bool> {
    #[cfg(target_os = "windows")]
    {
        use winapi::um::winbase::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

        let mut status: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
        if unsafe { GetSystemPowerStatus(&mut status) } == 0 {
            return None;
        }

        return match status.ACLineStatus {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        };
    }

    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

#[cfg(target_os = "windows")]
fn set_windows_power_request(state: &AppState, enabled: bool) -> Result<(), String> {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Power::{
        PowerClearRequest, PowerCreateRequest, PowerRequestDisplayRequired,
        PowerRequestSystemRequired, PowerSetRequest,
    };
    use windows::Win32::System::Threading::{
        POWER_REQUEST_CONTEXT_SIMPLE_STRING, REASON_CONTEXT, REASON_CONTEXT_0,
    };

    let mut request_slot = state
        .power_request
        .lock()
        .map_err(|_| "Power request state is unavailable".to_string())?;

    if enabled {
        if request_slot.is_some() {
            return Ok(());
        }

        let mut reason: Vec<u16> = "Never Sleep is keeping the PC awake"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let context = REASON_CONTEXT {
            Version: 0,
            Flags: POWER_REQUEST_CONTEXT_SIMPLE_STRING,
            Reason: REASON_CONTEXT_0 {
                SimpleReasonString: PWSTR(reason.as_mut_ptr()),
            },
        };
        let handle = unsafe { PowerCreateRequest(&context) }
            .map_err(|error| format!("Could not create Windows power request: {error}"))?;

        if let Err(error) = unsafe { PowerSetRequest(handle, PowerRequestDisplayRequired) } {
            let _ = unsafe { CloseHandle(handle) };
            return Err(format!("Could not prevent Windows idle lock: {error}"));
        }
        if let Err(error) = unsafe { PowerSetRequest(handle, PowerRequestSystemRequired) } {
            let _ = unsafe { PowerClearRequest(handle, PowerRequestDisplayRequired) };
            let _ = unsafe { CloseHandle(handle) };
            return Err(format!("Could not prevent Windows sleep: {error}"));
        }

        *request_slot = Some(handle.0);
        return Ok(());
    }

    if let Some(raw_handle) = request_slot.take() {
        let handle = HANDLE(raw_handle);
        let display_result = unsafe { PowerClearRequest(handle, PowerRequestDisplayRequired) };
        let system_result = unsafe { PowerClearRequest(handle, PowerRequestSystemRequired) };
        let close_result = unsafe { CloseHandle(handle) };
        display_result
            .map_err(|error| format!("Could not release display power request: {error}"))?;
        system_result
            .map_err(|error| format!("Could not release system power request: {error}"))?;
        close_result.map_err(|error| format!("Could not close Windows power request: {error}"))?;
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn set_windows_power_request(_state: &AppState, _enabled: bool) -> Result<(), String> {
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

#[cfg(target_os = "windows")]
fn get_windows_last_input_time() -> Option<u64> {
    use winapi::um::sysinfoapi::GetTickCount;
    use winapi::um::winuser::{GetLastInputInfo, LASTINPUTINFO};
    unsafe {
        let mut lii = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if GetLastInputInfo(&mut lii) != 0 {
            let current_tick = GetTickCount();
            let elapsed_ms = current_tick.wrapping_sub(lii.dwTime) as u64;
            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_millis() as u64;
            Some(now_ms.saturating_sub(elapsed_ms))
        } else {
            None
        }
    }
}

#[tauri::command]
fn get_last_activity(state: tauri::State<'_, AppState>) -> Result<(u64, String), String> {
    let mut activity = state.last_activity.safe_lock();
    #[cfg(target_os = "windows")]
    if let Some(win_input_time) = get_windows_last_input_time() {
        if win_input_time > activity.0.saturating_add(50) {
            *activity = (win_input_time, "mouse".to_string());
        }
    }
    Ok(activity.clone())
}

#[tauri::command]
fn record_input_activity(
    state: tauri::State<'_, AppState>,
    input_type: String,
) -> Result<(), String> {
    let normalized_type = if input_type == "mouse" {
        "mouse"
    } else {
        "keyboard"
    };
    let now: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis()
        .try_into()
        .unwrap_or(0);

    let mut last = state.last_activity.safe_lock();
    *last = (now, normalized_type.to_string());
    Ok(())
}

#[cfg(target_os = "windows")]
fn workstation_lock_state() -> Result<bool, String> {
    use winapi::um::winuser::{
        CloseDesktop, GetUserObjectInformationW, OpenInputDesktop, DESKTOP_READOBJECTS, UOI_NAME,
    };

    unsafe {
        let desktop = OpenInputDesktop(0, 0, DESKTOP_READOBJECTS);
        if desktop.is_null() {
            return Err(format!(
                "OpenInputDesktop failed: {}",
                std::io::Error::last_os_error()
            ));
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
        let query_error = if ok == 0 {
            Some(std::io::Error::last_os_error())
        } else {
            None
        };
        CloseDesktop(desktop);

        if let Some(error) = query_error {
            return Err(format!("GetUserObjectInformationW failed: {error}"));
        }
        if needed < std::mem::size_of::<u16>() as u32 {
            return Err("GetUserObjectInformationW returned an empty desktop name".into());
        }
        if needed as usize > name.len() * std::mem::size_of::<u16>() {
            return Err(
                "GetUserObjectInformationW returned a desktop name larger than the buffer".into(),
            );
        }

        let len = (needed as usize / std::mem::size_of::<u16>()).saturating_sub(1);
        let desktop_name = String::from_utf16_lossy(&name[..len]);
        if desktop_name.eq_ignore_ascii_case("Default") {
            Ok(false)
        } else if desktop_name.eq_ignore_ascii_case("Winlogon") {
            Ok(true)
        } else {
            Err(format!("Unexpected input desktop: {desktop_name}"))
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn workstation_lock_state() -> Result<bool, String> {
    Ok(false)
}

#[tauri::command]
fn is_workstation_locked() -> Result<bool, String> {
    workstation_lock_state()
}

#[tauri::command]
fn lock_workstation() -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        use winapi::um::winuser::LockWorkStation;
        let success = unsafe { LockWorkStation() };
        if success != 0 {
            Ok(true)
        } else {
            Err("Failed to lock workstation display".into())
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Lock workstation is only supported on Windows".into())
    }
}

static LAST_UNLOCK_TIME: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[tauri::command]
fn get_last_unlock_time() -> u64 {
    LAST_UNLOCK_TIME.load(std::sync::atomic::Ordering::Relaxed)
}

#[tauri::command]
async fn show_notification(
    app: tauri::AppHandle,
    title: String,
    body: String,
) -> Result<(), String> {
    use tauri::api::notification::Notification;

    if matches!(workstation_lock_state(), Ok(true)) {
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

fn create_overlay_window(
    app: &tauri::AppHandle,
    state: &tauri::State<'_, AppState>,
    url: &str,
    title: &str,
) -> Result<(), String> {
    let mut overlay_windows = state.overlay_windows.safe_lock();
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
    let mut failures = Vec::new();

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

        let window = match window {
            Ok(window) => window,
            Err(error) => {
                failures.push(format!("monitor {}: {}", i, error));
                continue;
            }
        };

        // Force maximize for Windows 11 compat
        let _ = window.maximize();

        // Use explicit Physical Position/Size to correctly cover monitor regardless of DPI
        let _ = window.set_position(tauri::Position::Physical(monitor.position().clone()));
        let _ = window.set_size(tauri::Size::Physical(monitor.size().clone()));

        if let Err(error) = window.show() {
            failures.push(format!("monitor {}: {}", i, error));
            let _ = window.close();
            continue;
        }

        overlay_windows.push(label);
    }

    if overlay_windows.is_empty() {
        if failures.is_empty() {
            return Err("Windows found no displays for the screen overlay".to_string());
        }
        return Err(format!(
            "Could not show a screen overlay on any display: {}",
            failures.join("; ")
        ));
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn create_laptop_overlay_window(
    app: &tauri::AppHandle,
    state: &tauri::State<'_, AppState>,
) -> Result<(), String> {
    let incompatible_mons = ddcci_incompatible_monitors()?;
    let main_window = app.get_window("main").ok_or("Main window not found")?;
    let monitors = main_window
        .available_monitors()
        .map_err(|e| format!("Failed to get monitors: {}", e))?;
    let mut overlay_windows = state.overlay_windows.safe_lock();

    for (i, monitor) in monitors.iter().enumerate() {
        let position = monitor.position();
        let size = monitor.size();
        let mon_name = monitor.name().map(|s| s.to_string()).unwrap_or_default();

        let is_laptop_display = incompatible_mons.iter().any(|info| {
            (!info.device_name.is_empty() && info.device_name == mon_name)
                || (position.x == info.x
                    && position.y == info.y
                    && size.width == info.width
                    && size.height == info.height)
        });

        if !is_laptop_display {
            continue;
        }

        let label = format!("overlay_laptop_{}", i);
        if let Some(existing) = app.get_window(&label) {
            let _ = existing.close();
        }

        let window =
            tauri::WindowBuilder::new(app, &label, tauri::WindowUrl::App("overlay.html".into()))
                .title("Laptop Screen Overlay")
                .decorations(false)
                .always_on_top(true)
                .resizable(true)
                .fullscreen(true)
                .visible(false)
                .build()
                .map_err(|e| format!("Failed to create laptop overlay: {}", e))?;

        let _ = window.maximize();
        let _ = window.set_position(tauri::Position::Physical(position.clone()));
        let _ = window.set_size(tauri::Size::Physical(size.clone()));
        window
            .show()
            .map_err(|e| format!("Failed to show laptop overlay: {}", e))?;
        let _ = window.set_focus();
        overlay_windows.push(label);
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn create_laptop_overlay_window(
    _app: &tauri::AppHandle,
    _state: &tauri::State<'_, AppState>,
) -> Result<(), String> {
    Err("Hybrid DDC/CI mode is only supported on Windows".to_string())
}

#[tauri::command]
async fn create_screen_overlay(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    method: Option<String>,
    custom_path: Option<String>,
) -> Result<(), String> {
    let m = method.unwrap_or_else(|| "ddcci".to_string());
    match m.as_str() {
        "win32" => create_darken_win32(),
        "gamma" => create_darken_gamma(),
        "ddcci" | "ddcci_laptop_overlay" => {
            match send_ddcci_vcp_power(4) {
                Ok(()) => {}
                Err(error) if error.contains("No DDC/CI compatible physical monitors") => {}
                Err(error) => return Err(error),
            }
            create_laptop_overlay_window(&app, &state)
        }
        "controlmymonitor" => execute_control_my_monitor(custom_path.as_deref(), "4"),
        "brightness" => create_darken_brightness(),
        _ => create_overlay_window(&app, &state, "overlay.html", "Screen Overlay"),
    }
}

#[tauri::command]
async fn close_screen_overlay(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    method: Option<String>,
    custom_path: Option<String>,
) -> Result<(), String> {
    let m = method.unwrap_or_else(|| "ddcci".to_string());
    let close_result = match m.as_str() {
        "win32" => close_darken_win32(),
        "gamma" => close_darken_gamma(),
        "ddcci" | "ddcci_laptop_overlay" => {
            let close_result = {
                let mut overlay_windows = state.overlay_windows.safe_lock();
                for label in overlay_windows.iter() {
                    if let Some(window) = app.get_window(label) {
                        let _ = window.close();
                    }
                }
                overlay_windows.clear();
                Ok::<(), String>(())
            };
            close_result?;

            match send_ddcci_vcp_power(1) {
                Ok(()) => Ok(()),
                Err(error) if error.contains("No DDC/CI compatible physical monitors") => Ok(()),
                Err(error) => Err(error),
            }
        }
        "controlmymonitor" => execute_control_my_monitor(custom_path.as_deref(), "1"),
        "brightness" => close_darken_brightness(),
        _ => {
            let mut overlay_windows = state.overlay_windows.safe_lock();
            for label in overlay_windows.iter() {
                if let Some(window) = app.get_window(label) {
                    let _ = window.close();
                }
            }
            overlay_windows.clear();
            Ok(())
        }
    };

    let now: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(0);
    {
        let mut last = state.last_activity.safe_lock();
        *last = (now, "mouse".to_string());
    }
    let _ = app.emit_all("darken-overlay-closed", ());

    close_result
}

// ===== KEYBOARD CLEANING MODE =====

static CLEANING_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static CLEANING_ESCAPE_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static CLEANING_LAST_ESCAPE_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static CLEANING_HOOK_THREAD_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static CLEANING_APP_HANDLE: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();
static CLEANING_WINDOWS: Mutex<Vec<String>> = Mutex::new(Vec::new());
// Raw Win32 HWND handles for secondary monitor black-screen windows (stored as usize for Send safety)
static CLEANING_WIN32_HWNDS: Mutex<Vec<usize>> = Mutex::new(Vec::new());

#[cfg(target_os = "windows")]
unsafe extern "system" fn low_level_keyboard_proc(
    code: std::os::raw::c_int,
    w_param: winapi::shared::minwindef::WPARAM,
    l_param: winapi::shared::minwindef::LPARAM,
) -> winapi::shared::minwindef::LRESULT {
    use winapi::um::winuser::{
        CallNextHookEx, KBDLLHOOKSTRUCT, VK_ESCAPE, WM_KEYDOWN, WM_SYSKEYDOWN,
    };

    if code >= 0 && CLEANING_ACTIVE.load(Ordering::SeqCst) {
        let kbd = &*(l_param as *const KBDLLHOOKSTRUCT);

        if kbd.vkCode == VK_ESCAPE as u32 {
            if w_param == WM_KEYDOWN as usize || w_param == WM_SYSKEYDOWN as usize {
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                let last_ms = CLEANING_LAST_ESCAPE_MS.load(Ordering::SeqCst);

                let next_count = if now_ms.saturating_sub(last_ms) > 3000 {
                    1
                } else {
                    CLEANING_ESCAPE_COUNT.load(Ordering::SeqCst) + 1
                };

                CLEANING_ESCAPE_COUNT.store(next_count, Ordering::SeqCst);
                CLEANING_LAST_ESCAPE_MS.store(now_ms, Ordering::SeqCst);

                if let Some(app) = CLEANING_APP_HANDLE.get() {
                    let _ = app.emit_all(
                        "cleaning-escape-progress",
                        serde_json::json!({ "count": next_count }),
                    );
                    if next_count >= 5 {
                        let app_clone = app.clone();
                        std::thread::spawn(move || {
                            let _ = stop_cleaning_mode_internal(&app_clone);
                        });
                    }
                }
            }
            return 1;
        }

        // Swallowing all other keystrokes
        return 1;
    }

    CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param)
}

#[cfg(target_os = "windows")]
fn start_keyboard_hook() -> Result<(), String> {
    if CLEANING_ACTIVE.load(Ordering::SeqCst) {
        return Ok(());
    }
    CLEANING_ACTIVE.store(true, Ordering::SeqCst);
    CLEANING_ESCAPE_COUNT.store(0, Ordering::SeqCst);
    CLEANING_LAST_ESCAPE_MS.store(0, Ordering::SeqCst);

    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);
    std::thread::spawn(move || {
        use winapi::um::errhandlingapi::GetLastError;
        use winapi::um::processthreadsapi::GetCurrentThreadId;
        use winapi::um::winuser::{
            DispatchMessageW, GetMessageW, PeekMessageW, SetWindowsHookExW, TranslateMessage,
            UnhookWindowsHookEx, MSG, PM_NOREMOVE, WH_KEYBOARD_LL,
        };

        unsafe {
            let thread_id = GetCurrentThreadId();
            CLEANING_HOOK_THREAD_ID.store(thread_id, Ordering::SeqCst);
            let mut msg: MSG = std::mem::zeroed();
            // Ensure the thread message queue exists before the caller can request shutdown.
            let _ = PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);

            let hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(low_level_keyboard_proc),
                std::ptr::null_mut(),
                0,
            );

            if hook.is_null() {
                let error_code = GetLastError();
                let _ = ready_tx.send(Err(format!(
                    "Failed to install the Windows keyboard hook (error {}).",
                    error_code
                )));
                CLEANING_ACTIVE.store(false, Ordering::SeqCst);
                CLEANING_HOOK_THREAD_ID.store(0, Ordering::SeqCst);
                return;
            }

            if ready_tx.send(Ok(())).is_err() {
                CLEANING_ACTIVE.store(false, Ordering::SeqCst);
                UnhookWindowsHookEx(hook);
                CLEANING_HOOK_THREAD_ID.store(0, Ordering::SeqCst);
                return;
            }

            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            UnhookWindowsHookEx(hook);
            CLEANING_HOOK_THREAD_ID.store(0, Ordering::SeqCst);
        }
    });

    match ready_rx.recv_timeout(std::time::Duration::from_secs(3)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(error),
        Err(_) => {
            stop_keyboard_hook();
            Err("Timed out while installing the Windows keyboard hook.".to_string())
        }
    }
}

#[cfg(target_os = "windows")]
fn stop_keyboard_hook() {
    CLEANING_ACTIVE.store(false, Ordering::SeqCst);
    let thread_id = CLEANING_HOOK_THREAD_ID.swap(0, Ordering::SeqCst);
    if thread_id != 0 {
        use winapi::um::winuser::{PostThreadMessageW, WM_QUIT};
        unsafe {
            let _ = PostThreadMessageW(thread_id, WM_QUIT, 0, 0);
        }
    }
}

#[cfg(target_os = "windows")]
fn create_black_win32_windows(rects: Vec<(i32, i32, i32, i32)>) {
    // Spawn a dedicated Win32 thread that owns the black-screen windows
    std::thread::spawn(move || unsafe {
        use winapi::shared::minwindef::UINT;
        use winapi::shared::windef::HWND;
        use winapi::um::libloaderapi::GetModuleHandleW;
        use winapi::um::winuser::{
            CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassExW,
            SetWindowPos, ShowWindow, TranslateMessage, MSG, SWP_NOACTIVATE, SW_SHOW, WM_USER,
            WNDCLASSEXW, WS_EX_NOACTIVATE, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
        };

        const WM_CLOSE_BLACK: UINT = WM_USER + 42;

        let class_name: Vec<u16> = "NeverSleepCleaningBlack\0".encode_utf16().collect();

        let hinstance = GetModuleHandleW(std::ptr::null());

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(DefWindowProcW),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: winapi::um::winuser::GetSysColorBrush(0), // COLOR_SCROLLBAR=0, gives black
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: std::ptr::null_mut(),
        };
        // Ignore error if class already registered
        let _ = RegisterClassExW(&wc);

        let mut hwnds: Vec<HWND> = Vec::new();
        for (x, y, w, h) in &rects {
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_NOACTIVATE,
                class_name.as_ptr(),
                std::ptr::null(),
                WS_POPUP | WS_VISIBLE,
                *x,
                *y,
                *w,
                *h,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                hinstance,
                std::ptr::null_mut(),
            );
            if !hwnd.is_null() {
                // Paint it black by setting background brush color
                use winapi::um::wingdi::{CreateSolidBrush, RGB};
                use winapi::um::winuser::{SetClassLongPtrW, GCLP_HBRBACKGROUND};
                let black_brush = CreateSolidBrush(RGB(0, 0, 0));
                SetClassLongPtrW(hwnd, GCLP_HBRBACKGROUND, black_brush as isize);
                use winapi::um::winuser::InvalidateRect;
                InvalidateRect(hwnd, std::ptr::null(), 1);
                SetWindowPos(
                    hwnd,
                    winapi::um::winuser::HWND_TOPMOST,
                    *x,
                    *y,
                    *w,
                    *h,
                    SWP_NOACTIVATE,
                );
                ShowWindow(hwnd, SW_SHOW);
                // Store as usize for thread-safety
                CLEANING_WIN32_HWNDS.safe_lock().push(hwnd as usize);
                hwnds.push(hwnd);
            }
        }

        // Message pump — stays alive until WM_CLOSE_BLACK
        let mut msg: MSG = std::mem::zeroed();
        loop {
            if !CLEANING_ACTIVE.load(Ordering::SeqCst) {
                break;
            }
            if GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) <= 0 {
                break;
            }
            if msg.message == WM_CLOSE_BLACK {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Destroy all black windows
        for hwnd in hwnds {
            use winapi::um::winuser::DestroyWindow;
            DestroyWindow(hwnd);
        }
        CLEANING_WIN32_HWNDS.safe_lock().clear();
    });
}

#[cfg(not(target_os = "windows"))]
fn create_black_win32_windows(_rects: Vec<(i32, i32, i32, i32)>) {}

#[cfg(target_os = "windows")]
fn close_black_win32_windows() {
    // CLEANING_ACTIVE is already set to false by stop_keyboard_hook().
    // The Win32 message pump loop checks this flag and will exit on its next wake.
    // Post WM_NULL to each black window so their GetMessageW call wakes up immediately.
    let hwnds = CLEANING_WIN32_HWNDS.safe_lock().clone();
    for hwnd_usize in hwnds {
        unsafe {
            use winapi::um::winuser::{PostMessageW, WM_NULL};
            let hwnd = hwnd_usize as winapi::shared::windef::HWND;
            PostMessageW(hwnd, WM_NULL, 0, 0);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn close_black_win32_windows() {}

fn stop_cleaning_mode_internal(app: &tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    stop_keyboard_hook();

    // Wake and destroy secondary monitor Win32 black windows
    close_black_win32_windows();

    let mut wins = CLEANING_WINDOWS.safe_lock();
    for label in wins.iter() {
        if let Some(win) = app.get_window(label) {
            let _ = win.close();
        }
    }
    wins.clear();
    let _ = app.emit_all("cleaning-mode-stopped", ());
    Ok(())
}

#[tauri::command]
async fn start_cleaning_mode(app: tauri::AppHandle) -> Result<(), String> {
    let _ = CLEANING_APP_HANDLE.set(app.clone());

    // Close any previous cleaning windows
    {
        let mut wins = CLEANING_WINDOWS.safe_lock();
        for label in wins.iter() {
            if let Some(win) = app.get_window(label) {
                let _ = win.close();
            }
        }
        wins.clear();
    }

    let main_window = app.get_window("main").ok_or("Main window not found")?;
    let monitors = main_window
        .available_monitors()
        .map_err(|e| e.to_string())?;

    let mut wins = CLEANING_WINDOWS.lock().unwrap();

    // Primary monitor (index 0): full cleaning.html WebView2 window
    // Secondary monitors: pure Win32 black windows (no WebView2 = no hook interference)
    let mut secondary_rects: Vec<(i32, i32, i32, i32)> = Vec::new();

    for (i, monitor) in monitors.iter().enumerate() {
        if i == 0 {
            let label = "cleaning_overlay_0".to_string();
            let window = tauri::WindowBuilder::new(
                &app,
                &label,
                tauri::WindowUrl::App("cleaning.html?primary=1".into()),
            )
            .title("Keyboard Locked for Cleaning")
            .decorations(false)
            .always_on_top(true)
            .resizable(false)
            .fullscreen(true)
            .visible(false)
            .build()
            .map_err(|e| format!("Failed to create cleaning window: {}", e))?;

            let _ = window.maximize();
            let _ = window.set_position(tauri::Position::Physical(monitor.position().clone()));
            let _ = window.set_size(tauri::Size::Physical(monitor.size().clone()));

            #[cfg(target_os = "windows")]
            if let Err(error) = start_keyboard_hook() {
                let _ = window.close();
                return Err(error);
            }

            let _ = window.show();
            let _ = window.set_focus();
            wins.push(label);
        } else {
            let pos = monitor.position();
            let size = monitor.size();
            secondary_rects.push((pos.x, pos.y, size.width as i32, size.height as i32));
        }
    }

    // Spawn pure Win32 black windows for secondary monitors (after hook is active)
    if !secondary_rects.is_empty() {
        create_black_win32_windows(secondary_rects);
    }

    let _ = app.emit_all("cleaning-mode-started", ());
    Ok(())
}

#[tauri::command]
async fn stop_cleaning_mode(app: tauri::AppHandle) -> Result<(), String> {
    stop_cleaning_mode_internal(&app)
}

// ===== BRIGHTNESS METHOD (via PowerShell WMI) =====

fn create_darken_brightness() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        // Read current brightness first and store it
        let get_output = std::process::Command::new("powershell")
            .args(&[
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "(Get-WmiObject -Namespace root/WMI -Class WmiMonitorBrightness).CurrentBrightness",
            ])
            .output()
            .map_err(|e| e.to_string())?;

        let brightness_str = String::from_utf8_lossy(&get_output.stdout);
        let current: i32 = brightness_str.trim().parse().unwrap_or(-1);
        PREV_BRIGHTNESS.store(current, std::sync::atomic::Ordering::SeqCst);

        // Set brightness to 0
        let set_output = std::process::Command::new("powershell")
            .args(&[
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "(Get-WmiObject -Namespace root/WMI -Class WmiMonitorBrightnessMethods).WmiSetBrightness(1,0)",
            ])
            .output()
            .map_err(|e| e.to_string())?;

        if !set_output.status.success() {
            return Err("WMI brightness control not available on this display".to_string());
        }
    }
    Ok(())
}

fn close_darken_brightness() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let prev = PREV_BRIGHTNESS.load(std::sync::atomic::Ordering::SeqCst);
        let restore = if prev >= 0 { prev } else { 70 };
        let cmd = format!(
            "(Get-WmiObject -Namespace root/WMI -Class WmiMonitorBrightnessMethods).WmiSetBrightness(1,{})",
            restore
        );
        let _ = std::process::Command::new("powershell")
            .args(&["-NoProfile", "-NonInteractive", "-Command", &cmd])
            .output();
        PREV_BRIGHTNESS.store(-1, std::sync::atomic::Ordering::SeqCst);
    }
    Ok(())
}

#[tauri::command]
fn get_screen_brightness() -> Result<i32, String> {
    #[cfg(target_os = "windows")]
    {
        let output = std::process::Command::new("powershell")
            .args(&[
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "(Get-WmiObject -Namespace root/WMI -Class WmiMonitorBrightness).CurrentBrightness",
            ])
            .output()
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout);
            return Ok(s.trim().parse::<i32>().unwrap_or(-1));
        }
    }
    Ok(-1) // -1 = not supported
}

// ===== WIN32 CRASH-FREE RAW WINDOW METHOD =====

fn create_darken_win32() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if WIN32_DARKEN_HWND.load(std::sync::atomic::Ordering::SeqCst) != 0 {
            return Ok(());
        }

        let (tx, rx) = std::sync::mpsc::channel::<isize>();

        std::thread::spawn(move || {
            use std::ptr;
            use winapi::shared::minwindef::{HINSTANCE, LPARAM, LRESULT, UINT, WPARAM};
            use winapi::shared::windef::{HBRUSH, HGDIOBJ, HWND};
            use winapi::um::libloaderapi::GetModuleHandleW;
            use winapi::um::winuser::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
                GetSystemMetrics, PostQuitMessage, RegisterClassW, SetWindowPos, ShowWindow,
                TranslateMessage, UpdateWindow, CS_HREDRAW, CS_VREDRAW, SM_CXVIRTUALSCREEN,
                SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_SHOW, WM_CLOSE,
                WM_DESTROY, WM_ERASEBKGND, WNDCLASSW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
            };

            #[link(name = "gdi32")]
            extern "system" {
                fn GetStockObject(i: i32) -> HGDIOBJ;
            }
            const BLACK_BRUSH: i32 = 4;
            const WS_EX_NOACTIVATE: u32 = 0x08000000;
            const SWP_SHOWWINDOW: u32 = 0x0040;
            const HWND_TOPMOST: isize = -1;

            unsafe extern "system" fn darken_wnd_proc(
                hwnd: HWND,
                msg: UINT,
                wparam: WPARAM,
                lparam: LPARAM,
            ) -> LRESULT {
                const WM_NCHITTEST: UINT = 0x0084;
                const WM_PAINT: UINT = 0x000F;
                match msg {
                    WM_NCHITTEST => -1, // HTTRANSPARENT: let mouse pass through
                    WM_ERASEBKGND => {
                        use winapi::shared::windef::RECT;
                        use winapi::um::winuser::{FillRect, GetClientRect};
                        let hdc = wparam as winapi::shared::windef::HDC;
                        let mut rc: RECT = std::mem::zeroed();
                        GetClientRect(hwnd, &mut rc);
                        let black_brush = GetStockObject(BLACK_BRUSH) as HBRUSH;
                        FillRect(hdc, &rc, black_brush);
                        1
                    }
                    WM_PAINT => {
                        use winapi::um::winuser::{BeginPaint, EndPaint, FillRect, PAINTSTRUCT};
                        let mut ps: PAINTSTRUCT = std::mem::zeroed();
                        let hdc = BeginPaint(hwnd, &mut ps);
                        let black_brush = GetStockObject(BLACK_BRUSH) as HBRUSH;
                        FillRect(hdc, &ps.rcPaint, black_brush);
                        EndPaint(hwnd, &ps);
                        0
                    }
                    WM_CLOSE => {
                        DestroyWindow(hwnd);
                        0
                    }
                    WM_DESTROY => {
                        PostQuitMessage(0);
                        0
                    }
                    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }

            unsafe {
                let class_name: Vec<u16> = "NeverSleepDarkenWndClass\0".encode_utf16().collect();
                let h_instance = GetModuleHandleW(ptr::null()) as HINSTANCE;
                let black_brush = GetStockObject(BLACK_BRUSH) as HBRUSH;

                let wc = WNDCLASSW {
                    style: CS_HREDRAW | CS_VREDRAW,
                    lpfnWndProc: Some(darken_wnd_proc),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: h_instance,
                    hIcon: ptr::null_mut(),
                    hCursor: ptr::null_mut(),
                    hbrBackground: black_brush,
                    lpszMenuName: ptr::null(),
                    lpszClassName: class_name.as_ptr(),
                };
                RegisterClassW(&wc);

                let title: Vec<u16> = "NeverSleepDarken\0".encode_utf16().collect();
                let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
                let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
                let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
                let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);

                let hwnd: HWND = CreateWindowExW(
                    WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    class_name.as_ptr(),
                    title.as_ptr(),
                    WS_POPUP | winapi::um::winuser::WS_VISIBLE,
                    x,
                    y,
                    w,
                    h,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    h_instance,
                    ptr::null_mut(),
                );

                if hwnd.is_null() {
                    let _ = tx.send(0);
                    return;
                }

                WIN32_DARKEN_HWND.store(hwnd as isize, std::sync::atomic::Ordering::SeqCst);
                let _ = tx.send(hwnd as isize);

                SetWindowPos(hwnd, HWND_TOPMOST as HWND, x, y, w, h, SWP_SHOWWINDOW);
                ShowWindow(hwnd, SW_SHOW);
                UpdateWindow(hwnd);

                let mut msg = std::mem::zeroed();
                while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                WIN32_DARKEN_HWND.store(0, std::sync::atomic::Ordering::SeqCst);
            }
        });

        match rx.recv_timeout(std::time::Duration::from_millis(1500)) {
            Ok(hwnd_val) => {
                if hwnd_val != 0 {
                    Ok(())
                } else {
                    Err("Failed to create Win32 darken window".to_string())
                }
            }
            Err(_) => Err("Timeout creating Win32 darken window".to_string()),
        }
    }
}

fn close_darken_win32() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use winapi::um::winuser::{PostMessageW, WM_CLOSE};
        let raw = WIN32_DARKEN_HWND.swap(0, std::sync::atomic::Ordering::SeqCst);
        if raw != 0 {
            unsafe {
                PostMessageW(raw as winapi::shared::windef::HWND, WM_CLOSE, 0, 0);
            }
        }
    }
    Ok(())
}

// ===== DESKTOP COLOR ENGINE (Windows Magnification API) =====

#[cfg(target_os = "windows")]
static COLOR_ENGINE_ACTIVE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn create_darken_gamma() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use winapi::um::libloaderapi::{GetProcAddress, LoadLibraryA};
        unsafe {
            let lib = LoadLibraryA(b"Magnification.dll\0".as_ptr() as *const i8);
            if lib.is_null() {
                return Err("Failed to load Magnification.dll".to_string());
            }

            type MagInitFn = unsafe extern "system" fn() -> winapi::shared::minwindef::BOOL;
            type MagSetEffectFn =
                unsafe extern "system" fn(*const f32) -> winapi::shared::minwindef::BOOL;

            let init_proc = GetProcAddress(lib, b"MagInitialize\0".as_ptr() as *const i8);
            let set_effect_proc =
                GetProcAddress(lib, b"MagSetFullscreenColorEffect\0".as_ptr() as *const i8);

            if init_proc.is_null() || set_effect_proc.is_null() {
                return Err("Failed to locate Magnification API entry points".to_string());
            }

            let mag_init: MagInitFn = std::mem::transmute(init_proc);
            let mag_set_effect: MagSetEffectFn = std::mem::transmute(set_effect_proc);

            if mag_init() == 0 {
                return Err("MagInitialize failed".to_string());
            }

            // 5x5 color matrix with zero RGB multipliers (transforms screen to pure black)
            let mut black_matrix = [0.0f32; 25];
            black_matrix[18] = 1.0; // alpha multiplier
            black_matrix[24] = 1.0; // identity scale

            if mag_set_effect(black_matrix.as_ptr()) == 0 {
                return Err("MagSetFullscreenColorEffect failed".to_string());
            }

            COLOR_ENGINE_ACTIVE.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
    Ok(())
}

fn close_darken_gamma() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if !COLOR_ENGINE_ACTIVE.swap(false, std::sync::atomic::Ordering::SeqCst) {
            return Ok(());
        }

        use winapi::um::libloaderapi::{GetProcAddress, LoadLibraryA};
        unsafe {
            let lib = LoadLibraryA(b"Magnification.dll\0".as_ptr() as *const i8);
            if !lib.is_null() {
                type MagSetEffectFn =
                    unsafe extern "system" fn(*const f32) -> winapi::shared::minwindef::BOOL;
                type MagUninitFn = unsafe extern "system" fn() -> winapi::shared::minwindef::BOOL;

                let set_effect_proc =
                    GetProcAddress(lib, b"MagSetFullscreenColorEffect\0".as_ptr() as *const i8);
                let uninit_proc = GetProcAddress(lib, b"MagUninitialize\0".as_ptr() as *const i8);

                if !set_effect_proc.is_null() {
                    let mag_set_effect: MagSetEffectFn = std::mem::transmute(set_effect_proc);
                    // 5x5 identity matrix
                    let mut identity = [0.0f32; 25];
                    identity[0] = 1.0;
                    identity[6] = 1.0;
                    identity[12] = 1.0;
                    identity[18] = 1.0;
                    identity[24] = 1.0;
                    let _ = mag_set_effect(identity.as_ptr());
                }

                if !uninit_proc.is_null() {
                    let mag_uninit: MagUninitFn = std::mem::transmute(uninit_proc);
                    let _ = mag_uninit();
                }
            }
        }
    }
    Ok(())
}

// ===== HARDWARE DDC/CI METHOD (Native dxva2.dll) =====

#[cfg(target_os = "windows")]
#[allow(non_snake_case)]
#[repr(C)]
#[derive(Clone, Copy)]
struct PHYSICAL_MONITOR {
    hPhysicalMonitor: *mut std::ffi::c_void,
    szPhysicalMonitorDescription: [u16; 128],
}

#[cfg(target_os = "windows")]
#[derive(Clone, Debug)]
struct IncompatibleMonitorInfo {
    device_name: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[cfg(target_os = "windows")]
fn ddcci_incompatible_monitors() -> Result<Vec<IncompatibleMonitorInfo>, String> {
    use winapi::shared::minwindef::{BOOL, LPARAM, TRUE};
    use winapi::shared::windef::{HDC, HMONITOR, LPRECT};
    use winapi::um::libloaderapi::{FreeLibrary, GetProcAddress, LoadLibraryA};
    use winapi::um::winuser::{EnumDisplayMonitors, GetMonitorInfoW, MONITORINFOEXW};

    type FnGetNumberOfPhysicalMonitors = unsafe extern "system" fn(HMONITOR, *mut u32) -> BOOL;
    type FnGetPhysicalMonitors =
        unsafe extern "system" fn(HMONITOR, u32, *mut PHYSICAL_MONITOR) -> BOOL;
    type FnGetVCPFeature =
        unsafe extern "system" fn(*mut std::ffi::c_void, u8, *mut u8, *mut u32, *mut u32) -> BOOL;
    type FnDestroyPhysicalMonitors = unsafe extern "system" fn(u32, *mut PHYSICAL_MONITOR) -> BOOL;

    struct MonitorContext {
        p_get_count: Option<FnGetNumberOfPhysicalMonitors>,
        p_get_mons: Option<FnGetPhysicalMonitors>,
        p_get_vcp: Option<FnGetVCPFeature>,
        p_destroy: Option<FnDestroyPhysicalMonitors>,
        incompatible: Vec<IncompatibleMonitorInfo>,
    }

    unsafe extern "system" fn monitor_enum_proc(
        h_monitor: HMONITOR,
        _: HDC,
        _: LPRECT,
        lparam: LPARAM,
    ) -> BOOL {
        let context = &mut *(lparam as *mut MonitorContext);
        let mut has_d6_support = false;

        if let (Some(p_get_count), Some(p_get_mons), Some(p_get_vcp), Some(p_destroy)) = (
            context.p_get_count,
            context.p_get_mons,
            context.p_get_vcp,
            context.p_destroy,
        ) {
            let mut count = 0u32;
            if p_get_count(h_monitor, &mut count) != 0 && count > 0 {
                let mut physical_mons = vec![
                    PHYSICAL_MONITOR {
                        hPhysicalMonitor: std::ptr::null_mut(),
                        szPhysicalMonitorDescription: [0; 128]
                    };
                    count as usize
                ];
                if p_get_mons(h_monitor, count, physical_mons.as_mut_ptr()) != 0 {
                    for mon in &physical_mons {
                        let mut vcp_type = 0u8;
                        let mut current_value = 0u32;
                        let mut maximum_value = 0u32;
                        if p_get_vcp(
                            mon.hPhysicalMonitor,
                            0xD6,
                            &mut vcp_type,
                            &mut current_value,
                            &mut maximum_value,
                        ) != 0
                        {
                            has_d6_support = true;
                            break;
                        }
                    }
                    p_destroy(count, physical_mons.as_mut_ptr());
                }
            }
        }

        if !has_d6_support {
            let mut info: MONITORINFOEXW = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
            if GetMonitorInfoW(h_monitor, &mut info as *mut _ as *mut _) != 0 {
                let len = info
                    .szDevice
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(info.szDevice.len());
                let dev_name = String::from_utf16_lossy(&info.szDevice[..len]);
                context.incompatible.push(IncompatibleMonitorInfo {
                    device_name: dev_name,
                    x: info.rcMonitor.left,
                    y: info.rcMonitor.top,
                    width: (info.rcMonitor.right - info.rcMonitor.left) as u32,
                    height: (info.rcMonitor.bottom - info.rcMonitor.top) as u32,
                });
            }
        }

        TRUE
    }

    unsafe {
        let dxva2 = LoadLibraryA("dxva2.dll\0".as_ptr() as *const i8);
        let mut p_get_count = None;
        let mut p_get_mons = None;
        let mut p_get_vcp = None;
        let mut p_destroy = None;

        if !dxva2.is_null() {
            let p1 = GetProcAddress(
                dxva2,
                "GetNumberOfPhysicalMonitorsFromHMONITOR\0".as_ptr() as *const i8,
            );
            let p2 = GetProcAddress(
                dxva2,
                "GetPhysicalMonitorsFromHMONITOR\0".as_ptr() as *const i8,
            );
            let p3 = GetProcAddress(
                dxva2,
                "GetVCPFeatureAndVCPFeatureReply\0".as_ptr() as *const i8,
            );
            let p4 = GetProcAddress(dxva2, "DestroyPhysicalMonitors\0".as_ptr() as *const i8);

            if !p1.is_null() && !p2.is_null() && !p3.is_null() && !p4.is_null() {
                p_get_count = Some(std::mem::transmute(p1));
                p_get_mons = Some(std::mem::transmute(p2));
                p_get_vcp = Some(std::mem::transmute(p3));
                p_destroy = Some(std::mem::transmute(p4));
            }
        }

        let mut context = MonitorContext {
            p_get_count,
            p_get_mons,
            p_get_vcp,
            p_destroy,
            incompatible: Vec::new(),
        };

        if EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            Some(monitor_enum_proc),
            &mut context as *mut _ as LPARAM,
        ) == 0
        {
            if !dxva2.is_null() {
                FreeLibrary(dxva2);
            }
            return Err("Windows could not enumerate displays".to_string());
        }

        if !dxva2.is_null() {
            FreeLibrary(dxva2);
        }

        Ok(context.incompatible)
    }
}

#[cfg(target_os = "windows")]
#[allow(non_snake_case)]
fn send_ddcci_vcp_power(power_value: u32) -> Result<(), String> {
    use winapi::shared::minwindef::{BOOL, LPARAM, TRUE};
    use winapi::shared::windef::{HDC, HMONITOR, LPRECT};
    use winapi::um::libloaderapi::{FreeLibrary, GetProcAddress, LoadLibraryA};
    use winapi::um::winuser::EnumDisplayMonitors;

    type FnGetNumberOfPhysicalMonitors =
        unsafe extern "system" fn(hMonitor: HMONITOR, pdw: *mut u32) -> BOOL;
    type FnGetPhysicalMonitors = unsafe extern "system" fn(
        hMonitor: HMONITOR,
        count: u32,
        pArray: *mut PHYSICAL_MONITOR,
    ) -> BOOL;
    type FnSetVCPFeature = unsafe extern "system" fn(
        hMonitor: *mut std::ffi::c_void,
        vcp_code: u8,
        new_val: u32,
    ) -> BOOL;
    type FnGetVCPFeature = unsafe extern "system" fn(
        hMonitor: *mut std::ffi::c_void,
        vcp_code: u8,
        vcp_type: *mut u8,
        current_value: *mut u32,
        maximum_value: *mut u32,
    ) -> BOOL;
    type FnDestroyPhysicalMonitors =
        unsafe extern "system" fn(count: u32, pArray: *mut PHYSICAL_MONITOR) -> BOOL;

    unsafe {
        let dxva2 = LoadLibraryA("dxva2.dll\0".as_ptr() as *const i8);
        if dxva2.is_null() {
            return Err("dxva2.dll not available on this system".to_string());
        }

        let p_get_count_raw = GetProcAddress(
            dxva2,
            "GetNumberOfPhysicalMonitorsFromHMONITOR\0".as_ptr() as *const i8,
        );
        let p_get_mons_raw = GetProcAddress(
            dxva2,
            "GetPhysicalMonitorsFromHMONITOR\0".as_ptr() as *const i8,
        );
        let p_set_vcp_raw = GetProcAddress(dxva2, "SetVCPFeature\0".as_ptr() as *const i8);
        let p_get_vcp_raw = GetProcAddress(
            dxva2,
            "GetVCPFeatureAndVCPFeatureReply\0".as_ptr() as *const i8,
        );
        let p_destroy_raw =
            GetProcAddress(dxva2, "DestroyPhysicalMonitors\0".as_ptr() as *const i8);

        if p_get_count_raw.is_null()
            || p_get_mons_raw.is_null()
            || p_set_vcp_raw.is_null()
            || p_get_vcp_raw.is_null()
            || p_destroy_raw.is_null()
        {
            FreeLibrary(dxva2);
            return Err("DDC/CI functions not found in dxva2.dll".to_string());
        }

        let p_get_count: FnGetNumberOfPhysicalMonitors = std::mem::transmute(p_get_count_raw);
        let p_get_mons: FnGetPhysicalMonitors = std::mem::transmute(p_get_mons_raw);
        let p_set_vcp: FnSetVCPFeature = std::mem::transmute(p_set_vcp_raw);
        let p_get_vcp: FnGetVCPFeature = std::mem::transmute(p_get_vcp_raw);
        let p_destroy: FnDestroyPhysicalMonitors = std::mem::transmute(p_destroy_raw);

        struct MonitorContext {
            p_get_count: FnGetNumberOfPhysicalMonitors,
            p_get_mons: FnGetPhysicalMonitors,
            p_set_vcp: FnSetVCPFeature,
            p_get_vcp: FnGetVCPFeature,
            p_destroy: FnDestroyPhysicalMonitors,
            power_value: u32,
            success_count: u32,
            physical_monitor_count: u32,
            verified_on_count: u32,
        }

        unsafe extern "system" fn monitor_enum_proc(
            h_monitor: HMONITOR,
            _: HDC,
            _: LPRECT,
            lparam: LPARAM,
        ) -> BOOL {
            let ctx = &mut *(lparam as *mut MonitorContext);
            let mut count = 0u32;
            if (ctx.p_get_count)(h_monitor, &mut count) != 0 && count > 0 {
                let mut physical_mons = vec![
                    PHYSICAL_MONITOR {
                        hPhysicalMonitor: std::ptr::null_mut(),
                        szPhysicalMonitorDescription: [0; 128]
                    };
                    count as usize
                ];
                if (ctx.p_get_mons)(h_monitor, count, physical_mons.as_mut_ptr()) != 0 {
                    for mon in &physical_mons {
                        let mut vcp_type = 0u8;
                        let mut current_value = 0u32;
                        let mut maximum_value = 0u32;
                        let supports_d6 = (ctx.p_get_vcp)(
                            mon.hPhysicalMonitor,
                            0xD6,
                            &mut vcp_type,
                            &mut current_value,
                            &mut maximum_value,
                        ) != 0;

                        if supports_d6 {
                            ctx.physical_monitor_count += 1;
                            // VCP 0xD6 is standard VESA Power Mode: 4 = Standby/Off, 1 = On
                            if (ctx.p_set_vcp)(mon.hPhysicalMonitor, 0xD6, ctx.power_value) != 0 {
                                ctx.success_count += 1;
                            }
                            if ctx.power_value == 1 {
                                let mut vcp_type2 = 0u8;
                                let mut cur2 = 0u32;
                                let mut max2 = 0u32;
                                if (ctx.p_get_vcp)(
                                    mon.hPhysicalMonitor,
                                    0xD6,
                                    &mut vcp_type2,
                                    &mut cur2,
                                    &mut max2,
                                ) != 0
                                    && cur2 == 1
                                {
                                    ctx.verified_on_count += 1;
                                }
                            }
                        }
                    }
                    (ctx.p_destroy)(count, physical_mons.as_mut_ptr());
                }
            }
            TRUE
        }

        let mut ctx = MonitorContext {
            p_get_count,
            p_get_mons,
            p_set_vcp,
            p_get_vcp,
            p_destroy,
            power_value,
            success_count: 0,
            physical_monitor_count: 0,
            verified_on_count: 0,
        };

        // Some monitors need a short delay between DDC/CI commands, especially
        // when several displays are connected through the same GPU adapter.
        let attempts = 5;
        for attempt in 0..attempts {
            ctx.success_count = 0;
            ctx.physical_monitor_count = 0;
            ctx.verified_on_count = 0;

            EnumDisplayMonitors(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                Some(monitor_enum_proc),
                &mut ctx as *mut _ as LPARAM,
            );

            let all_monitors_succeeded = ctx.physical_monitor_count > 0
                && ctx.success_count == ctx.physical_monitor_count
                && (power_value != 1 || ctx.verified_on_count == ctx.physical_monitor_count);

            if all_monitors_succeeded {
                break;
            }

            if attempt + 1 < attempts {
                std::thread::sleep(std::time::Duration::from_millis(350));
            }
        }

        FreeLibrary(dxva2);

        if ctx.physical_monitor_count == 0 {
            Err("No DDC/CI compatible physical monitors responded".to_string())
        } else if ctx.physical_monitor_count > 0
            && ctx.success_count == ctx.physical_monitor_count
            && (power_value != 1 || ctx.verified_on_count == ctx.physical_monitor_count)
        {
            Ok(())
        } else if power_value == 1 {
            Err(format!(
                "Not all DDC/CI monitors reported On ({} of {} verified)",
                ctx.verified_on_count, ctx.physical_monitor_count
            ))
        } else {
            Err(format!(
                "Not all DDC/CI monitors accepted standby ({} of {} succeeded)",
                ctx.success_count, ctx.physical_monitor_count
            ))
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn send_ddcci_vcp_power(_power_value: u32) -> Result<(), String> {
    Err("DDC/CI is only supported on Windows".to_string())
}

// ===== CONTROLMYMONITOR (NirSoft) METHOD =====

fn find_control_my_monitor(custom_path: Option<&str>) -> Option<std::path::PathBuf> {
    if let Some(cp) = custom_path {
        let trimmed = cp.trim();
        if !trimmed.is_empty() {
            let p = std::path::PathBuf::from(trimmed);
            if p.is_file() {
                return Some(p);
            }
        }
    }

    // 1. Next to current executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let local_p = dir.join("ControlMyMonitor.exe");
            if local_p.is_file() {
                return Some(local_p);
            }
        }
    }

    // 2. Standard install locations
    let candidates = [
        "C:\\Program Files\\NirSoft\\ControlMyMonitor.exe",
        "C:\\Program Files (x86)\\NirSoft\\ControlMyMonitor.exe",
        "C:\\Tools\\ControlMyMonitor.exe",
        "C:\\NirSoft\\ControlMyMonitor.exe",
    ];
    for c in &candidates {
        let p = std::path::PathBuf::from(c);
        if p.is_file() {
            return Some(p);
        }
    }

    // 3. LocalAppData NirSoft
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let p = std::path::PathBuf::from(local_app_data)
            .join("NirSoft")
            .join("ControlMyMonitor.exe");
        if p.is_file() {
            return Some(p);
        }
    }

    // 4. In PATH
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let p = dir.join("ControlMyMonitor.exe");
            if p.is_file() {
                return Some(p);
            }
        }
    }

    None
}

#[tauri::command]
fn check_control_my_monitor(custom_path: Option<String>) -> Result<String, String> {
    match find_control_my_monitor(custom_path.as_deref()) {
        Some(p) => Ok(p.to_string_lossy().to_string()),
        None => Err("ControlMyMonitor.exe not found".to_string()),
    }
}

fn execute_control_my_monitor(custom_path: Option<&str>, value: &str) -> Result<(), String> {
    let exe = find_control_my_monitor(custom_path)
        .ok_or_else(|| "ControlMyMonitor.exe not found on system".to_string())?;

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let _ = std::process::Command::new(exe)
            .args(&["/SetValueAll", "D6", value])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Failed to run ControlMyMonitor: {}", e))?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new(exe)
            .args(&["/SetValueAll", "D6", value])
            .spawn()
            .map_err(|e| format!("Failed to run ControlMyMonitor: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
fn set_autostart(enable: bool, minimized: bool) -> Result<(), String> {
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
            let value = if minimized {
                format!("\"{}\" --minimized", exe_path)
            } else {
                format!("\"{}\"", exe_path)
            };
            key.set_value("NeverSleepTauri", &value)
                .map_err(|e| e.to_string())?;
        } else {
            key.delete_value("NeverSleepTauri").ok();
        }
    }
    Ok(())
}

#[tauri::command]
fn check_autostart() -> Result<serde_json::Value, String> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let path = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
        if let Ok(key) = hkcu.open_subkey(path) {
            let val: Result<String, _> = key.get_value("NeverSleepTauri");
            if let Ok(v) = val {
                let minimized = v.contains("--minimized");
                return Ok(serde_json::json!({ "enabled": true, "minimized": minimized }));
            }
        }
    }
    Ok(serde_json::json!({ "enabled": false, "minimized": false }))
}

#[tauri::command]
fn hide_window(window: tauri::Window) {
    let _ = window.hide();
}

#[tauri::command]
fn minimize_window(window: tauri::Window) {
    let _ = window.minimize();
}

fn restore_main_window(window: &tauri::Window, take_focus: bool) {
    if window.is_minimized().unwrap_or(false) {
        let _ = window.unminimize();
    }
    let _ = window.show();

    let pinned = MAIN_WINDOW_PINNED.load(Ordering::SeqCst);
    if take_focus {
        if !pinned {
            // Preserve the existing Windows focus workaround only while unpinned.
            let _ = window.set_always_on_top(true);
            let _ = window.set_focus();
            let _ = window.set_always_on_top(false);
        } else {
            let _ = window.set_focus();
        }
    }

    // Reapply after show/unminimize so native window restoration cannot lose the saved state.
    let _ = window.set_always_on_top(pinned);
}

#[tauri::command]
fn focus_main_window(window: tauri::Window) {
    restore_main_window(&window, true);
}

#[tauri::command]
fn show_main_window(window: tauri::Window) {
    restore_main_window(&window, false);
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
fn is_classic_only() -> bool {
    // 1. Check for classic_only.flag next to current exe
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            if dir.join("classic_only.flag").exists() {
                return true;
            }
        }
    }

    // 2. Check Windows Registry (HKCU\Software\neversleep\Never Sleep)
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(sub) = hkcu.open_subkey("Software\\neversleep\\Never Sleep") {
            if let Ok(val) = sub.get_value::<u32, _>("ClassicOnly") {
                if val == 1 {
                    return true;
                }
            }
        }
    }

    false
}

#[tauri::command]
fn save_settings(app: tauri::AppHandle, mut settings: String) -> Result<(), String> {
    if is_classic_only() {
        if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&settings) {
            if let Some(obj) = json.as_object_mut() {
                obj.insert(
                    "viewMode".to_string(),
                    serde_json::Value::String("classic".to_string()),
                );
                if let Ok(updated) = serde_json::to_string(&json) {
                    settings = updated;
                }
            }
        }
    }
    let path = get_settings_path(app.clone())?;
    let previous_settings = std::fs::read_to_string(&path).ok();
    std::fs::write(path, &settings).map_err(|e| e.to_string())?;
    let debug_logging_enabled = serde_json::from_str::<serde_json::Value>(&settings)
        .ok()
        .and_then(|json| {
            json.get("debugLoggingEnabled")
                .and_then(|value| value.as_bool())
        })
        .unwrap_or(false);
    if let Some(state) = app.try_state::<AppState>() {
        set_debug_logging(&app, &state, debug_logging_enabled);
        if debug_logging_enabled {
            if let Some(previous) = previous_settings.as_deref() {
                if let Some(summary) = summarize_settings_changes(previous, &settings) {
                    let _ = append_debug_log(&app, &state, &summary);
                }
            }
        }
    }
    let _ = app.emit_all("settings-updated", settings);
    Ok(())
}

fn summarize_settings_changes(previous: &str, current: &str) -> Option<String> {
    let previous = serde_json::from_str::<serde_json::Value>(previous).ok()?;
    let current = serde_json::from_str::<serde_json::Value>(current).ok()?;
    let previous = previous.as_object()?;
    let current = current.as_object()?;
    let mut changes = Vec::new();

    for (key, value) in current {
        if previous.get(key) != Some(value) {
            changes.push(format!(
                "{}={}",
                key,
                serde_json::to_string(value).unwrap_or_else(|_| "<unavailable>".to_string())
            ));
        }
    }
    for key in previous.keys() {
        if !current.contains_key(key) {
            changes.push(format!("{}=<removed>", key));
        }
    }

    if changes.is_empty() {
        None
    } else {
        Some(format!("Settings changed: {}", changes.join(", ")))
    }
}

#[tauri::command]
fn load_settings(app: tauri::AppHandle) -> Result<String, String> {
    let path = get_settings_path(app)?;
    if path.exists() {
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        if is_classic_only() {
            if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(obj) = json.as_object_mut() {
                    obj.insert(
                        "viewMode".to_string(),
                        serde_json::Value::String("classic".to_string()),
                    );
                    if let Ok(updated) = serde_json::to_string(&json) {
                        return Ok(updated);
                    }
                }
            }
        }
        Ok(content)
    } else {
        if is_classic_only() {
            Ok(r#"{"viewMode":"classic"}"#.to_string())
        } else {
            Ok("{}".to_string())
        }
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
        let _ = window
            .eval("if (window.refreshSettingsFromStorage) window.refreshSettingsFromStorage();");
        if let Some(sec) = &section {
            let _ = window.eval(&format!(
                "if (window.showOnlySection) window.showOnlySection('{}');",
                sec
            ));
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
        let _ = tauri::WindowBuilder::new(&app, "settings", tauri::WindowUrl::App(url_str.into()))
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
    if let Some(state) = app.try_state::<AppState>() {
        let _ = append_debug_log(&app, &state, "Application is shutting down");
    }
    app.exit(0);
}

#[tauri::command]
fn restart_app(app: tauri::AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        let _ = append_debug_log(&app, &state, "Application is restarting");
    }
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
        "Stylish 3D Coffee Cup Icon with Steam Effect - Woopicx.png" => Ok(include_bytes!(
            "../../dist/Stylish 3D Coffee Cup Icon with Steam Effect - Woopicx.png"
        ) as &[u8]),
        "Stylish 3D Coffee Cup Icon with Steam and Isometric Design.png" => Ok(include_bytes!(
            "../../dist/Stylish 3D Coffee Cup Icon with Steam and Isometric Design.png"
        )
            as &[u8]),
        "Stylized Coffee Machine Icon with White Cup - Woopicx.png" => Ok(include_bytes!(
            "../../dist/Stylized Coffee Machine Icon with White Cup - Woopicx.png"
        ) as &[u8]),
        "A visually striking illustration.png" => {
            Ok(include_bytes!("../../dist/A visually striking illustration.png") as &[u8])
        }
        "3D Battery Icon with Lightning Bolt in Isometric View.png" => Ok(include_bytes!(
            "../../dist/3D Battery Icon with Lightning Bolt in Isometric View.png"
        ) as &[u8]),
        _ => Err(format!("Unknown icon name: {}", filename)),
    }
}

#[cfg(target_os = "windows")]
fn load_normalized_icon_image(filename: &str) -> Result<image::RgbaImage, String> {
    let img = image::load_from_memory(embedded_icon_bytes(filename)?)
        .map_err(|e| format!("Failed to decode embedded image {}: {}", filename, e))?;

    let image = img.to_rgba8();

    let resized = image::imageops::resize(&image, 256, 256, image::imageops::FilterType::Lanczos3);

    Ok(resized)
}

#[cfg(target_os = "windows")]
fn create_multi_res_ico(filename: &str) -> Result<Vec<u8>, String> {
    let raw_bytes = embedded_icon_bytes(filename)?;
    let base_img =
        image::load_from_memory(raw_bytes).map_err(|e| format!("Failed to decode image: {}", e))?;

    let rgba_base = base_img.to_rgba8();

    let sizes = [16, 24, 32, 48, 64, 128, 256];
    let mut png_images = Vec::new();

    for &sz in &sizes {
        let resized =
            image::imageops::resize(&rgba_base, sz, sz, image::imageops::FilterType::Lanczos3);
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
fn write_current_icon_file(
    app: &tauri::AppHandle,
    filename: &str,
) -> Result<std::path::PathBuf, String> {
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
fn apply_native_window_icons(
    app: &tauri::AppHandle,
    icon_path: &std::path::Path,
) -> Result<(), String> {
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
            window
                .set_icon(icon.clone())
                .map_err(|e| format!("Failed to set main window icon: {}", e))?;
        }

        // Also apply to settings window if open
        if let Some(window) = app.get_window("settings") {
            window
                .set_icon(icon.clone())
                .map_err(|e| format!("Failed to set settings window icon: {}", e))?;
        }

        // Update system tray icon
        app.tray_handle()
            .set_icon(icon)
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
            escaped_icon_path, escaped_exe_path
        );

        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ])
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
async fn check_for_update(current_version: String) -> Result<serde_json::Value, String> {
    // Scan ALL releases to find the highest version by asset filename
    let api_url = "https://api.github.com/repos/netanel3000fine/NeverSleep-App/releases";

    let json_text = std::thread::spawn(move || {
        let client = reqwest::blocking::Client::builder()
            .user_agent("NeverSleep-Updater/1.0")
            .build()
            .map_err(|e| e.to_string())?;
        let resp = client
            .get(api_url)
            .header("Accept", "application/vnd.github+json")
            .send()
            .map_err(|e| e.to_string())?;
        resp.text().map_err(|e| e.to_string())
    })
    .join()
    .map_err(|_| "Thread panicked".to_string())??;

    let releases: serde_json::Value =
        serde_json::from_str(&json_text).map_err(|e| format!("JSON parse error: {}", e))?;

    fn parse_ver(s: &str) -> (u32, u32, u32) {
        let p: Vec<u32> = s
            .trim_start_matches('v')
            .split('.')
            .filter_map(|x| x.parse().ok())
            .collect();
        if p.len() == 3 {
            (p[0], p[1], p[2])
        } else {
            (0, 0, 0)
        }
    }
    fn ver_str(v: (u32, u32, u32)) -> String {
        format!("{}.{}.{}", v.0, v.1, v.2)
    }
    fn is_newer(a: (u32, u32, u32), b: (u32, u32, u32)) -> bool {
        a.0 > b.0 || (a.0 == b.0 && a.1 > b.1) || (a.0 == b.0 && a.1 == b.1 && a.2 > b.2)
    }

    let cur = parse_ver(&current_version);
    let mut best_ver: (u32, u32, u32) = (0, 0, 0);
    let mut best_url = String::new();

    if let Some(arr) = releases.as_array() {
        for release in arr {
            if let Some(assets) = release["assets"].as_array() {
                for asset in assets {
                    let name = asset["name"].as_str().unwrap_or("");
                    let url = asset["browser_download_url"].as_str().unwrap_or("");
                    // Match: Never.Sleep_14.3.0_x64-setup.exe  or  Never Sleep_14.3.0_x64-setup.exe
                    if name.contains("x64-setup") && name.ends_with(".exe") && !url.is_empty() {
                        // Extract version from filename: any 3-part number group
                        let parts: Vec<&str> = name
                            .split(|c: char| !c.is_alphanumeric() && c != '.')
                            .filter(|s| !s.is_empty())
                            .collect();
                        for part in &parts {
                            let v = parse_ver(part);
                            if v != (0, 0, 0) && is_newer(v, best_ver) {
                                best_ver = v;
                                best_url = url.to_string();
                            }
                        }
                    }
                }
            }
        }
    }

    if best_ver == (0, 0, 0) {
        return Ok(serde_json::json!({ "shouldUpdate": false }));
    }

    if is_newer(best_ver, cur) {
        Ok(serde_json::json!({
            "shouldUpdate": true,
            "version": ver_str(best_ver),
            "url": best_url
        }))
    } else {
        Ok(serde_json::json!({ "shouldUpdate": false }))
    }
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
    MAIN_WINDOW_PINNED.store(pinned, Ordering::SeqCst);
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

/// Clips a window to a rounded rectangle using SetWindowRgn.
/// This makes DWM acrylic visually respect the CSS border-radius.
/// radius_px: logical pixels (will be scaled to physical pixels).
#[cfg(target_os = "windows")]
#[allow(dead_code)]
unsafe fn apply_rounded_window_region(
    hwnd: winapi::shared::windef::HWND,
    width_px: i32,
    height_px: i32,
    radius_px: i32,
) {
    use winapi::um::wingdi::CreateRoundRectRgn;
    use winapi::um::winuser::SetWindowRgn;
    let rgn = CreateRoundRectRgn(0, 0, width_px, height_px, radius_px * 2, radius_px * 2);
    // SetWindowRgn takes ownership of the region; no need to DeleteObject after
    SetWindowRgn(hwnd, rgn, 1 /* bRedraw = TRUE */);
}

#[cfg(target_os = "windows")]
unsafe fn clear_window_region(hwnd: winapi::shared::windef::HWND) {
    use winapi::um::winuser::SetWindowRgn;
    SetWindowRgn(hwnd, std::ptr::null_mut(), 1);
}

#[tauri::command]
fn set_window_effect(app: tauri::AppHandle, effect: String) {
    #[cfg(target_os = "windows")]
    {
        use tauri::Manager;
        use window_vibrancy::clear_acrylic;

        let _ = effect; // All windows use high-fidelity CSS glassmorphism, avoiding Windows 11 DWM titlebar/frame glitches

        if let Some(main_win) = app.get_window("main") {
            let _ = clear_acrylic(&main_win);
            let _ = main_win.set_decorations(false);
            let _ = main_win.set_resizable(false);
            if let Ok(hwnd_raw) = main_win.hwnd() {
                let hwnd = hwnd_raw.0 as winapi::shared::windef::HWND;
                unsafe {
                    clear_window_region(hwnd);
                }
            }
        }
        if let Some(settings_win) = app.get_window("settings") {
            let _ = clear_acrylic(&settings_win);
            let _ = settings_win.set_resizable(false);
            if let Ok(hwnd_raw) = settings_win.hwnd() {
                let hwnd = hwnd_raw.0 as winapi::shared::windef::HWND;
                unsafe {
                    clear_window_region(hwnd);
                }
            }
        }
        if let Some(tray_win) = app.get_window("tray") {
            let _ = clear_acrylic(&tray_win);
            let _ = tray_win.set_decorations(false);
            let _ = tray_win.set_resizable(false);
            if let Ok(hwnd_raw) = tray_win.hwnd() {
                let hwnd = hwnd_raw.0 as winapi::shared::windef::HWND;
                unsafe {
                    clear_window_region(hwnd);
                }
            }
        }
    }
}

fn position_main_window_top_center(window: &tauri::Window, logical_width: Option<f64>) {
    let Some(monitor) = window.primary_monitor().ok().flatten() else {
        return;
    };
    let scale_factor = monitor.scale_factor();
    let width_px = logical_width
        .map(|width| (width * scale_factor).round() as i32)
        .or_else(|| window.outer_size().ok().map(|size| size.width as i32));
    let Some(width_px) = width_px else {
        return;
    };

    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let x = monitor_position.x + ((monitor_size.width as i32 - width_px) / 2);
    let y = monitor_position.y + (16.0 * scale_factor).round() as i32;
    let _ = window.set_position(tauri::Position::Physical(
        tauri::PhysicalPosition::new(x, y),
    ));
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
        position_main_window_top_center(&window, Some(width as f64));
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
        let _ = window.set_decorations(false);
    }
}

#[tauri::command]
fn set_decorations(app: tauri::AppHandle, decorations: bool) {
    if let Some(window) = app.get_window("main") {
        let _ = window.set_decorations(decorations);
    }
}

#[cfg(target_os = "windows")]
fn get_media_manager(
) -> Option<&'static windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager> {
    static MEDIA_MANAGER: std::sync::OnceLock<
        Option<windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager>,
    > = std::sync::OnceLock::new();
    MEDIA_MANAGER
        .get_or_init(|| {
            use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;
            GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
                .ok()
                .and_then(|op| op.get().ok())
        })
        .as_ref()
}

#[tauri::command]
async fn is_media_playing() -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackStatus;

        let mut playing = false;
        if let Some(manager) = get_media_manager() {
            if let Ok(sessions) = manager.GetSessions() {
                if let Ok(count) = sessions.Size() {
                    for index in 0..count {
                        let session = match sessions.GetAt(index) {
                            Ok(value) => value,
                            Err(_) => continue,
                        };
                        let info = match session.GetPlaybackInfo() {
                            Ok(value) => value,
                            Err(_) => continue,
                        };
                        let status = match info.PlaybackStatus() {
                            Ok(value) => value,
                            Err(_) => continue,
                        };
                        if status
                            != GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing
                        {
                            continue;
                        }

                        if let Ok(app_id) = session.SourceAppUserModelId() {
                            let app_id_lower = app_id.to_string().to_lowercase();
                            // Browsers can report web video sessions as Music, so
                            // identify actual playback by status rather than type.
                            if app_id_lower.contains("spotify")
                                || app_id_lower.contains("itunes")
                                || app_id_lower.contains("apple music")
                            {
                                continue;
                            }
                        }
                        playing = true;
                        break;
                    }
                }
            }
        }
        return playing;
    }
    #[allow(unreachable_code)]
    false
}

#[tauri::command]
fn update_tray_menu_state(
    _app: tauri::AppHandle,
    _status_text: String,
    _disabled: bool,
    _keep_awake: bool,
    _schedule_mode_active: bool,
    _active_duration_mins: Option<u32>,
    _media_enabled: bool,
    _notifications_enabled: bool,
    _pinned: bool,
    _autostart: bool,
    _darken_method: Option<String>,
) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
fn toggle_app_disabled(app: tauri::AppHandle) -> Result<bool, String> {
    // The main window owns the state transition; emitting only avoids toggling twice.
    let _ = app.emit_all("tray-action", "disable_app");
    Ok(true)
}

#[tauri::command]
fn resize_tray_window(app: tauri::AppHandle, height: f64) {
    if let Some(tray_win) = app.get_window("tray") {
        let sf = if let Some(monitor) = tray_win.primary_monitor().ok().flatten() {
            monitor.scale_factor()
        } else {
            1.0
        };
        let (_screen_w, screen_h) = if let Some(monitor) = tray_win.primary_monitor().ok().flatten()
        {
            let s = monitor.size();
            (s.width as f64 / sf, s.height as f64 / sf)
        } else {
            (1920.0, 1080.0)
        };
        let current_pos = tray_win.outer_position().unwrap_or_default();
        let current_size = tray_win.outer_size().unwrap_or_default();
        let current_h = current_size.height as f64 / sf;
        let current_x = current_pos.x as f64 / sf;
        let current_y = current_pos.y as f64 / sf;

        let target_h = height.max(150.0).min(500.0);
        let new_y = current_y + (current_h - target_h);

        let _ = tray_win.set_size(tauri::Size::Logical(tauri::LogicalSize {
            width: 320.0,
            height: target_h,
        }));
        let _ = tray_win.set_position(tauri::LogicalPosition::new(
            current_x,
            new_y.max(8.0).min(screen_h - target_h - 8.0),
        ));
    }
}

fn clear_webview_http_cache() {
    #[cfg(target_os = "windows")]
    {
        if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
            let base = std::path::PathBuf::from(local_appdata)
                .join("com.neversleep.app")
                .join("EBWebView")
                .join("Default");
            let _ = std::fs::remove_dir_all(base.join("Cache"));
            let _ = std::fs::remove_dir_all(base.join("Code Cache"));
        }
    }
}

fn main() {
    // Install global crash / panic logger to write details to NeverSleep-crash.log on Desktop
    std::panic::set_hook(Box::new(|panic_info| {
        let payload = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic payload".to_string()
        };
        let location = panic_info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());

        let timestamp = {
            #[cfg(target_os = "windows")]
            {
                local_timestamp().1
            }
            #[cfg(not(target_os = "windows"))]
            {
                utc_timestamp().1
            }
        };

        let message = format!(
            "[{}] CRITICAL APPLICATION PANIC at {}\nMessage: {}\n\n",
            timestamp, location, payload
        );

        eprintln!("{}", message);

        if let Some(desktop) = dirs_next::desktop_dir() {
            let crash_path = desktop.join("NeverSleep-crash.log");
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&crash_path)
            {
                use std::io::Write;
                let _ = writeln!(f, "{}", message);
            }
        }
    }));

    clear_webview_http_cache();

    // Single Instance Check using WinAPI Mutex
    #[cfg(target_os = "windows")]
    {
        std::env::set_var(
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
            "--disable-background-timer-throttling --disable-backgrounding-occluded-windows --disable-renderer-backgrounding",
        );

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
                    .unwrap_or_default()
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
                .unwrap_or_default()
                .as_millis()
                .try_into()
                .unwrap_or(0);

            let input_type = match event.event_type {
                EventType::MouseMove { .. }
                | EventType::Wheel { .. }
                | EventType::ButtonPress { .. }
                | EventType::ButtonRelease { .. } => "mouse",
                EventType::KeyPress(_) => "keyboard",
                // A release is the tail of a key action, not new activity. Ignoring it
                // prevents focus transitions from extending the darken timer spuriously.
                EventType::KeyRelease(_) => return,
            };

            let mut last = activity_clone.safe_lock();
            *last = (now, input_type.to_string());
        }) {
            eprintln!("Error listening to input events: {:?}", error);
        }
    });

    // System Tray Menu Construction
    let status_header =
        CustomMenuItem::new("status_header".to_string(), "⚡ Never Sleep: Active").disabled();
    let disable_app = CustomMenuItem::new("disable_app".to_string(), "Disable App");
    let keep_awake =
        CustomMenuItem::new("keep_awake".to_string(), "Keep Screen Awake Mode").selected();

    let darken_overlay =
        CustomMenuItem::new("darken_overlay".to_string(), "🪟 Overlay Window").selected();
    let darken_displaysleep = CustomMenuItem::new(
        "darken_displaysleep".to_string(),
        "🔌 Display Sleep (Standby)",
    );
    let darken_win32 = CustomMenuItem::new("darken_win32".to_string(), "🔲 Win32 Black Window");
    let darken_gamma = CustomMenuItem::new("darken_gamma".to_string(), "💡 Gamma Ramp Blackout");
    let darken_ddcci =
        CustomMenuItem::new("darken_ddcci".to_string(), "🖥️ Hardware DDC/CI (Native)");
    let darken_cmm = CustomMenuItem::new("darken_cmm".to_string(), "🛠️ ControlMyMonitor (NirSoft)");
    let darken_brightness =
        CustomMenuItem::new("darken_brightness".to_string(), "🔅 Screen Brightness");

    let darken_menu = SystemTrayMenu::new()
        .add_item(darken_overlay)
        .add_item(darken_displaysleep)
        .add_item(darken_win32)
        .add_item(darken_gamma)
        .add_item(darken_ddcci)
        .add_item(darken_cmm)
        .add_item(darken_brightness);

    let darken_submenu = SystemTraySubmenu::new("🌙 Darken Method", darken_menu);

    let timer_15m = CustomMenuItem::new("timer_15m".to_string(), "⏱️ 15 Minutes");
    let timer_30m = CustomMenuItem::new("timer_30m".to_string(), "⏱️ 30 Minutes");
    let timer_1h = CustomMenuItem::new("timer_1h".to_string(), "⏱️ 1 Hour");
    let timer_2h = CustomMenuItem::new("timer_2h".to_string(), "⏱️ 2 Hours");
    let timer_4h = CustomMenuItem::new("timer_4h".to_string(), "⏱️ 4 Hours");
    let timer_cancel =
        CustomMenuItem::new("timer_cancel".to_string(), "♾️ Always On (Cancel Timer)").selected();

    let duration_menu = SystemTrayMenu::new()
        .add_item(timer_15m)
        .add_item(timer_30m)
        .add_item(timer_1h)
        .add_item(timer_2h)
        .add_item(timer_4h)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(timer_cancel);

    let duration_submenu = SystemTraySubmenu::new("⏱️ Disable After Duration", duration_menu);

    let feat_media =
        CustomMenuItem::new("feat_media".to_string(), "Media Detection Auto-Keep-Awake");
    let feat_notifications =
        CustomMenuItem::new("feat_notifications".to_string(), "App Notifications");
    let feat_pinned = CustomMenuItem::new("feat_pinned".to_string(), "Always on Top (Pinned)");
    let feat_autostart =
        CustomMenuItem::new("feat_autostart".to_string(), "Run at Windows Startup");

    let features_menu = SystemTrayMenu::new()
        .add_item(feat_media)
        .add_item(feat_notifications)
        .add_item(feat_pinned)
        .add_item(feat_autostart);

    let features_submenu = SystemTraySubmenu::new("🛡️ Quick Features", features_menu);

    let show = CustomMenuItem::new("show".to_string(), "💻 Show Main Window");
    let settings = CustomMenuItem::new("settings".to_string(), "⚙️ Settings");
    let restart = CustomMenuItem::new("restart".to_string(), "🔄 Restart App");
    let update = CustomMenuItem::new("update".to_string(), "⬆️ Check for Updates");
    let quit = CustomMenuItem::new("quit".to_string(), "❌ Quit");

    let _tray_menu = SystemTrayMenu::new()
        .add_item(status_header)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(disable_app)
        .add_item(keep_awake)
        .add_submenu(darken_submenu)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_submenu(duration_submenu)
        .add_submenu(features_submenu)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(show)
        .add_item(settings)
        .add_item(restart)
        .add_item(update)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(quit);

    tauri::Builder::default()
        .manage(AppState {
            overlay_windows: Mutex::new(Vec::new()),
            last_activity,
            debug_log: Mutex::new(DebugLogState {
                enabled: false,
                desktop_dir: None,
                last_error: None,
            }),
            power_request: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            prevent_sleep,
            allow_sleep,
            get_ac_power_status,
            force_sleep,
            create_screen_overlay,
            close_screen_overlay,
            get_last_activity,
            record_input_activity,
            is_workstation_locked,
            lock_workstation,
            get_last_unlock_time,
            show_notification,
            log_to_file,
            get_debug_log_status,
            set_autostart,
            check_autostart,
            hide_window,
            focus_main_window,
            show_main_window,
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
            check_for_update,
            is_media_playing,
            set_app_icon,
            hard_apply_app_icon,
            update_tray_menu_state,
            get_screen_brightness,
            check_control_my_monitor,
            is_classic_only,
            toggle_app_disabled,
            resize_tray_window,
            start_cleaning_mode,
            stop_cleaning_mode
        ])
        .system_tray(SystemTray::new().with_tooltip("Never Sleep"))
        .on_system_tray_event(|app, event| match event {
            SystemTrayEvent::LeftClick { .. } => {
                // Left-click: focus the main window safely
                if let Some(window) = app.get_window("main") {
                    restore_main_window(&window, true);
                }
            }
            SystemTrayEvent::RightClick { position, size, .. } => {
                // Right-click: show custom glassmorphic tray flyout above the cursor
                if let Some(tray_win) = app.get_window("tray") {
                    if tray_win.is_visible().unwrap_or(false) {
                        let _ = tray_win.hide();
                        return;
                    }

                    // Flyout dimensions (must match tauri.conf.json)
                    let sf = if let Some(monitor) = tray_win.primary_monitor().ok().flatten() {
                        monitor.scale_factor()
                    } else {
                        1.0
                    };
                    let flyout_w: f64 = 320.0;
                    let flyout_h = tray_win
                        .outer_size()
                        .map(|size| size.height as f64 / sf)
                        .unwrap_or(380.0);

                    let (screen_w, screen_h) = if let Some(monitor) = tray_win.primary_monitor().ok().flatten() {
                        let s = monitor.size();
                        (s.width as f64 / sf, s.height as f64 / sf)
                    } else {
                        (1920.0, 1080.0)
                    };

                    // position and size from Tauri are in PHYSICAL pixels: convert to logical
                    let icon_cx = (position.x + size.width / 2.0) / sf;
                    let icon_top = position.y / sf;
                    let icon_bottom = (position.y + size.height) / sf;

                    let mut x = icon_cx - flyout_w / 2.0;
                    let mut y = icon_top - flyout_h - 10.0;

                    // If taskbar is at the top of the screen
                    if y < 8.0 {
                        y = icon_bottom + 10.0;
                    }

                    // Clamp to screen bounds
                    if x + flyout_w > screen_w - 8.0 { x = screen_w - flyout_w - 8.0; }
                    if x < 8.0 { x = 8.0; }
                    if y + flyout_h > screen_h - 8.0 { y = screen_h - flyout_h - 8.0; }
                    if y < 8.0 { y = 8.0; }

                    let _ = tray_win.set_position(tauri::LogicalPosition::new(x, y));
                    let _ = tray_win.show();
                    let _ = tray_win.set_focus();
                    // Record when the tray became visible (for the 600ms focus-loss grace period)
                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    TRAY_SHOWN_AT.store(now_ms, Ordering::SeqCst);
                    // Notify the tray window to sync its state
                    let _ = tray_win.emit("tray-opened", ());
                }
            }
            _ => {}
        })
        .on_window_event(|event| match event.event() {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                let window = event.window();
                if window.label().starts_with("cleaning_overlay_") {
                    let _ = stop_cleaning_mode_internal(&window.app_handle());
                } else if window.label() == "settings" || window.label() == "tray" {
                    // Hide instead of close to keep state
                    let _ = window.hide();
                    api.prevent_close();
                } else if window.label() == "main" {
                    // Quit app on main window close
                    if let Some(state) = window.app_handle().try_state::<AppState>() {
                        let app_handle = window.app_handle();
                        let _ = append_debug_log(&app_handle, &state, "Main window closed; application is shutting down");
                    }
                    std::process::exit(0);
                }
            }
            tauri::WindowEvent::Focused(false) => {
                let window = event.window();
                // Only auto-dismiss the tray flyout, not other windows
                if window.label() == "tray" {
                    let shown_at = TRAY_SHOWN_AT.load(Ordering::SeqCst);
                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    // 600ms grace period: ignore spurious blur right after opening
                    // (Windows Explorer needs time to settle focus on show)
                    if shown_at > 0 && now_ms.saturating_sub(shown_at) >= 600 {
                        let _ = window.hide();
                    }
                }
            }
            tauri::WindowEvent::Focused(true) => {
                let window = event.window();
                if window.label() == "main" && MAIN_WINDOW_PINNED.load(Ordering::SeqCst) {
                    // Windows can drop topmost state while restoring a minimized window.
                    let _ = window.set_always_on_top(true);
                }
            }
            tauri::WindowEvent::Resized(_) => {
                let window = event.window();
                if window.label() == "main" && !window.is_minimized().unwrap_or(false) {
                    let pinned = MAIN_WINDOW_PINNED.load(Ordering::SeqCst);
                    let _ = window.set_always_on_top(pinned);
                }
            }
            _ => {}
        })
        .setup(|app| {
            // Position the main window before showing it to avoid the default top-left placement.
            let args: Vec<String> = std::env::args().collect();
            let launch_minimized = args.iter().any(|arg| arg == "--minimized");

            // Restore the debug logging preference before frontend windows begin polling.
            let app_handle = app.handle();
            if let Some(state) = app.try_state::<AppState>() {
                if let Ok(settings_path) = get_settings_path(app_handle.clone()) {
                    if let Ok(contents) = std::fs::read_to_string(settings_path) {
                        if let Ok(settings) = serde_json::from_str::<serde_json::Value>(&contents) {
                            let pinned = settings
                                .get("pinned")
                                .and_then(|value| value.as_bool())
                                .unwrap_or(false);
                            MAIN_WINDOW_PINNED.store(pinned, Ordering::SeqCst);

                            let logging_enabled = settings
                                .get("debugLoggingEnabled")
                                .and_then(|value| value.as_bool())
                                .unwrap_or(false);
                            set_debug_logging(&app_handle, &state, logging_enabled);
                            if logging_enabled {
                                let _ = append_debug_log(&app_handle, &state, "Application started");
                            }
                        }
                    }
                }

                // Screen-darken mode also needs an active display power request so Windows
                // does not start its idle lock/screensaver countdown while the app is running.
                match set_windows_power_request(&state, true) {
                    Ok(()) => {
                        let _ = append_debug_log(
                            &app_handle,
                            &state,
                            "Windows idle lock and sleep prevention activated at startup",
                        );
                    }
                    Err(error) => {
                        let _ = append_debug_log(
                            &app_handle,
                            &state,
                            &format!("Windows idle lock prevention failed at startup: {error}"),
                        );
                    }
                }
            }

            if let Some(window) = app.get_window("main") {
                position_main_window_top_center(&window, None);
                let pinned = MAIN_WINDOW_PINNED.load(Ordering::SeqCst);
                let _ = window.set_always_on_top(pinned);
                if launch_minimized {
                    let _ = window.hide();
                } else {
                    let _ = window.show();
                }
            }

            // Restore saved app icon from settings
            {
                let app_handle = app.handle();
                std::thread::spawn(move || {
                    // Small delay to ensure windows are ready
                    std::thread::sleep(std::time::Duration::from_millis(800));
                    if let Ok(path) = get_settings_path(app_handle.clone()) {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                                let icon_file = json
                                    .get("appIcon")
                                    .and_then(|v| v.as_str())
                                    .filter(|filename| !filename.is_empty())
                                    .unwrap_or("A visually striking illustration.png");
                                // Reapply the saved icon to recreated Windows shortcuts after reinstall,
                                // as well as to the live window and system tray.
                                let _ = hard_apply_app_icon(app_handle, icon_file.to_string());
                            }
                        }
                    }
                });
            }

            // Background thread to continuously detect workstation lock and unlock
            // and notify frontend, immune to webview background timer throttling
            {
                let app_handle = app.handle();
                std::thread::spawn(move || {
                    let mut was_locked = false;
                    let mut last_detector_error: Option<String> = None;
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        let is_locked = match workstation_lock_state() {
                            Ok(is_locked) => {
                                last_detector_error = None;
                                is_locked
                            }
                            Err(error) => {
                                if last_detector_error.as_deref() != Some(error.as_str()) {
                                    if let Some(state) = app_handle.try_state::<AppState>() {
                                        let _ = append_debug_log(
                                            &app_handle,
                                            &state,
                                            &format!("Lock status query failed; no lock transition inferred: {error}"),
                                        );
                                    }
                                    last_detector_error = Some(error);
                                }
                                continue;
                            }
                        };
                        if is_locked && !was_locked {
                            was_locked = true;
                            let _ = app_handle.emit_all("workstation-locked", ());
                            if let Some(state) = app_handle.try_state::<AppState>() {
                                let _ = append_debug_log(&app_handle, &state, "Workstation lock detected");
                            }
                        } else if !is_locked && was_locked {
                            was_locked = false;
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_millis() as u64)
                                .unwrap_or(0);
                            LAST_UNLOCK_TIME.store(now, std::sync::atomic::Ordering::Relaxed);
                            let _ = app_handle.emit_all("workstation-unlocked", now);
                            if let Some(state) = app_handle.try_state::<AppState>() {
                                let _ = append_debug_log(&app_handle, &state, "Workstation unlock detected");
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
