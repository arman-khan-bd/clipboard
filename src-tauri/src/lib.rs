use arboard::Clipboard;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State, WindowEvent,
};

#[cfg(not(windows))]
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipItem {
    pub id: String,
    pub text: String,
    pub timestamp: u64,
    pub pinned: bool,
    #[serde(rename = "type")]
    pub item_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub hotkey: String,
    pub capacity: usize,
    #[serde(rename = "autoPaste")]
    pub auto_paste: bool,
    pub feedback: bool,
    #[serde(rename = "clearOnExit")]
    pub clear_on_exit: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            hotkey: "Win+V".to_string(),
            capacity: 100,
            auto_paste: true,
            feedback: true,
            clear_on_exit: false,
        }
    }
}

pub struct AppState {
    pub history: Mutex<Vec<ClipItem>>,
    pub settings: Mutex<AppSettings>,
    pub is_paused: AtomicBool,
    pub data_dir: PathBuf,
}

fn detect_type(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("ftp://")
        || trimmed.starts_with("www.")
    {
        return "link".to_string();
    }
    if trimmed.contains('{')
        || trimmed.contains('}')
        || trimmed.contains("=>")
        || trimmed.contains("fn ")
        || trimmed.contains("function")
        || trimmed.contains("const ")
        || trimmed.contains("let ")
        || trimmed.contains("def ")
        || trimmed.contains("class ")
        || trimmed.contains("import ")
        || trimmed.contains("#include")
        || trimmed.contains("<html")
        || trimmed.contains("\n  ")
        || trimmed.contains("\n\t")
    {
        return "code".to_string();
    }
    "text".to_string()
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn save_history_to_disk(data_dir: &PathBuf, history: &[ClipItem]) {
    let path = data_dir.join("history.json");
    if let Ok(json) = serde_json::to_string_pretty(history) {
        let _ = fs::write(path, json);
    }
}

fn load_history_from_disk(data_dir: &PathBuf) -> Vec<ClipItem> {
    let path = data_dir.join("history.json");
    if let Ok(content) = fs::read_to_string(path) {
        if let Ok(items) = serde_json::from_str::<Vec<ClipItem>>(&content) {
            return items;
        }
    }
    Vec::new()
}

fn save_settings_to_disk(data_dir: &PathBuf, settings: &AppSettings) {
    let path = data_dir.join("settings.json");
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = fs::write(path, json);
    }
}

fn load_settings_from_disk(data_dir: &PathBuf) -> AppSettings {
    let path = data_dir.join("settings.json");
    if let Ok(content) = fs::read_to_string(path) {
        if let Ok(s) = serde_json::from_str::<AppSettings>(&content) {
            return s;
        }
    }
    AppSettings::default()
}

// Windows Low-Level Keyboard Hook to guarantee Win+V works even if Explorer intercepts it
#[cfg(windows)]
pub mod win_hook {
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::mpsc::{channel, Sender};
    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_RCONTROL, VK_RMENU,
        VK_RSHIFT, VK_RWIN, VK_SHIFT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW,
        TranslateMessage, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN,
        WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    static WIN_DOWN: AtomicBool = AtomicBool::new(false);
    static CTRL_DOWN: AtomicBool = AtomicBool::new(false);
    static ALT_DOWN: AtomicBool = AtomicBool::new(false);
    static SHIFT_DOWN: AtomicBool = AtomicBool::new(false);

    // Modifier bitmask: 1 = Win, 2 = Ctrl, 4 = Alt, 8 = Shift
    static HOTKEY_MODS: AtomicU32 = AtomicU32::new(1); // Default Win
    static HOTKEY_KEY: AtomicU32 = AtomicU32::new(0x56); // Default 'V'

    static mut HOOK_TX: Option<Sender<()>> = None;

    pub fn set_hotkey(hotkey: &str) {
        let clean = hotkey.trim().to_lowercase();
        let mut mods = 0u32;
        if clean.contains("win") || clean.contains("super") || clean.contains("meta") {
            mods |= 1;
        }
        if clean.contains("ctrl") || clean.contains("control") || clean.contains("commandorcontrol") {
            mods |= 2;
        }
        if clean.contains("alt") {
            mods |= 4;
        }
        if clean.contains("shift") {
            mods |= 8;
        }

        // Find primary key
        let parts: Vec<&str> = clean.split('+').map(|s| s.trim()).collect();
        let last_key = parts
            .iter()
            .find(|&&p| {
                !matches!(
                    p,
                    "win" | "super" | "meta" | "ctrl" | "control" | "commandorcontrol" | "alt" | "shift"
                )
            })
            .copied()
            .unwrap_or("v");

        let vk = match last_key {
            "v" => 0x56,
            "c" => 0x43,
            "x" => 0x58,
            "z" => 0x5A,
            "b" => 0x42,
            "insert" => 0x2D,
            "space" => 0x20,
            s if s.len() == 1 => {
                let ch = s.chars().next().unwrap().to_ascii_uppercase();
                ch as u32
            }
            _ => 0x56,
        };

        HOTKEY_MODS.store(mods, Ordering::SeqCst);
        HOTKEY_KEY.store(vk, Ordering::SeqCst);
    }

