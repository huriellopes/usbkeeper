//! I/O bruto em `\\.\PhysicalDriveN`, sem o cache do Windows.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::fs::{FileExt, OpenOptionsExt};
use std::os::windows::io::AsRawHandle;

use usbkeeper_core::{BlockDevice, CoreError};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Ioctl::{GET_LENGTH_INFORMATION, IOCTL_DISK_GET_LENGTH_INFO};

const FILE_SHARE_READ_WRITE: u32 = 0x1 | 0x2;
const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
const FILE_FLAG_WRITE_THROUGH: u32 = 0x8000_0000;
const ERROR_ACCESS_DENIED: i32 = 5;

pub const NEED_ADMIN: &str = "precisa de administrador: abra o terminal como administrador \
     (ou use `sudo usbkeeper …` no Windows 11 24H2+)";

pub struct RawDevice {
    file: File,
    size: u64,
}

fn friendly(path: &str, e: io::Error) -> CoreError {
    if e.raw_os_error() == Some(ERROR_ACCESS_DENIED) {
        CoreError::Backend(format!("{path}: {NEED_ADMIN}"))
    } else {
        CoreError::Backend(format!("abrindo {path}: {e}"))
    }
}

fn disk_length(file: &File) -> io::Result<u64> {
    let mut info = GET_LENGTH_INFORMATION::default();
    let mut returned = 0u32;
    // SAFETY: handle válido enquanto `file` existir; buffer do tamanho da estrutura.
    unsafe {
        DeviceIoControl(
            HANDLE(file.as_raw_handle()),
            IOCTL_DISK_GET_LENGTH_INFO,
            None,
            0,
            Some(&mut info as *mut _ as *mut _),
            size_of::<GET_LENGTH_INFORMATION>() as u32,
            Some(&mut returned),
            None,
        )
    }
    .map_err(|e| io::Error::from_raw_os_error(e.code().0 & 0xFFFF))?;
    Ok(info.Length as u64)
}

impl RawDevice {
    pub fn open(path: &str, write: bool) -> Result<Self, CoreError> {
        let file = OpenOptions::new()
            .read(true)
            .write(write)
            .share_mode(FILE_SHARE_READ_WRITE)
            .custom_flags(FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH)
            .open(path)
            .map_err(|e| friendly(path, e))?;
        // Disco físico responde ao IOCTL; um arquivo comum (testes) usa o tamanho do arquivo.
        let size = match disk_length(&file) {
            Ok(n) => n,
            Err(_) => file.metadata().map_err(|e| friendly(path, e))?.len(),
        };
        Ok(Self { file, size })
    }
}

impl BlockDevice for RawDevice {
    fn size(&self) -> u64 {
        self.size
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        let mut done = 0;
        while done < buf.len() {
            let n = self
                .file
                .seek_read(&mut buf[done..], offset + done as u64)?;
            if n == 0 {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "fim do disco"));
            }
            done += n;
        }
        Ok(())
    }

    fn write_at(&mut self, offset: u64, buf: &[u8]) -> io::Result<()> {
        let mut done = 0;
        while done < buf.len() {
            let n = self.file.seek_write(&buf[done..], offset + done as u64)?;
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "o disco não aceitou a escrita",
                ));
            }
            done += n;
        }
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        // Sem buffer do sistema (NO_BUFFERING + WRITE_THROUGH); garante o cache do disco.
        self.file.sync_all()
    }
}
