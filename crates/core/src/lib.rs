//! Núcleo portátil do usbkeeper: modelo de dados, regras de segurança e
//! contratos que cada backend de plataforma implementa.

pub mod advice;
pub mod bench;
pub mod blockdev;
pub mod error;
pub mod fakecheck;
pub mod fakedrive;
pub mod format;
pub mod image;
pub mod iso;
pub mod model;
pub mod pattern;
pub mod platform;
pub mod repair;
pub mod safety;
pub mod scan;
pub mod units;
pub mod wipe;

pub use blockdev::{BlockDevice, Phase, Progress};
pub use error::CoreError;
pub use model::{Device, DeviceIdentity, Partition, Transport, UsbInfo, UsbSpeed};
pub use platform::{Ejector, Enumerator, Formatter, Holder, Mounts, Repairer, Step};
