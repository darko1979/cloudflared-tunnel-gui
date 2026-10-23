mod cloudflared;
mod commands;
mod config;
mod process;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, RunEvent};

const TRAY_ID: &str = "main";

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub(crate) fn update_tray_tooltip(app: &AppHandle, running: bool) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let status = if running { "Tunnel running" } else { "Tunnel stopped" };
        let _ = tray.set_tooltip(Some(format!("Cloudflared Tunnel GUI - {status}")));
    }
}

fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &PredefinedMenuItem::separator(app)?, &quit])?;

    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
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
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    update_tray_tooltip(app, false);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // A hidden window in the tray should be brought back, not duplicated
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main_window(app)))
        .plugin(tauri_plugin_opener::init())
        .manage(process::TunnelState::default())
        .setup(|app| {
            // Minimize to the system tray (macOS keeps its normal Dock behaviour)
            if !cfg!(target_os = "macos") {
                create_tray(app.handle())?;
                if let Some(window) = app.get_webview_window("main") {
                    let w = window.clone();
                    window.on_window_event(move |event| {
                        if let tauri::WindowEvent::Resized(_) = event {
                            if w.is_minimized().unwrap_or(false) {
                                let _ = w.hide();
                            }
                        }
                    });
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::cloudflared_check,
            commands::auth_login,
            commands::auth_check,
            commands::auth_logout,
            commands::config_get,
            commands::config_save,
            commands::tunnel_list,
            commands::tunnel_create,
            commands::tunnel_update,
            commands::tunnel_delete,
            commands::tunnel_status,
            commands::tunnel_start,
            commands::tunnel_stop,
            commands::tunnel_restart,
            commands::tunnel_route_dns,
            commands::tunnel_delete_dns,
            commands::shell_open_external,
            commands::shell_open_path,
            commands::network_check_port,
            commands::network_check_port_lsof,
            commands::dns_check,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Stop the tunnel before quitting
            if let RunEvent::Exit = event {
                tauri::async_runtime::block_on(process::stop(app));
            }
        });
}
