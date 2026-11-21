//! WebView2 com pouca memória enquanto a janela está minimizada.

use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
    ICoreWebView2_19,
};
use windows_core::Interface;

/// Pede ao WebView2 para liberar memória (`low`) ou voltar ao normal.
/// WebView2 antigo, sem a interface 19: ignora em silêncio.
pub fn low_memory(window: &tauri::WebviewWindow, low: bool) {
    let _ = window.with_webview(move |w| unsafe {
        let Ok(core) = w.controller().CoreWebView2() else {
            return;
        };
        let Ok(v19) = core.cast::<ICoreWebView2_19>() else {
            return;
        };
        let level = if low {
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
        } else {
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
        };
        let _ = v19.SetMemoryUsageTargetLevel(level);
    });
}
