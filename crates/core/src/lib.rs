//! Núcleo portátil do usbkeeper: modelo de dados, regras de segurança e
//! contratos que cada backend de plataforma implementa.

pub mod error;
pub mod model;
pub mod platform;
pub mod safety;
pub mod units;

pub use error::CoreError;
pub use model::{Device, DeviceIdentity, Partition, Transport, UsbInfo, UsbSpeed};
pub use platform::Enumerator;
