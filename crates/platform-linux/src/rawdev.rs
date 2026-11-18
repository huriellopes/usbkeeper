//! Dispositivo de blocos bruto no Linux: O_DIRECT (sem page cache, para que
//! a leitura de volta venha mesmo do pendrive) e O_EXCL na escrita (o kernel
//! recusa se houver partição montada ou outro programa com acesso exclusivo).

use std::fs::{File, OpenOptions};
use std::io::{self, Seek, SeekFrom};
use std::os::unix::fs::{FileExt, OpenOptionsExt};
use std::os::unix::io::AsRawFd;

use usbkeeper_core::{BlockDevice, CoreError};

pub struct RawDevice {
    file: File,
    size: u64,
    direct: bool,
}

fn open_with(path: &str, write: bool, direct: bool) -> io::Result<File> {
    let mut flags = 0;
    if direct {
        flags |= libc::O_DIRECT;
    }
    if write {
        flags |= libc::O_EXCL;
    }
    OpenOptions::new()
        .read(true)
        .write(write)
        .custom_flags(flags)
        .open(path)
}

fn friendly(path: &str, err: io::Error) -> CoreError {
    match err.raw_os_error() {
        Some(libc::EBUSY) => CoreError::Backend(format!(
            "{path} está em uso (montado ou aberto por outro programa); desmonte antes"
        )),
        Some(libc::EACCES | libc::EPERM) => {
            CoreError::Backend(format!("sem permissão para acessar {path}; rode com sudo"))
        }
        Some(libc::EROFS) => CoreError::Backend(format!(
            "{path} está protegido contra gravação (trava física ou do sistema)"
        )),
        Some(libc::ENOMEDIUM) => CoreError::Backend(format!("{path} está sem mídia")),
        _ => CoreError::Io(err),
    }
}

impl RawDevice {
    pub fn open(path: &str, write: bool) -> Result<Self, CoreError> {
        // Alguns sistemas de arquivos (tmpfs) não aceitam O_DIRECT: nesse caso
        // segue sem ele e descarta o cache manualmente em flush().
        let (mut file, direct) = match open_with(path, write, true) {
            Ok(f) => (f, true),
            Err(e) if e.raw_os_error() == Some(libc::EINVAL) => (
                open_with(path, write, false).map_err(|e| friendly(path, e))?,
                false,
            ),
            Err(e) => return Err(friendly(path, e)),
        };
        let size = file.seek(SeekFrom::End(0))?;
        Ok(Self { file, size, direct })
    }

    pub fn is_direct(&self) -> bool {
        self.direct
    }
}

impl BlockDevice for RawDevice {
    fn size(&self) -> u64 {
        self.size
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        self.file.read_exact_at(buf, offset)
    }

    fn write_at(&mut self, offset: u64, buf: &[u8]) -> io::Result<()> {
        self.file.write_all_at(buf, offset)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.sync_all()?;
        if !self.direct {
            // SAFETY: fd válido enquanto `self.file` existir.
            let rc = unsafe {
                libc::posix_fadvise(self.file.as_raw_fd(), 0, 0, libc::POSIX_FADV_DONTNEED)
            };
            if rc != 0 {
                return Err(io::Error::from_raw_os_error(rc));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use usbkeeper_core::blockdev::{AlignedBuf, Silent};
    use usbkeeper_core::fakecheck::{QuickOptions, Verdict, full_check, quick_probe};

    fn image(size: u64) -> (tempfile::TempDir, String) {
        // Dentro de target/ (disco de verdade); /tmp pode ser tmpfs.
        let dir =
            tempfile::tempdir_in(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target")).unwrap();
        let path = dir.path().join("pen.img");
        File::create(&path).unwrap().set_len(size).unwrap();
        (dir, path.to_string_lossy().into_owned())
    }

    #[test]
    fn roundtrip_on_image_file() {
        let (_dir, path) = image(8 << 20);
        let mut dev = RawDevice::open(&path, true).unwrap();
        assert_eq!(dev.size(), 8 << 20);
        let mut buf = AlignedBuf::new(4096);
        buf[..4].copy_from_slice(b"oi!!");
        dev.write_at(1 << 20, &buf).unwrap();
        dev.flush().unwrap();
        let mut back = AlignedBuf::new(4096);
        dev.read_at(1 << 20, &mut back).unwrap();
        assert_eq!(&back[..4], b"oi!!");
    }

    #[test]
    fn genuine_image_passes_both_checks() {
        let (_dir, path) = image(32 << 20);
        let mut dev = RawDevice::open(&path, true).unwrap();
        let never = AtomicBool::new(false);
        let opts = QuickOptions {
            seed: 5,
            backup: true,
        };
        let quick = quick_probe(&mut dev, opts, &mut Silent, &never).unwrap();
        assert_eq!(quick.verdict, Verdict::Genuine);
        let full = full_check(&mut dev, 6, &mut Silent, &never).unwrap();
        assert_eq!(full.verdict, Verdict::Genuine);
        assert_eq!(full.ok_bytes, 32 << 20);
    }

    #[test]
    fn missing_path_is_an_error() {
        assert!(RawDevice::open("/dev/usbkeeper-nao-existe", false).is_err());
    }
}
