//! Núcleo portátil do usbkeeper: modelo de dados, regras de segurança e
//! contratos que cada backend de plataforma implementa.

pub mod bench;
pub mod blockdev;
pub mod error;
pub mod fakecheck;
pub mod fakedrive;
pub mod format;
pub mod model;
pub mod pattern;
pub mod platform;
pub mod safety;
pub mod units;

pub use blockdev::{BlockDevice, Phase, Progress};
pub use error::CoreError;
pub use model::{Device, DeviceIdentity, Partition, Transport, UsbInfo, UsbSpeed};
pub use platform::{Ejector, Enumerator, Formatter, Holder, Mounts, Step};
