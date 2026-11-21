//! I/O bruto em `\\.\PhysicalDriveN`, sem o cache do Windows.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::fs::{FileExt, OpenOptionsExt};
use std::os::windows::io::AsRawHandle;

use usbkeeper_core::{BlockDevice, CoreError};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{
    FindFirstVolumeW, FindNextVolumeW, FindVolumeClose, GetVolumePathNamesForVolumeNameW,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Ioctl::{
    FSCTL_DISMOUNT_VOLUME, FSCTL_LOCK_VOLUME, GET_LENGTH_INFORMATION, IOCTL_DISK_GET_LENGTH_INFO,
    IOCTL_DISK_UPDATE_PROPERTIES,
};
use windows::core::PCWSTR;

use crate::paths::{disk_number, extent_disks, volume_device};

/// `CTL_CODE(IOCTL_VOLUME_BASE = 'V', 0, METHOD_BUFFERED, FILE_ANY_ACCESS)`; não exportado pela crate.
const IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS: u32 = 0x0056_0000;
const LOCK_TRIES: u32 = 5;

const FILE_SHARE_READ_WRITE: u32 = 0x1 | 0x2;
const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
const FILE_FLAG_WRITE_THROUGH: u32 = 0x8000_0000;
const ERROR_ACCESS_DENIED: i32 = 5;

pub const NEED_ADMIN: &str = "precisa de administrador: abra o terminal como administrador \
     (ou use `sudo usbkeeper …` no Windows 11 24H2+)";

pub struct RawDevice {
    file: File,
    size: u64,
    /// Volumes do disco travados e desmontados enquanto se grava (soltos ao descartar).
    _locked: Vec<File>,
    wrote: bool,
}

fn friendly(path: &str, e: io::Error) -> CoreError {
    if e.raw_os_error() == Some(ERROR_ACCESS_DENIED) {
        CoreError::Backend(format!("{path}: {NEED_ADMIN}"))
    } else {
        CoreError::Backend(format!("abrindo {path}: {e}"))
    }
}

/// `DeviceIoControl` sem buffers de entrada; devolve os bytes de saída.
fn ioctl(file: &File, code: u32, out: &mut [u8]) -> io::Result<usize> {
    let mut returned = 0u32;
    // SAFETY: handle válido enquanto `file` existir; `out` vive durante a chamada.
    unsafe {
        DeviceIoControl(
            HANDLE(file.as_raw_handle()),
            code,
            None,
            0,
            (!out.is_empty()).then_some(out.as_mut_ptr() as *mut _),
            out.len() as u32,
            Some(&mut returned),
            None,
        )
    }
    .map_err(|e| io::Error::from_raw_os_error(e.code().0 & 0xFFFF))?;
    Ok(returned as usize)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// Todos os volumes do sistema (`\\?\Volume{…}\`).
fn all_volumes() -> Vec<String> {
    let mut names = Vec::new();
    let mut buf = [0u16; 64];
    // SAFETY: buffer local; o handle de busca é fechado no fim.
    unsafe {
        let Ok(find) = FindFirstVolumeW(&mut buf) else {
            return names;
        };
        loop {
            names.push(from_wide(&buf));
            if FindNextVolumeW(find, &mut buf).is_err() {
                break;
            }
        }
        let _ = FindVolumeClose(find);
    }
    names
}

/// Letra/pasta de um volume para mensagens (`E:\`), ou o próprio nome.
fn volume_label(name: &str) -> String {
    let mut buf = [0u16; 260];
    let mut len = 0u32;
    let w = wide(name);
    // SAFETY: `w` termina em zero e vive durante a chamada; buffer local.
    let ok =
        unsafe { GetVolumePathNamesForVolumeNameW(PCWSTR(w.as_ptr()), Some(&mut buf), &mut len) }
            .is_ok();
    let first = from_wide(&buf);
    if ok && !first.is_empty() {
        first
    } else {
        name.to_string()
    }
}

/// Volumes com alguma extensão no disco `n`.
fn volumes_on_disk(n: u32) -> Vec<String> {
    all_volumes()
        .into_iter()
        .filter(|v| {
            // Sem acesso de leitura/escrita: só consulta (não toca nos volumes de outros discos).
            let Ok(f) = OpenOptions::new()
                .access_mode(0)
                .share_mode(FILE_SHARE_READ_WRITE)
                .open(volume_device(v))
            else {
                return false;
            };
            let mut out = [0u8; 8 + 24 * 16];
            ioctl(&f, IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS, &mut out)
                .map(|len| extent_disks(&out[..len]).contains(&n))
                .unwrap_or(false)
        })
        .collect()
}

/// Trava e desmonta todos os volumes do disco `n`. Os handles devolvidos
/// mantêm a trava; ao descartá-los, o Windows volta a montar sob demanda.
pub fn lock_volumes(n: u32) -> Result<Vec<File>, CoreError> {
    let mut locked = Vec::new();
    for v in volumes_on_disk(n) {
        let in_use = || {
            CoreError::Backend(format!(
                "o volume {} está em uso: feche as janelas do Explorer e os programas que usam \
                 o pendrive e tente de novo (nada foi gravado)",
                volume_label(&v)
            ))
        };
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(FILE_SHARE_READ_WRITE)
            .open(volume_device(&v))
            .map_err(|e| friendly(&volume_label(&v), e))?;
        let mut ok = false;
        for attempt in 0..LOCK_TRIES {
            if ioctl(&f, FSCTL_LOCK_VOLUME, &mut []).is_ok() {
                ok = true;
                break;
            }
            if attempt + 1 < LOCK_TRIES {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        }
        if !ok {
            return Err(in_use());
        }
        ioctl(&f, FSCTL_DISMOUNT_VOLUME, &mut []).map_err(|_| in_use())?;
        locked.push(f);
    }
    Ok(locked)
}

fn disk_length(file: &File) -> io::Result<u64> {
    let mut out = [0u8; size_of::<GET_LENGTH_INFORMATION>()];
    ioctl(file, IOCTL_DISK_GET_LENGTH_INFO, &mut out)?;
    Ok(i64::from_le_bytes(out[..8].try_into().unwrap()) as u64)
}

impl RawDevice {
    pub fn open(path: &str, write: bool) -> Result<Self, CoreError> {
        // O Windows recusa gravar em setores de volume montado: trava antes.
        let locked = match (write, disk_number(path)) {
            (true, Some(n)) => lock_volumes(n)?,
            _ => Vec::new(),
        };
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
        Ok(Self {
            file,
            size,
            _locked: locked,
            wrote: false,
        })
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
        self.wrote = true;
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
        // A releitura da tabela (IOCTL_DISK_UPDATE_PROPERTIES) fica para o Drop:
        // ao reler, o Windows monta o que foi gravado e pode escrever nele (bit
        // de "sujo" do NTFS, assinatura de disco MBR), o que estragaria a
        // conferência por hash feita logo depois do flush.
        self.file.sync_all()
    }
}

/// Ao soltar o disco (depois de gravar e conferir): faz o Windows reler a
/// tabela de partições, como o `udevadm settle` no Linux.
impl Drop for RawDevice {
    fn drop(&mut self) {
        if self.wrote {
            let _ = self.file.sync_all();
            let _ = ioctl(&self.file, IOCTL_DISK_UPDATE_PROPERTIES, &mut []);
        }
    }
}
