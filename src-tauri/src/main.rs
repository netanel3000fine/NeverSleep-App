#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use rdev::{listen, Event as RdevEvent, EventType};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{
    CustomMenuItem, Manager, SystemTray, SystemTrayEvent, SystemTrayMenu, SystemTrayMenuItem,
    SystemTraySubmenu,
};
use std::sync::atomic::{AtomicPtr, Ordering};

#[cfg(target_os = "windows")]
static MUTEX_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

// Global HWND storage for the Win32 darken window (isize is the raw HWND value)
#[cfg(target_os = "windows")]
static WIN32_DARKEN_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

// Global: previous brightness level before we dimmed it (for restore)
#[cfg(target_os = "windows")]
static PREV_BRIGHTNESS: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);


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

    let mut last = state.last_activity.lock().unwrap();
    *last = (now, normalized_type.to_string());
    Ok(())
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

#[cfg(target_os = "windows")]
fn create_laptop_overlay_window(
    app: &tauri::AppHandle,
    state: &tauri::State<'_, AppState>,
) -> Result<(), String> {
    let laptop_rects = ddcci_incompatible_monitor_rects()?;
    let main_window = app.get_window("main").ok_or("Main window not found")?;
    let monitors = main_window
        .available_monitors()
        .map_err(|e| format!("Failed to get monitors: {}", e))?;
    let mut overlay_windows = state.overlay_windows.lock().unwrap();

    for (i, monitor) in monitors.iter().enumerate() {
        let position = monitor.position();
        let size = monitor.size();
        let is_laptop_display = laptop_rects.iter().any(|(x, y, width, height)| {
            position.x == *x
                && position.y == *y
                && size.width == *width
                && size.height == *height
        });

        if !is_laptop_display {
            continue;
        }

        let label = format!("overlay_laptop_{}", i);
        let window = tauri::WindowBuilder::new(
            app,
            &label,
            tauri::WindowUrl::App("overlay.html".into()),
        )
        .title("Laptop Screen Overlay")
        .decorations(false)
        .always_on_top(true)
        .resizable(true)
        .fullscreen(true)
        .visible(false)
        .build()
        .map_err(|e| format!("Failed to create laptop overlay: {}", e))?;

        let _ = window.set_position(tauri::Position::Physical(position.clone()));
        let _ = window.set_size(tauri::Size::Physical(size.clone()));
        window
            .show()
            .map_err(|e| format!("Failed to show laptop overlay: {}", e))?;
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
    let m = method.unwrap_or_else(|| "overlay".to_string());
    match m.as_str() {
        "displaysleep" => create_darken_display_sleep(),
        "win32" => create_darken_win32(),
        "gamma" => create_darken_gamma(),
        "ddcci" => send_ddcci_vcp_power(4),
        "ddcci_laptop_overlay" => {
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
    let m = method.unwrap_or_else(|| "overlay".to_string());
    match m.as_str() {
        "displaysleep" => close_darken_display_sleep(),
        "win32" => close_darken_win32(),
        "gamma" => close_darken_gamma(),
        "ddcci" => send_ddcci_vcp_power(1),
        "ddcci_laptop_overlay" => {
            let close_result = {
                let mut overlay_windows = state.overlay_windows.lock().unwrap();
                for label in overlay_windows.iter() {
                    if let Some(window) = app.get_window(label) {
                        window.close().map_err(|e| e.to_string())?;
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
            let mut overlay_windows = state.overlay_windows.lock().unwrap();
            for label in overlay_windows.iter() {
                if let Some(window) = app.get_window(label) {
                    window.close().ok();
                }
            }
            overlay_windows.clear();
            Ok(())
        }
    }
}

// ===== DISPLAY SLEEP METHOD (Windows SC_MONITORPOWER) =====

fn create_darken_display_sleep() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use winapi::um::winuser::{SendMessageW, HWND_BROADCAST, WM_SYSCOMMAND, SC_MONITORPOWER};
        unsafe {
            // 2 = Monitor Power Off / Standby
            SendMessageW(HWND_BROADCAST, WM_SYSCOMMAND, SC_MONITORPOWER, 2);
        }
    }
    Ok(())
}

fn close_darken_display_sleep() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use winapi::um::winuser::{SendMessageW, HWND_BROADCAST, WM_SYSCOMMAND, SC_MONITORPOWER};
        unsafe {
            // -1 = Monitor Power On
            SendMessageW(HWND_BROADCAST, WM_SYSCOMMAND, SC_MONITORPOWER, -1);
        }
    }
    Ok(())
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
            use winapi::shared::windef::{HWND, HGDIOBJ, HBRUSH};
            use winapi::um::winuser::{
                CreateWindowExW, ShowWindow, UpdateWindow, DefWindowProcW,
                GetSystemMetrics, RegisterClassW, GetMessageW, TranslateMessage,
                DispatchMessageW, PostQuitMessage, DestroyWindow,
                SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
                SW_SHOW, WNDCLASSW, WS_EX_TOPMOST, WS_EX_TOOLWINDOW,
                WS_EX_TRANSPARENT, WS_POPUP, CS_HREDRAW, CS_VREDRAW, WM_CLOSE, WM_DESTROY,
                WM_ERASEBKGND,
            };
            use winapi::um::libloaderapi::GetModuleHandleW;
            use winapi::shared::minwindef::{HINSTANCE, LRESULT, WPARAM, LPARAM, UINT};

            #[link(name = "gdi32")]
            extern "system" {
                fn GetStockObject(i: i32) -> HGDIOBJ;
            }
            const BLACK_BRUSH: i32 = 4;

            unsafe extern "system" fn darken_wnd_proc(
                hwnd: HWND,
                msg: UINT,
                wparam: WPARAM,
                lparam: LPARAM,
            ) -> LRESULT {
                match msg {
                    WM_ERASEBKGND => 1,
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
                    WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT,
                    class_name.as_ptr(),
                    title.as_ptr(),
                    WS_POPUP,
                    x, y, w, h,
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

// ===== GAMMA RAMP METHOD =====

#[cfg(target_os = "windows")]
static SAVED_GAMMA_RAMP: Mutex<Option<Vec<u16>>> = Mutex::new(None);

fn create_darken_gamma() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use winapi::um::winuser::{GetDC, ReleaseDC};
        use winapi::shared::windef::HDC;

        #[link(name = "gdi32")]
        extern "system" {
            fn GetDeviceGammaRamp(hdc: HDC, lpRamp: *mut std::ffi::c_void) -> winapi::shared::minwindef::BOOL;
            fn SetDeviceGammaRamp(hdc: HDC, lpRamp: *const std::ffi::c_void) -> winapi::shared::minwindef::BOOL;
        }

        unsafe {
            let hdc = GetDC(std::ptr::null_mut());
            if hdc.is_null() {
                return Err("Failed to get primary display DC".to_string());
            }

            let mut original_ramp = vec![0u16; 3 * 256];
            if GetDeviceGammaRamp(hdc, original_ramp.as_mut_ptr() as *mut std::ffi::c_void) != 0 {
                let mut saved = SAVED_GAMMA_RAMP.lock().unwrap();
                *saved = Some(original_ramp);

                let black_ramp = vec![0u16; 3 * 256];
                SetDeviceGammaRamp(hdc, black_ramp.as_ptr() as *const std::ffi::c_void);
            } else {
                ReleaseDC(std::ptr::null_mut(), hdc);
                return Err("Display driver does not support hardware gamma ramp".to_string());
            }
            ReleaseDC(std::ptr::null_mut(), hdc);
        }
    }
    Ok(())
}

fn close_darken_gamma() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use winapi::um::winuser::{GetDC, ReleaseDC};
        use winapi::shared::windef::HDC;

        #[link(name = "gdi32")]
        extern "system" {
            fn SetDeviceGammaRamp(hdc: HDC, lpRamp: *const std::ffi::c_void) -> winapi::shared::minwindef::BOOL;
        }

        unsafe {
            let mut saved = SAVED_GAMMA_RAMP.lock().unwrap();
            if let Some(ramp) = saved.take() {
                let hdc = GetDC(std::ptr::null_mut());
                if !hdc.is_null() {
                    SetDeviceGammaRamp(hdc, ramp.as_ptr() as *const std::ffi::c_void);
                    ReleaseDC(std::ptr::null_mut(), hdc);
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
struct PHYSICAL_MONITOR {
    hPhysicalMonitor: *mut std::ffi::c_void,
    szPhysicalMonitorDescription: [u16; 128],
}

#[cfg(target_os = "windows")]
fn ddcci_incompatible_monitor_rects() -> Result<Vec<(i32, i32, u32, u32)>, String> {
    use winapi::shared::minwindef::{BOOL, LPARAM, TRUE};
    use winapi::shared::windef::{HDC, HMONITOR, LPRECT, RECT};
    use winapi::um::winuser::{
        EnumDisplayMonitors, GetMonitorInfoW, MONITORINFO,
    };

    struct MonitorContext {
        rects: Vec<(i32, i32, u32, u32)>,
    }

    unsafe extern "system" fn monitor_enum_proc(
        h_monitor: HMONITOR,
        _: HDC,
        _: LPRECT,
        lparam: LPARAM,
    ) -> BOOL {
        let context = &mut *(lparam as *mut MonitorContext);
        let mut physical_count = 0u32;

        if get_physical_monitor_count(h_monitor, &mut physical_count) && physical_count == 0 {
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                rcMonitor: RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                },
                rcWork: RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                },
                dwFlags: 0,
            };
            if GetMonitorInfoW(h_monitor, &mut info) != 0 {
                context.rects.push((
                    info.rcMonitor.left,
                    info.rcMonitor.top,
                    (info.rcMonitor.right - info.rcMonitor.left) as u32,
                    (info.rcMonitor.bottom - info.rcMonitor.top) as u32,
                ));
            }
        }

        TRUE
    }

    unsafe {
        let mut context = MonitorContext { rects: Vec::new() };
        if EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            Some(monitor_enum_proc),
            &mut context as *mut _ as LPARAM,
        ) == 0
        {
            return Err("Windows could not enumerate displays".to_string());
        }
        Ok(context.rects)
    }
}

#[cfg(target_os = "windows")]
unsafe fn get_physical_monitor_count(
    h_monitor: winapi::shared::windef::HMONITOR,
    count: *mut u32,
) -> bool {
    use winapi::um::libloaderapi::{FreeLibrary, GetProcAddress, LoadLibraryA};
    use winapi::shared::windef::HMONITOR;
    let dxva2 = LoadLibraryA("dxva2.dll\0".as_ptr() as *const i8);
    if dxva2.is_null() {
        return false;
    }

    type GetCount = unsafe extern "system" fn(HMONITOR, *mut u32) -> i32;
    let proc = GetProcAddress(
        dxva2,
        "GetNumberOfPhysicalMonitorsFromHMONITOR\0".as_ptr() as *const i8,
    );
    if proc.is_null() {
        FreeLibrary(dxva2);
        return false;
    }

    let get_count: GetCount = std::mem::transmute(proc);
    let result = get_count(h_monitor, count) != 0;
    FreeLibrary(dxva2);
    result
}

#[cfg(target_os = "windows")]
#[allow(non_snake_case)]
fn send_ddcci_vcp_power(power_value: u32) -> Result<(), String> {
    use winapi::um::libloaderapi::{LoadLibraryA, GetProcAddress, FreeLibrary};
    use winapi::um::winuser::EnumDisplayMonitors;
    use winapi::shared::windef::{HMONITOR, HDC, LPRECT};
    use winapi::shared::minwindef::{BOOL, LPARAM, TRUE};

    type FnGetNumberOfPhysicalMonitors = unsafe extern "system" fn(hMonitor: HMONITOR, pdw: *mut u32) -> BOOL;
    type FnGetPhysicalMonitors = unsafe extern "system" fn(hMonitor: HMONITOR, count: u32, pArray: *mut PHYSICAL_MONITOR) -> BOOL;
    type FnSetVCPFeature = unsafe extern "system" fn(hMonitor: *mut std::ffi::c_void, vcp_code: u8, new_val: u32) -> BOOL;
    type FnGetVCPFeature = unsafe extern "system" fn(
        hMonitor: *mut std::ffi::c_void,
        vcp_code: u8,
        vcp_type: *mut u8,
        current_value: *mut u32,
        maximum_value: *mut u32,
    ) -> BOOL;
    type FnDestroyPhysicalMonitors = unsafe extern "system" fn(count: u32, pArray: *mut PHYSICAL_MONITOR) -> BOOL;

    unsafe {
        let dxva2 = LoadLibraryA("dxva2.dll\0".as_ptr() as *const i8);
        if dxva2.is_null() {
            return Err("dxva2.dll not available on this system".to_string());
        }

        let p_get_count_raw = GetProcAddress(dxva2, "GetNumberOfPhysicalMonitorsFromHMONITOR\0".as_ptr() as *const i8);
        let p_get_mons_raw = GetProcAddress(dxva2, "GetPhysicalMonitorsFromHMONITOR\0".as_ptr() as *const i8);
        let p_set_vcp_raw = GetProcAddress(dxva2, "SetVCPFeature\0".as_ptr() as *const i8);
        let p_get_vcp_raw = GetProcAddress(dxva2, "GetVCPFeatureAndVCPFeatureReply\0".as_ptr() as *const i8);
        let p_destroy_raw = GetProcAddress(dxva2, "DestroyPhysicalMonitors\0".as_ptr() as *const i8);

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
                let mut physical_mons = Vec::<PHYSICAL_MONITOR>::with_capacity(count as usize);
                physical_mons.set_len(count as usize);
                if (ctx.p_get_mons)(h_monitor, count, physical_mons.as_mut_ptr()) != 0 {
                    for mon in &physical_mons {
                        ctx.physical_monitor_count += 1;
                        // VCP 0xD6 is standard VESA Power Mode: 4 = Standby/Off, 1 = On
                        if (ctx.p_set_vcp)(mon.hPhysicalMonitor, 0xD6, ctx.power_value) != 0 {
                            ctx.success_count += 1;
                        }
                        if ctx.power_value == 1 {
                            let mut vcp_type = 0u8;
                            let mut current_value = 0u32;
                            let mut maximum_value = 0u32;
                            if (ctx.p_get_vcp)(
                                mon.hPhysicalMonitor,
                                0xD6,
                                &mut vcp_type,
                                &mut current_value,
                                &mut maximum_value,
                            ) != 0
                                && current_value == 1
                            {
                                ctx.verified_on_count += 1;
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
                && (power_value != 1
                    || ctx.verified_on_count == ctx.physical_monitor_count);

            if all_monitors_succeeded
            {
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
        let p = std::path::PathBuf::from(local_app_data).join("NirSoft").join("ControlMyMonitor.exe");
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
async fn check_for_update(current_version: String) -> Result<serde_json::Value, String> {
    // Scan ALL releases to find the highest version by asset filename
    let api_url = "https://api.github.com/repos/netanel3000fine/NeverSleep-App/releases";

    let json_text = std::thread::spawn(move || {
        let client = reqwest::blocking::Client::builder()
            .user_agent("NeverSleep-Updater/1.0")
            .build()
            .map_err(|e| e.to_string())?;
        let resp = client.get(api_url)
            .header("Accept", "application/vnd.github+json")
            .send()
            .map_err(|e| e.to_string())?;
        resp.text().map_err(|e| e.to_string())
    })
    .join()
    .map_err(|_| "Thread panicked".to_string())??;

    let releases: serde_json::Value = serde_json::from_str(&json_text)
        .map_err(|e| format!("JSON parse error: {}", e))?;

    fn parse_ver(s: &str) -> (u32, u32, u32) {
        let p: Vec<u32> = s.trim_start_matches('v').split('.').filter_map(|x| x.parse().ok()).collect();
        if p.len() == 3 { (p[0], p[1], p[2]) } else { (0, 0, 0) }
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
                        let parts: Vec<&str> = name.split(|c: char| !c.is_alphanumeric() && c != '.').filter(|s| !s.is_empty()).collect();
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

#[tauri::command]
fn update_tray_menu_state(
    app: tauri::AppHandle,
    status_text: String,
    disabled: bool,
    keep_awake: bool,
    schedule_mode_active: bool,
    active_duration_mins: Option<u32>,
    media_enabled: bool,
    notifications_enabled: bool,
    pinned: bool,
    autostart: bool,
) -> Result<(), String> {
    let handle = app.tray_handle();

    let _ = handle.get_item("status_header").set_title(&status_text);
    let _ = handle.get_item("disable_app").set_selected(disabled);
    let _ = handle.get_item("keep_awake").set_selected(keep_awake && !disabled);

    let duration_enabled = !schedule_mode_active && !disabled;
    let timer_ids = [
        ("timer_15m", 15),
        ("timer_30m", 30),
        ("timer_1h", 60),
        ("timer_2h", 120),
        ("timer_4h", 240),
    ];

    for (id, mins) in timer_ids {
        let item = handle.get_item(id);
        let _ = item.set_enabled(duration_enabled);
        let is_selected = active_duration_mins == Some(mins);
        let _ = item.set_selected(is_selected);
    }
    let cancel_item = handle.get_item("timer_cancel");
    let _ = cancel_item.set_enabled(duration_enabled);
    let _ = cancel_item.set_selected(active_duration_mins.is_none());

    let _ = handle.get_item("feat_media").set_selected(media_enabled);
    let _ = handle.get_item("feat_notifications").set_selected(notifications_enabled);
    let _ = handle.get_item("feat_pinned").set_selected(pinned);
    let _ = handle.get_item("feat_autostart").set_selected(autostart);

    Ok(())
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

    // System Tray Menu Construction
    let status_header = CustomMenuItem::new("status_header".to_string(), "⚡ Never Sleep: Active").disabled();
    let disable_app = CustomMenuItem::new("disable_app".to_string(), "Disable App");
    let keep_awake = CustomMenuItem::new("keep_awake".to_string(), "Keep Screen Awake Mode").selected();

    let timer_15m = CustomMenuItem::new("timer_15m".to_string(), "⏱️ 15 Minutes");
    let timer_30m = CustomMenuItem::new("timer_30m".to_string(), "⏱️ 30 Minutes");
    let timer_1h = CustomMenuItem::new("timer_1h".to_string(), "⏱️ 1 Hour");
    let timer_2h = CustomMenuItem::new("timer_2h".to_string(), "⏱️ 2 Hours");
    let timer_4h = CustomMenuItem::new("timer_4h".to_string(), "⏱️ 4 Hours");
    let timer_cancel = CustomMenuItem::new("timer_cancel".to_string(), "♾️ Always On (Cancel Timer)").selected();

    let duration_menu = SystemTrayMenu::new()
        .add_item(timer_15m)
        .add_item(timer_30m)
        .add_item(timer_1h)
        .add_item(timer_2h)
        .add_item(timer_4h)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(timer_cancel);

    let duration_submenu = SystemTraySubmenu::new("⏱️ Disable After Duration", duration_menu);

    let feat_media = CustomMenuItem::new("feat_media".to_string(), "Media Detection Auto-Keep-Awake");
    let feat_notifications = CustomMenuItem::new("feat_notifications".to_string(), "App Notifications");
    let feat_pinned = CustomMenuItem::new("feat_pinned".to_string(), "Always on Top (Pinned)");
    let feat_autostart = CustomMenuItem::new("feat_autostart".to_string(), "Run at Windows Startup");

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

    let tray_menu = SystemTrayMenu::new()
        .add_item(status_header)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(disable_app)
        .add_item(keep_awake)
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
        })
        .invoke_handler(tauri::generate_handler![
            prevent_sleep,
            allow_sleep,
            force_sleep,
            create_screen_overlay,
            close_screen_overlay,
            get_last_activity,
            record_input_activity,
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
            check_for_update,
            is_media_playing,
            set_app_icon,
            hard_apply_app_icon,
            update_tray_menu_state,
            get_screen_brightness,
            check_control_my_monitor
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
                    if window.is_minimized().unwrap_or(false) {
                        window.unminimize().unwrap();
                    }
                    window.show().unwrap();
                    window.set_focus().unwrap();
                }
                "settings" => {
                    open_settings(app.clone(), None);
                }
                "restart" => {
                    restart_app(app.clone());
                }
                "update" => {
                    if let Some(window) = app.get_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                        let _ = window.emit("check-for-updates", ());
                    }
                }
                "quit" => {
                    std::process::exit(0);
                }
                "feat_autostart" => {
                    if let Ok(res) = check_autostart() {
                        let current = res.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
                        let new_state = !current;
                        let _ = set_autostart(new_state, false);
                        let _ = app.tray_handle().get_item("feat_autostart").set_selected(new_state);
                        let _ = app.emit_all("tray-action", "autostart-toggled");
                    }
                }
                action => {
                    let _ = app.emit_all("tray-action", action);
                }
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
            // Hide main window on startup if launched with --minimized flag
            let args: Vec<String> = std::env::args().collect();
            if args.iter().any(|a| a == "--minimized") {
                if let Some(window) = app.get_window("main") {
                    let _ = window.hide();
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
