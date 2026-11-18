pub mod devices;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            devices::list_devices,
            devices::usb_hint,
            devices::device_holders,
            devices::history_events,
        ])
        .run(tauri::generate_context!())
        .expect("falha ao iniciar a interface");
}
