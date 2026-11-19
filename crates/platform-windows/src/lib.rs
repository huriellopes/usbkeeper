//! Implementação Windows do usbkeeper.
//!
//! A interpretação da saída do PowerShell ([`storage`]) é pura e compila em
//! qualquer sistema (testada no CI Linux também); o que executa coisas é
//! exclusivo do Windows.

pub mod paths;
pub mod storage;

#[cfg(windows)]
mod admin;
#[cfg(windows)]
mod enumerate;
#[cfg(windows)]
mod mounts;
#[cfg(windows)]
mod rawdev;

#[cfg(windows)]
pub use admin::is_elevated;
#[cfg(windows)]
pub use enumerate::WindowsEnumerator;
#[cfg(windows)]
pub use mounts::WindowsMounts;
#[cfg(windows)]
pub use rawdev::{RawDevice, lock_volumes};
