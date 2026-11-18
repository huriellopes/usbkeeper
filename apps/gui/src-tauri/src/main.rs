#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // O compositing do WebKitGTK guarda camadas em memória sem ganho visível
    // numa UI simples. USBKEEPER_COMPOSITING=1 mantém o padrão do WebKit.
    if std::env::var_os("USBKEEPER_COMPOSITING").is_none()
        && std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none()
    {
        // SAFETY: ainda single-thread, antes de qualquer thread do Tauri.
        unsafe { std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1") };
    }
    usbkeeper_gui_lib::run()
}