    unsafe extern "system" fn hook_proc(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
        if code >= 0 {
            let kbd = unsafe { *(l_param as *const KBDLLHOOKSTRUCT) };
            let vk = kbd.vkCode as u16;
            let is_down = w_param as u32 == WM_KEYDOWN || w_param as u32 == WM_SYSKEYDOWN;
            let is_up = w_param as u32 == WM_KEYUP || w_param as u32 == WM_SYSKEYUP;

            match vk {
                VK_LWIN | VK_RWIN => {
                    if is_down { WIN_DOWN.store(true, Ordering::SeqCst); }
                    if is_up { WIN_DOWN.store(false, Ordering::SeqCst); }
                }
                VK_LCONTROL | VK_RCONTROL | VK_CONTROL => {
                    if is_down { CTRL_DOWN.store(true, Ordering::SeqCst); }
                    if is_up { CTRL_DOWN.store(false, Ordering::SeqCst); }
                }
                VK_LMENU | VK_RMENU | VK_MENU => {
                    if is_down { ALT_DOWN.store(true, Ordering::SeqCst); }
                    if is_up { ALT_DOWN.store(false, Ordering::SeqCst); }
                }
                VK_LSHIFT | VK_RSHIFT | VK_SHIFT => {
                    if is_down { SHIFT_DOWN.store(true, Ordering::SeqCst); }
                    if is_up { SHIFT_DOWN.store(false, Ordering::SeqCst); }
                }
                _ => {}
            }

            if is_down {
                use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
                    GetAsyncKeyState, keybd_event, KEYEVENTF_KEYUP,
                };

                let req_mods = HOTKEY_MODS.load(Ordering::SeqCst);
                let req_key = HOTKEY_KEY.load(Ordering::SeqCst) as u16;

                let is_key_async_down = |vk_code: u16| -> bool {
                    (unsafe { GetAsyncKeyState(vk_code as i32) } as u16 & 0x8000) != 0
                };

                let has_win = WIN_DOWN.load(Ordering::SeqCst)
                    || is_key_async_down(VK_LWIN)
                    || is_key_async_down(VK_RWIN);
                let has_ctrl = CTRL_DOWN.load(Ordering::SeqCst)
                    || is_key_async_down(VK_CONTROL)
                    || is_key_async_down(VK_LCONTROL)
                    || is_key_async_down(VK_RCONTROL);
                let has_alt = ALT_DOWN.load(Ordering::SeqCst)
                    || is_key_async_down(VK_MENU)
                    || is_key_async_down(VK_LMENU)
                    || is_key_async_down(VK_RMENU);
                let has_shift = SHIFT_DOWN.load(Ordering::SeqCst)
                    || is_key_async_down(VK_SHIFT)
                    || is_key_async_down(VK_LSHIFT)
                    || is_key_async_down(VK_RSHIFT);

                let win_match = (req_mods & 1 != 0) == has_win;
                let ctrl_match = (req_mods & 2 != 0) == has_ctrl;
                let alt_match = (req_mods & 4 != 0) == has_alt;
                let shift_match = (req_mods & 8 != 0) == has_shift;

                if win_match && ctrl_match && alt_match && shift_match && vk == req_key {
                    unsafe {
                        if let Some(ref tx) = HOOK_TX {
                            let _ = tx.send(());
                        }
                    }
                    // Suppress default Windows Clipboard if Win key is part of the shortcut
                    if has_win {
                        unsafe {
                            keybd_event(0xFF, 0, 0, 0);
                            keybd_event(0xFF, 0, KEYEVENTF_KEYUP, 0);
                        }
                        return 1;
                    }
                }
            }
        }
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param) }
    }

    pub fn start_hook(on_trigger: impl Fn() + Send + 'static) {
        let (tx, rx) = channel();
        unsafe {
            HOOK_TX = Some(tx);
        }

        std::thread::spawn(move || {
            while let Ok(()) = rx.recv() {
                on_trigger();
            }
        });

        std::thread::spawn(|| unsafe {
            let h_mod = windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null());
            let hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(hook_proc),
                h_mod,
                0,
            );
            if hook.is_null() {
                eprintln!("Failed to register low level keyboard hook");
                return;
            }

            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            UnhookWindowsHookEx(hook);
        });
    }
}

