// The tauri_panel! macro generates code with `-> ()` that clippy flags.
#![allow(clippy::unused_unit)]

mod background;
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

pub fn run_with_sentry(client: sentry::ClientInitGuard) {
    let builder = base_builder().plugin(tauri_plugin_sentry::init(&client));
    finish_builder(builder);
    drop(client);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    finish_builder(base_builder());
}

fn base_builder() -> tauri::Builder<tauri::Wry> {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_overlay(app);
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
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
        let mut autostart = tauri_plugin_autostart::Builder::new().arg("--background");
        #[cfg(target_os = "macos")]
        {
            autostart =
                autostart.macos_launcher(tauri_plugin_autostart::MacosLauncher::LaunchAgent);
        }
        builder = builder.plugin(autostart.build());
    }

    builder
}

fn finish_builder(builder: tauri::Builder<tauri::Wry>) {
    builder
        .manage(background::SyncLock::default())
        .manage(background::IndexActivityState::default())
        .manage(findr_client::SearchProcessState::default())
        .manage(commands::AuthorizedPaths::default())
        .manage(license::ValidationCacheState::default())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            #[cfg(target_os = "macos")]
            setup_macos_panel(app);

            if !std::env::args().any(|arg| arg == "--background") {
                show_overlay(app.handle());
            }

            #[cfg(not(target_os = "macos"))]
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_visible_on_all_workspaces(true);
                let win_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        if let Err(e) = win_clone.hide() {
                            eprintln!("[lib] failed to hide main window on focus loss: {e}");
                        }
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
            let settings_item =
                MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[
                    &show_item,
                    &settings_item,
                    &PredefinedMenuItem::separator(app)?,
                    &quit_item,
                ],
            )?;

            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            // Build tray icon — log and continue if it fails
            let tray_icon = app.default_window_icon().cloned();
            match tray_icon {
                Some(icon) => {
                    match TrayIconBuilder::with_id("main-tray")
                        .icon(icon)
                        .icon_as_template(true)
                        .menu(&menu)
                        .show_menu_on_left_click(false)
                        .on_menu_event(|app, event| match event.id.as_ref() {
                            "show" => show_overlay(app),
                            "settings" => show_settings(app),
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
                        .build(app)
                    {
                        Ok(_tray) => {}
                        Err(e) => {
                            eprintln!("[lib] failed to build tray icon: {e}");
                        }
                    }
                }
                None => {
                    eprintln!("[lib] no default window icon available, skipping tray icon");
                }
            }

            let _shutdown_flag = background::spawn_index_daemon(app.handle().clone());
            // TODO: store shutdown_flag in managed state if graceful shutdown on exit is needed

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "settings" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    if let Err(e) = window.hide() {
                        eprintln!("[lib] failed to hide settings window on close: {e}");
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::search,
            commands::get_recent_files,
            commands::read_preview_text,
            commands::open_result,
            commands::reveal_result,
            commands::copy_text,
            commands::track_interaction,
            commands::get_index_status,
            commands::get_index_activity,
            commands::uses_legacy_opaque_overlay,
            commands::get_findr_version,
            commands::hide_overlay,
            commands::get_license_state,
            commands::activate_license,
            commands::start_trial,
            commands::get_trial_days_remaining,
            commands::move_to_trash,
            commands::open_settings,
            commands::get_doctor_report,
            commands::add_scan_path,
            commands::remove_scan_path,
            commands::run_reindex,
            commands::run_sync,
            commands::set_api_key,
            commands::get_api_key_status,
            commands::get_home_dir,
            commands::get_autostart_status,
            commands::set_autostart,
            commands::get_theme,
            commands::set_theme,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                show_overlay(app);
            }
        });
}

// -- macOS: NSPanel overlay (works over fullscreen apps) --

#[cfg(target_os = "macos")]
fn setup_macos_panel(app: &tauri::App) {
    let window = match app.get_webview_window("main") {
        Some(w) => w,
        None => {
            eprintln!("[lib] main window not found, skipping NSPanel setup");
            return;
        }
    };

    let panel = match window.to_panel::<FindrPanel>() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[lib] failed to convert main window to NSPanel: {e}");
            eprintln!("[lib] falling back to regular window behavior");
            return;
        }
    };

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

fn show_settings<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("settings") {
        if let Err(e) = window.show() {
            eprintln!("[lib] failed to show settings window: {e}");
        }
        if let Err(e) = window.set_focus() {
            eprintln!("[lib] failed to focus settings window: {e}");
        }
    }
}

// -- Non-macOS fallback --

#[cfg(not(target_os = "macos"))]
fn show_overlay<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(e) = window.show() {
            eprintln!("[lib] failed to show main window: {e}");
        }
        if let Err(e) = window.set_focus() {
            eprintln!("[lib] failed to focus main window: {e}");
        }
        if let Err(e) = window.center() {
            eprintln!("[lib] failed to center main window: {e}");
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn toggle_overlay<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        match window.is_visible() {
            Ok(true) => {
                if let Err(e) = window.hide() {
                    eprintln!("[lib] failed to hide main window: {e}");
                }
            }
            _ => {
                if let Err(e) = window.show() {
                    eprintln!("[lib] failed to show main window: {e}");
                }
                if let Err(e) = window.set_focus() {
                    eprintln!("[lib] failed to focus main window: {e}");
                }
            }
        }
    }
}
