pub mod cli;
pub mod devices;
pub mod guide;
pub mod ops;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ops::Ops::default())
        .manage(devices::DeviceCache::default())
        .invoke_handler(tauri::generate_handler![
            devices::list_devices,
            devices::usb_hint,
            devices::device_holders,
            devices::history_events,
            devices::advise,
            devices::mount_device,
            devices::open_folder,
            devices::image_meta,
            guide::guide_topics,
            ops::start_operation,
            ops::cancel_operation,
            ops::save_report,
        ])
        .run(tauri::generate_context!())
        .expect("falha ao iniciar a interface");
}