// Commands
#[tauri::command]
fn get_history(state: State<'_, AppState>) -> Result<Vec<ClipItem>, String> {
    let history = state.history.lock().map_err(|e| e.to_string())?;
    Ok(history.clone())
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    let settings = state.settings.lock().map_err(|e| e.to_string())?;
    Ok(settings.clone())
}

#[tauri::command]
fn copy_item(
    id: String,
    auto_paste: bool,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let mut history = state.history.lock().map_err(|e| e.to_string())?;
    let found = history.iter_mut().find(|item| item.id == id);
    if let Some(item) = found {
        item.timestamp = current_time_ms();
        let text_to_copy = item.text.clone();

        // Write to system clipboard
        if let Ok(mut cb) = Clipboard::new() {
            let _ = cb.set_text(&text_to_copy);
        }

        // Hide flyout immediately so focus returns to the target window
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }

        // Auto paste if requested (Windows)
        #[cfg(windows)]
        if auto_paste {
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(120));
                unsafe {
                    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
                        keybd_event, KEYEVENTF_KEYUP, VK_CONTROL,
                    };
                    keybd_event(VK_CONTROL as u8, 0, 0, 0);
                    keybd_event(b'V', 0, 0, 0);
                    keybd_event(b'V', 0, KEYEVENTF_KEYUP, 0);
                    keybd_event(VK_CONTROL as u8, 0, KEYEVENTF_KEYUP, 0);
                }
            });
        }
    }
    save_history_to_disk(&state.data_dir, &history);
    let _ = app.emit("clipboard-refresh", ());
    Ok(())
}

#[tauri::command]
fn delete_item(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut history = state.history.lock().map_err(|e| e.to_string())?;
    history.retain(|i| i.id != id);
    save_history_to_disk(&state.data_dir, &history);
    Ok(())
}

#[tauri::command]
fn toggle_pin(id: String, state: State<'_, AppState>) -> Result<bool, String> {
    let mut history = state.history.lock().map_err(|e| e.to_string())?;
    let mut is_pinned = false;
    if let Some(item) = history.iter_mut().find(|i| i.id == id) {
        item.pinned = !item.pinned;
        is_pinned = item.pinned;
    }
    save_history_to_disk(&state.data_dir, &history);
    Ok(is_pinned)
}

#[tauri::command]
fn clear_all_history(state: State<'_, AppState>) -> Result<(), String> {
    let mut history = state.history.lock().map_err(|e| e.to_string())?;
    history.retain(|i| i.pinned);
    save_history_to_disk(&state.data_dir, &history);
    Ok(())
}

#[tauri::command]
fn toggle_pause(state: State<'_, AppState>) -> Result<bool, String> {
    let prev = state.is_paused.fetch_xor(true, Ordering::SeqCst);
    Ok(!prev)
}

#[tauri::command]
fn save_settings(
    settings: AppSettings,
    state: State<'_, AppState>,
    #[allow(unused_variables)] app: AppHandle,
) -> Result<(), String> {
    let mut cur = state.settings.lock().map_err(|e| e.to_string())?;
    *cur = settings.clone();
    save_settings_to_disk(&state.data_dir, &settings);

    #[cfg(windows)]
    {
        win_hook::set_hotkey(&settings.hotkey);
    }

    #[cfg(not(windows))]
    {
        let _ = app.global_shortcut().unregister_all();
        if let Ok(shortcut) = settings.hotkey.parse::<Shortcut>() {
            let app_handle = app.clone();
            let _ = app.global_shortcut().on_shortcut(shortcut, move |_app, _sc, event| {
                if event.state == ShortcutState::Pressed {
                    show_clipboard_flyout(&app_handle);
                }
            });
        }
    }

    Ok(())
}

#[tauri::command]
fn hide_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    Ok(())
}

