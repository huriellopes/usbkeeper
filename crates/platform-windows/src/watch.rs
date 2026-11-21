//! Avisos do Windows quando um disco ou volume chega ou sai, sem thread,
//! janela nem consulta periódica: o callback só incrementa um contador, que
//! entra na assinatura barata da lista de discos.

use std::sync::atomic::{AtomicU64, Ordering};

use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_NOTIFY_ACTION, CM_NOTIFY_EVENT_DATA, CM_NOTIFY_FILTER, CM_NOTIFY_FILTER_0,
    CM_NOTIFY_FILTER_0_0, CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE, CM_Register_Notification,
    CR_SUCCESS, HCMNOTIFICATION,
};
use windows::Win32::Storage::FileSystem::GetLogicalDrives;
use windows::Win32::System::Ioctl::{GUID_DEVINTERFACE_DISK, GUID_DEVINTERFACE_VOLUME};

static GENERATION: AtomicU64 = AtomicU64::new(0);

unsafe extern "system" fn changed(
    _h: HCMNOTIFICATION,
    _ctx: *const core::ffi::c_void,
    _action: CM_NOTIFY_ACTION,
    _data: *const CM_NOTIFY_EVENT_DATA,
    _size: u32,
) -> u32 {
    GENERATION.fetch_add(1, Ordering::Relaxed);
    0
}

/// Registra os avisos (uma vez; valem até o processo terminar). Se falhar,
/// a lista ainda é relida pelas letras de unidade e pelo prazo máximo.
pub fn start() {
    for guid in [GUID_DEVINTERFACE_DISK, GUID_DEVINTERFACE_VOLUME] {
        let filter = CM_NOTIFY_FILTER {
            cbSize: size_of::<CM_NOTIFY_FILTER>() as u32,
            FilterType: CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE,
            u: CM_NOTIFY_FILTER_0 {
                DeviceInterface: CM_NOTIFY_FILTER_0_0 { ClassGuid: guid },
            },
            ..Default::default()
        };
        let mut handle = HCMNOTIFICATION::default();
        let r = unsafe { CM_Register_Notification(&filter, None, Some(changed), &mut handle) };
        if r != CR_SUCCESS {
            GENERATION.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Muda quando um disco/volume chega ou sai, ou quando as letras mudam.
pub fn signature() -> String {
    let drives = unsafe { GetLogicalDrives() };
    format!("{}\n{drives:x}", GENERATION.load(Ordering::Relaxed))
}
