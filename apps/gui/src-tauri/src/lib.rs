pub mod cli;
pub mod devices;
#[cfg(windows)]
pub mod elevate;
pub mod guide;
pub mod ops;
pub mod spawn;
pub mod throttle;
#[cfg(windows)]
mod webview;

use std::sync::atomic::Ordering;
use tauri::{Manager, WindowEvent};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ops::Ops::default())
        .manage(devices::DeviceCache::default())
        .manage(devices::Hidden::default())
        .setup(|_app| {
            // Avisos do sistema quando disco/volume chega ou sai: a lista só
            // é relida (PowerShell) quando algo mudou.
            #[cfg(windows)]
            usbkeeper_windows::watch::start();
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(
                event,
                WindowEvent::Resized(_) | WindowEvent::Focused(_) | WindowEvent::Moved(_)
            ) {
                let hidden =
                    window.is_minimized().unwrap_or(false) || !window.is_visible().unwrap_or(true);
                let state = window.state::<devices::Hidden>();
                let was = state.0.swap(hidden, Ordering::Relaxed);
                if was != hidden {
                    on_hidden_changed(window, hidden);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            devices::list_devices,
            devices::usb_hint,
            devices::device_holders,
            devices::history_events,
            devices::advise,
            devices::mount_device,
            devices::open_folder,
            devices::image_meta,
            devices::platform,
            guide::guide_topics,
            ops::start_operation,
            ops::cancel_operation,
            ops::save_report,
        ])
        .run(tauri::generate_context!())
        .expect("falha ao iniciar a interface");
}

/// Minimizada: o WebView2 passa a poupar memória (Windows); restaurada, volta.
#[cfg(windows)]
fn on_hidden_changed(window: &tauri::Window, hidden: bool) {
    if let Some(w) = window.app_handle().get_webview_window(window.label()) {
        webview::low_memory(&w, hidden);
    }
}

#[cfg(not(windows))]
fn on_hidden_changed(_window: &tauri::Window, _hidden: bool) {}