// Shows the clipboard window floating in bottom-right above taskbar (like Windows native clipboard)
fn show_clipboard_flyout(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
            return;
        }

        // Position window anchored to bottom-right corner
        if let Ok(Some(monitor)) = window.current_monitor() {
            let screen_size = monitor.size();
            let scale = monitor.scale_factor();
            let win_width = (360.0 * scale) as i32;
            let win_height = (520.0 * scale) as i32;
            let margin_x = (16.0 * scale) as i32;
            let margin_y = (60.0 * scale) as i32; // above taskbar

            let x = screen_size.width as i32 - win_width - margin_x;
            let y = screen_size.height as i32 - win_height - margin_y;

            let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(x, y)));
        }

        let _ = window.set_always_on_top(true);
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();

        #[cfg(windows)]
        {
            if let Ok(hwnd) = window.hwnd() {
                unsafe {
                    windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd.0 as _);
                    windows_sys::Win32::UI::WindowsAndMessaging::BringWindowToTop(hwnd.0 as _);
                }
            }
        }

        let _ = window.emit("window-opened", ());
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default();

    #[cfg(not(windows))]
    {
        builder = builder.plugin(tauri_plugin_global_shortcut::Builder::new().build());
    }

    builder
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("./data"));
            let _ = fs::create_dir_all(&data_dir);

            let history = load_history_from_disk(&data_dir);
            let settings = load_settings_from_disk(&data_dir);

            let app_state = AppState {
                history: Mutex::new(history),
                settings: Mutex::new(settings.clone()),
                is_paused: AtomicBool::new(false),
                data_dir: data_dir.clone(),
            };
            app.manage(app_state);

            // Auto-hide window on blur (just like Windows 11 Clipboard)
            if let Some(main_window) = app.get_webview_window("main") {
                let window_clone = main_window.clone();
                main_window.on_window_event(move |event| {
                    if let WindowEvent::Focused(false) = event {
                        let _ = window_clone.hide();
                    }
                });
            }

            // System Tray Setup
            let show_item = MenuItem::with_id(app, "show", "Show Clipboard (Win+V)", true, None::<&str>)?;
            let clear_item = MenuItem::with_id(app, "clear", "Clear History", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit ClipStore", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show_item, &clear_item, &quit_item])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("ClipStore - Clipboard History")
                .menu(&tray_menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        show_clipboard_flyout(app);
                    }
                    "clear" => {
                        if let Some(state) = app.try_state::<AppState>() {
                            if let Ok(mut history) = state.history.lock() {
                                history.retain(|i| i.pinned);
                                save_history_to_disk(&state.data_dir, &history);
                            }
                            let _ = app.emit("clipboard-refresh", ());
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_clipboard_flyout(tray.app_handle());
                    }
                })
                .build(app)?;

            let app_handle = app.handle().clone();

            // Set up Windows low-level hook for Win+V and other shortcuts
            #[cfg(windows)]
            {
                win_hook::set_hotkey(&settings.hotkey);
                let handle_clone = app_handle.clone();
                win_hook::start_hook(move || {
                    show_clipboard_flyout(&handle_clone);
                });
            }

            // Linux fallback shortcut handling
            #[cfg(not(windows))]
            {
                let handle_clone = app_handle.clone();
                if let Ok(sc) = settings.hotkey.parse::<Shortcut>() {
                    let _ = app.global_shortcut().on_shortcut(sc, move |_app, _sc, event| {
                        if event.state == ShortcutState::Pressed {
                            show_clipboard_flyout(&handle_clone);
                        }
                    });
                }
            }

            // Background clipboard poller thread
            let thread_app = app_handle.clone();
            std::thread::spawn(move || {
                let mut last_clipboard_text = String::new();
                let mut cb = match Clipboard::new() {
                    Ok(clipboard) => clipboard,
                    Err(e) => {
                        eprintln!("Failed to initialize clipboard listener: {}", e);
                        return;
                    }
                };

                if let Ok(initial_text) = cb.get_text() {
                    last_clipboard_text = initial_text;
                }

                loop {
                    std::thread::sleep(std::time::Duration::from_millis(350));

                    if let Some(state) = thread_app.try_state::<AppState>() {
                        if state.is_paused.load(Ordering::Relaxed) {
                            continue;
                        }

                        if let Ok(text) = cb.get_text() {
                            let trimmed = text.trim();
                            if !trimmed.is_empty() && text != last_clipboard_text {
                                last_clipboard_text = text.clone();

                                let item_type = detect_type(&text);
                                let now = current_time_ms();
                                let new_item = ClipItem {
                                    id: format!("{}_{}", now, fastrand_u32()),
                                    text: text.clone(),
                                    timestamp: now,
                                    pinned: false,
                                    item_type,
                                };

                                if let Ok(mut history) = state.history.lock() {
                                    let capacity = state
                                        .settings
                                        .lock()
                                        .map(|s| s.capacity)
                                        .unwrap_or(100);

                                    if let Some(pos) = history.iter().position(|i| i.text == text) {
                                        history[pos].timestamp = now;
                                    } else {
                                        history.insert(0, new_item.clone());
                                        if history.len() > capacity {
                                            if let Some(oldest_idx) = history
                                                .iter()
                                                .rposition(|i| !i.pinned)
                                            {
                                                history.remove(oldest_idx);
                                            }
                                        }
                                    }
                                    save_history_to_disk(&state.data_dir, &history);
                                }

                                let _ = thread_app.emit("clipboard-changed", new_item);
                            }
                        }
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_history,
            get_settings,
            copy_item,
            delete_item,
            toggle_pin,
            clear_all_history,
            toggle_pause,
            save_settings,
            hide_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ClipStore");
}

fn fastrand_u32() -> u32 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    nanos ^ 0x5bd1e995
}
