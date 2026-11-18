pub mod cli;
pub mod devices;
pub mod ops;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ops::Ops::default())
        .invoke_handler(tauri::generate_handler![
            devices::list_devices,
            devices::usb_hint,
            devices::device_holders,
            devices::history_events,
            ops::start_operation,
            ops::cancel_operation,
        ])
        .run(tauri::generate_context!())
        .expect("falha ao iniciar a interface");
}
