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
            hotkey: "Super+V".to_string(),
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

// Commands
#[tauri::command]
fn get_history(state: State<'_, AppState>) -> Result<Vec<ClipItem>, String> {
    let history = state.history.lock().map_err(|e| e.to_string())?;
    Ok(history.clone())
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

        // Auto paste if requested (Windows)
        #[cfg(windows)]
        if auto_paste {
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(100));
                unsafe {
                    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
                        keybd_event, KEYEVENTF_KEYUP, VK_CONTROL,
                    };
                    // Ctrl down
                    keybd_event(VK_CONTROL as u8, 0, 0, 0);
                    // 'V' down
                    keybd_event(b'V', 0, 0, 0);
                    // 'V' up
                    keybd_event(b'V', 0, KEYEVENTF_KEYUP, 0);
                    // Ctrl up
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
    app: AppHandle,
) -> Result<(), String> {
    let mut cur = state.settings.lock().map_err(|e| e.to_string())?;
    let old_hotkey = cur.hotkey.clone();
    *cur = settings.clone();
    save_settings_to_disk(&state.data_dir, &settings);

    // Re-register hotkey if changed
    if old_hotkey != settings.hotkey {
        let _ = app.global_shortcut().unregister_all();
        if let Ok(shortcut) = settings.hotkey.parse::<Shortcut>() {
            let app_handle = app.clone();
            let _ = app.global_shortcut().on_shortcut(shortcut, move |_app, _sc, event| {
                if event.state == ShortcutState::Pressed {
                    toggle_window_visibility(&app_handle);
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

fn toggle_window_visibility(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Get or create app data directory
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("./data"));
            let _ = fs::create_dir_all(&data_dir);

            // Load persisted data
            let history = load_history_from_disk(&data_dir);
            let settings = load_settings_from_disk(&data_dir);

            let app_state = AppState {
                history: Mutex::new(history),
                settings: Mutex::new(settings.clone()),
                is_paused: AtomicBool::new(false),
                data_dir: data_dir.clone(),
            };
            app.manage(app_state);

            // Auto-hide window on blur (like native Windows Clipboard)
            if let Some(main_window) = app.get_webview_window("main") {
                let window_clone = main_window.clone();
                main_window.on_window_event(move |event| {
                    if let WindowEvent::Focused(false) = event {
                        let _ = window_clone.hide();
                    }
                });
            }

            // Setup System Tray
            let show_item = MenuItem::with_id(app, "show", "Show Clipboard (Win+V)", true, None::<&str>)?;
            let clear_item = MenuItem::with_id(app, "clear", "Clear Unpinned History", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit ClipStore", true, None::<&str>)?;

            let tray_menu = Menu::with_items(app, &[&show_item, &clear_item, &quit_item])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("ClipStore - Clipboard Manager (Win+V)")
                .menu(&tray_menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        toggle_window_visibility(app);
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
                        toggle_window_visibility(tray.app_handle());
                    }
                })
                .build(app)?;

            // Register global shortcut
            let app_handle = app.handle().clone();
            let hotkey_str = settings.hotkey.clone();

            // Try configured shortcut (e.g. Super+V)
            let registered = if let Ok(shortcut) = hotkey_str.parse::<Shortcut>() {
                let handle_clone = app_handle.clone();
                app.global_shortcut().on_shortcut(shortcut, move |_app, _sc, event| {
                    if event.state == ShortcutState::Pressed {
                        toggle_window_visibility(&handle_clone);
                    }
                }).is_ok()
            } else {
                false
            };

            // If Super+V fails (e.g. Windows reserved hotkey), also register fallback universal Ctrl+Shift+V
            if !registered || hotkey_str != "CommandOrControl+Shift+V" {
                if let Ok(fallback) = "CommandOrControl+Shift+V".parse::<Shortcut>() {
                    let handle_clone = app_handle.clone();
                    let _ = app.global_shortcut().on_shortcut(fallback, move |_app, _sc, event| {
                        if event.state == ShortcutState::Pressed {
                            toggle_window_visibility(&handle_clone);
                        }
                    });
                }
            }

            // Spawn background clipboard polling thread
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

                // Seed last clipboard text
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

                                    // Deduplicate: if exists, update timestamp and move to top
                                    if let Some(pos) = history.iter().position(|i| i.text == text) {
                                        history[pos].timestamp = now;
                                    } else {
                                        history.insert(0, new_item.clone());
                                        if history.len() > capacity {
                                            // Evict oldest unpinned
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
