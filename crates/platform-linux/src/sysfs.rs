//! Lê dados USB do sysfs subindo da entrada do disco até o nó USB que tem
//! `idVendor`/`speed`. A raiz é parâmetro para testes com árvore falsa.

use std::fs;
use std::path::Path;

use usbkeeper_core::{UsbInfo, UsbSpeed};

fn read(dir: &Path, attr: &str) -> Option<String> {
    fs::read_to_string(dir.join(attr))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// `sys_root` normalmente é `/sys`; `kernel_name` é algo como `sdb`.
pub fn usb_info(sys_root: &Path, kernel_name: &str) -> Option<UsbInfo> {
    let root = fs::canonicalize(sys_root).ok()?;
    let start = fs::canonicalize(sys_root.join("block").join(kernel_name)).ok()?;

    start
        .ancestors()
        .take_while(|dir| dir.starts_with(&root) && *dir != root)
        .find(|dir| dir.join("idVendor").is_file() && dir.join("speed").is_file())
        .map(|dir| UsbInfo {
            vendor_id: read(dir, "idVendor").unwrap_or_default(),
            product_id: read(dir, "idProduct").unwrap_or_default(),
            manufacturer: read(dir, "manufacturer"),
            product: read(dir, "product"),
            usb_version: read(dir, "version"),
            speed: read(dir, "speed")
                .map(|s| UsbSpeed::from_mbps(&s))
                .unwrap_or(UsbSpeed::Unknown),
        })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    /// Monta uma árvore parecida com a real:
    /// /sys/block/sdb -> ../devices/pci/usb2/2-1/2-1:1.0/host6/.../block/sdb
    fn fake_sys(speed: &str, version: &str) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let usb_dev = tmp.path().join("devices/pci0000:00/usb2/2-1");
        let block = usb_dev.join("2-1:1.0/host6/target6:0:0/6:0:0:0/block/sdb");
        fs::create_dir_all(&block).unwrap();
        for (attr, value) in [
            ("idVendor", "0951\n"),
            ("idProduct", "1666\n"),
            ("manufacturer", "Kingston\n"),
            ("product", "DataTraveler 3.0\n"),
            ("version", version),
            ("speed", speed),
        ] {
            fs::write(usb_dev.join(attr), value).unwrap();
        }
        // O controlador raiz também tem speed, mas sem idVendor no mesmo nível
        // que o dispositivo; não deve ser escolhido antes do 2-1.
        fs::write(tmp.path().join("devices/pci0000:00/usb2/speed"), "5000\n").unwrap();
        fs::create_dir_all(tmp.path().join("block")).unwrap();
        symlink(&block, tmp.path().join("block/sdb")).unwrap();
        tmp
    }

    #[test]
    fn finds_usb_ancestor() {
        let sys = fake_sys("480\n", " 3.20\n");
        let info = usb_info(sys.path(), "sdb").expect("deveria achar o nó USB");
        assert_eq!(info.vendor_id, "0951");
        assert_eq!(info.product.as_deref(), Some("DataTraveler 3.0"));
        assert_eq!(info.speed, UsbSpeed::High);
        assert_eq!(info.usb_version.as_deref(), Some("3.20"));
        assert!(info.is_bottlenecked());
    }

    #[test]
    fn non_usb_disk_has_no_info() {
        let tmp = tempfile::tempdir().unwrap();
        let block = tmp.path().join("devices/pci0000:00/ata1/host0/block/sda");
        fs::create_dir_all(&block).unwrap();
        fs::create_dir_all(tmp.path().join("block")).unwrap();
        symlink(&block, tmp.path().join("block/sda")).unwrap();
        assert_eq!(usb_info(tmp.path(), "sda"), None);
    }

    #[test]
    fn missing_device_has_no_info() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(usb_info(tmp.path(), "sdz"), None);
    }
}
