mod commands;
mod findr_client;
mod license;

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[cfg(target_os = "macos")]
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt,
};

#[cfg(target_os = "macos")]
tauri_panel! {
    panel!(FindrPanel {
        config: {
            can_become_key_window: true,
            is_floating_panel: true
        }
    })

    panel_event!(FindrPanelEventHandler {
        window_did_resign_key(notification: &NSNotification) -> ()
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_process::init());

    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_nspanel::init());
    }

    {
        let mods = if cfg!(target_os = "macos") {
            Modifiers::META | Modifiers::SHIFT
        } else {
            Modifiers::CONTROL | Modifiers::SHIFT
        };
        let toggle_shortcut = Shortcut::new(Some(mods), Code::KeyF);

        builder = builder.plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if shortcut == &toggle_shortcut && event.state() == ShortcutState::Pressed {
                        toggle_overlay(app);
                    }
                })
                .build(),
        );
        builder = builder.plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ));
    }

    builder
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            #[cfg(target_os = "macos")]
            setup_macos_panel(app);

            #[cfg(not(target_os = "macos"))]
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_visible_on_all_workspaces(true);
                let win_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let _ = win_clone.hide();
                    }
                });
            }

            {
                let mods = if cfg!(target_os = "macos") {
                    Modifiers::META | Modifiers::SHIFT
                } else {
                    Modifiers::CONTROL | Modifiers::SHIFT
                };
                app.global_shortcut()
                    .register(Shortcut::new(Some(mods), Code::KeyF))?;
            }

            let show_item = MenuItem::with_id(app, "show", "Show findr", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[
                    &show_item,
                    &PredefinedMenuItem::separator(app)?,
                    &quit_item,
                ],
            )?;

            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .icon_as_template(true)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_overlay(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_overlay(tray.app_handle());
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::search,
            commands::get_recent_files,
            commands::track_interaction,
            commands::get_index_status,
            commands::get_findr_version,
            commands::hide_overlay,
            commands::get_license_state,
            commands::activate_license,
            commands::start_trial,
            commands::get_trial_days_remaining,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ── macOS: NSPanel overlay (works over fullscreen apps) ──

#[cfg(target_os = "macos")]
fn setup_macos_panel(app: &tauri::App) {
    let window = app.get_webview_window("main").unwrap();
    let panel = window.to_panel::<FindrPanel>().unwrap();

    panel.set_level(PanelLevel::Floating.value());
    panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .full_screen_auxiliary()
            .can_join_all_spaces()
            .into(),
    );
    panel.set_hides_on_deactivate(false);

    // Hide panel when it loses key status (click outside)
    let handler = FindrPanelEventHandler::new();
    let app_handle = app.handle().clone();
    handler.window_did_resign_key(move |_notification| {
        if let Ok(p) = app_handle.get_webview_panel("main") {
            p.hide();
        }
    });
    panel.set_event_handler(Some(handler.as_ref()));
}

#[cfg(target_os = "macos")]
fn show_overlay<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Ok(panel) = app.get_webview_panel("main") {
        panel.show();
        panel.make_key_window();
    }
}

#[cfg(target_os = "macos")]
fn toggle_overlay<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Ok(panel) = app.get_webview_panel("main") {
        if panel.is_visible() {
            panel.hide();
        } else {
            panel.show();
        panel.make_key_window();
        }
    }
}

// ── Non-macOS fallback ──

#[cfg(not(target_os = "macos"))]
fn show_overlay<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.center();
    }
}

#[cfg(not(target_os = "macos"))]
fn toggle_overlay<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        match window.is_visible() {
            Ok(true) => {
                let _ = window.hide();
            }
            _ => {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    }
}
