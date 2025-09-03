use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Listener, Manager, WebviewUrl, WebviewWindowBuilder,
};

mod commands;
mod state;
mod services;
mod controllers;

use state::AppState;
use services::global_shortcuts::{GlobalShortcutService, ShortcutAction};
use services::conference_window::{ConferenceWindow, ConferenceConfig};
use controllers::call_controller::CallController;
use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Initialize settings store
            let settings_store = match blink::storage::SettingsStore::load() {
                Ok(store) => store,
                Err(e) => {
                    log::error!("Failed to load settings: {}", e);
                    log::info!("Using default settings");
                    blink::storage::SettingsStore::new_with_path(
                        dirs::config_dir()
                            .unwrap_or_else(|| std::path::PathBuf::from("."))
                            .join("blink")
                            .join("settings.json")
                    )
                }
            };
            
            // Create global shortcut service
            let mut shortcuts_service = GlobalShortcutService::new(app.handle().clone());
            
            // Setup default hotkeys from settings
            {
                let store = &settings_store;
                let settings = store.settings();
                if let Err(e) = shortcuts_service.setup_default_hotkeys(&settings.keybinds) {
                    log::error!("Failed to setup default hotkeys: {}", e);
                }
            }
            
            // Create conference window manager
            let conference_window = ConferenceWindow::new(app.handle().clone());
            
            // Create call controller
            let call_controller = CallController::new(app.handle().clone());
            
            // Set up app state
            app.manage(AppState {
                settings_store: Mutex::new(settings_store),
                shortcuts: Mutex::new(shortcuts_service),
                conference_window: Mutex::new(conference_window),
                call_controller: Mutex::new(call_controller),
            });
            // Create menu items
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let settings_i = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            
            // Create menu
            let menu = Menu::with_items(app, &[&settings_i, &quit_i])?;
            
            // Create tray icon with menu
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Blink - Right-click for menu")
                .menu(&menu)
                .show_menu_on_left_click(false) // Right-click only for menu
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        log::info!("User selected quit from tray menu");
                        app.exit(0);
                    }
                    "settings" => {
                        log::info!("User selected settings from tray menu");
                        // Show settings window or create it
                        match app.get_webview_window("settings") {
                            Some(window) => {
                                // Window exists, show and focus it
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                            None => {
                                // Create settings window
                                let _settings_window = WebviewWindowBuilder::new(
                                    app,
                                    "settings",
                                    WebviewUrl::App("settings.html".into())
                                )
                                .title("Blink Settings")
                                .inner_size(700.0, 600.0)
                                .resizable(true)
                                .build()
                                .expect("failed to build settings window");
                            }
                        }
                    }
                    _ => {
                        log::warn!("Unknown menu item: {:?}", event.id);
                    }
                })
                .on_tray_icon_event(|_tray, event| {
                    // Only log significant events, not every mouse movement
                    match event {
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } => {
                            log::debug!("Tray icon left-clicked");
                        }
                        TrayIconEvent::DoubleClick { .. } => {
                            log::debug!("Tray icon double-clicked");
                        }
                        // Ignore Move, Enter, Leave events to reduce noise
                        TrayIconEvent::Move { .. } |
                        TrayIconEvent::Enter { .. } |
                        TrayIconEvent::Leave { .. } => {}
                        _ => {
                            log::debug!("Tray event: {:?}", event);
                        }
                    }
                })
                .build(app)?;
            
            // Hide main window on startup
            if let Some(window) = app.get_webview_window("main") {
                window.hide()?;
            }
            
            // Check if this is first run and show onboarding if needed
            let app_state = app.state::<AppState>();
            let is_first_run = {
                let settings_store = app_state.settings_store.lock().unwrap();
                settings_store.get_targets().is_empty()
            };
            
            if is_first_run {
                log::info!("First run detected - showing setup wizard");
                
                // Open setup wizard for first-time users
                let _wizard_window = WebviewWindowBuilder::new(
                    app,
                    "setup-wizard",
                    WebviewUrl::App("setup-wizard.html".into())
                )
                .title("Welcome to Blink - Setup Wizard")
                .inner_size(700.0, 800.0)
                .center()
                .resizable(false)
                .build()
                .expect("failed to build setup wizard window");
            }
            
            // Listen for hotkey events
            let app_handle = app.handle().clone();
            app.listen("hotkey-pressed", move |event| {
                log::info!("Hotkey event received: {:?}", event.payload());
                
                // Parse the action
                if let Ok(action) = serde_json::from_str::<ShortcutAction>(event.payload()) {
                    let state = app_handle.state::<AppState>();
                    
                    match action {
                        ShortcutAction::JoinPrimary => {
                            log::info!("Join primary target requested");
                            
                            // Get primary target from settings
                            let settings_store = state.settings_store.lock().unwrap();
                            if let Some(target) = settings_store.get_primary_target() {
                                log::info!("Primary target found: {} with code: {}", target.label, target.code);
                                let room_id = blink::core::room_id_from_code(&target.code);
                                log::info!("Generated room ID from code '{}': '{}'", target.code, room_id);
                                let config = ConferenceConfig {
                                    room_id: room_id.clone(),
                                    display_name: "You".to_string(),
                                    start_with_audio_muted: !target.call_defaults.start_with_audio,
                                    start_with_video_muted: !target.call_defaults.start_with_video,
                                    always_on_top: settings_store.settings().app_settings.always_on_top,
                                };
                                let target_id = target.id.clone();
                                drop(settings_store);
                                
                                // Open directly in browser instead of using conference window
                                use services::external_browser::ExternalBrowserService;
                                if let Err(e) = ExternalBrowserService::open_meeting(&app_handle, &room_id) {
                                    log::error!("Failed to open meeting in browser: {}", e);
                                    // TODO: Show toast notification
                                }
                            } else {
                                log::warn!("No primary target configured");
                            }
                        }
                        ShortcutAction::JoinTarget { id } => {
                            log::info!("Join target {} requested", id);
                            
                            // Get target from settings
                            let settings_store = state.settings_store.lock().unwrap();
                            if let Some(target) = settings_store.get_target(&id) {
                                let room_id = blink::core::room_id_from_code(&target.code);
                                drop(settings_store);
                                
                                // Open directly in browser instead of using conference window
                                use services::external_browser::ExternalBrowserService;
                                if let Err(e) = ExternalBrowserService::open_meeting(&app_handle, &room_id) {
                                    log::error!("Failed to open meeting in browser: {}", e);
                                    // TODO: Show toast notification
                                }
                            } else {
                                log::warn!("Target {} not found", id);
                            }
                        }
                        ShortcutAction::Hangup => {
                            log::info!("Hangup requested - not applicable when using external browser");
                            // When using external browser, users must close the browser tab/window manually
                        }
                    }
                }
            });
            
            // Listen for conference events
            let app_handle_clone = app.handle().clone();
            app.listen("videoConferenceJoined", move |_| {
                log::info!("Conference joined event received");
                let state = app_handle_clone.state::<AppState>();
                let controller = state.call_controller.lock().unwrap();
                controller.on_conference_joined();
            });
            
            let app_handle_clone2 = app.handle().clone();
            app.listen("videoConferenceLeft", move |_| {
                log::info!("Conference left event received");
                let state = app_handle_clone2.state::<AppState>();
                let controller = state.call_controller.lock().unwrap();
                controller.on_conference_left();
            });
            
            log::info!("Blink initialized successfully");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::generate_code,
            commands::validate_hotkey,
            commands::test_hotkey,
            commands::remove_target,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}