//! WSL2 não enxerga discos USB do Windows; é preciso repassá-los com usbipd-win.

pub const USBIPD_HINT: &str = "\
Você está no WSL2: discos USB do Windows não aparecem aqui automaticamente.
No PowerShell do Windows (como administrador):
  winget install usbipd
  usbipd list
  usbipd bind --busid <BUSID>
  usbipd attach --wsl --busid <BUSID>
Depois rode o usbkeeper de novo. Para formatar no lado Windows, prefira a versão Windows.";

pub fn is_wsl_kernel(proc_version: &str) -> bool {
    proc_version.to_ascii_lowercase().contains("microsoft")
}

pub fn running_in_wsl() -> bool {
    std::fs::read_to_string("/proc/version")
        .map(|v| is_wsl_kernel(&v))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_wsl_kernel() {
        assert!(is_wsl_kernel(
            "Linux version 6.6.87.2-microsoft-standard-WSL2 (root@host) ..."
        ));
        assert!(!is_wsl_kernel(
            "Linux version 7.0.0-43-generic (buildd@lcy02) ..."
        ));
    }
}
