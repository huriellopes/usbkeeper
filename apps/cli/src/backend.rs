//! Escolhe as implementações da plataforma atual.

use usbkeeper_core::{BlockDevice, CoreError, Ejector, Enumerator, Formatter, Mounts};

pub type OpenRaw = fn(&str, bool) -> Result<Box<dyn BlockDevice>, CoreError>;

pub struct Backend {
    pub enumerator: Box<dyn Enumerator>,
    pub formatter: Box<dyn Formatter>,
    pub mounts: Box<dyn Mounts>,
    pub ejector: Box<dyn Ejector>,
    /// Abre o dispositivo bruto (caminho, escrita?).
    pub open_raw: OpenRaw,
}

#[cfg(target_os = "linux")]
pub fn backend(include_virtual: bool) -> Backend {
    use usbkeeper_linux::{LinuxEjector, LinuxEnumerator, LinuxFormatter, LinuxMounts, RawDevice};
    Backend {
        enumerator: Box::new(LinuxEnumerator {
            include_virtual,
            ..Default::default()
        }),
        formatter: Box::new(LinuxFormatter::default()),
        mounts: Box::new(LinuxMounts),
        ejector: Box::new(LinuxEjector),
        open_raw: |path, write| Ok(Box::new(RawDevice::open(path, write)?)),
    }
}

#[cfg(not(target_os = "linux"))]
pub fn backend(_include_virtual: bool) -> Backend {
    use usbkeeper_core::{Device, Holder, Step, format::FormatRequest};

    struct Unsupported;
    fn err() -> CoreError {
        CoreError::Backend(
            "plataforma ainda não suportada (Windows e macOS chegam nas próximas etapas)".into(),
        )
    }
    impl Enumerator for Unsupported {
        fn list(&self) -> Result<Vec<Device>, CoreError> {
            Err(err())
        }
    }
    impl Formatter for Unsupported {
        fn plan(&self, _: &Device, _: &FormatRequest) -> Result<Vec<Step>, CoreError> {
            Err(err())
        }
        fn execute(&self, _: &[Step]) -> Result<(), CoreError> {
            Err(err())
        }
    }
    impl Mounts for Unsupported {
        fn unmount_all(&self, _: &Device) -> Result<(), CoreError> {
            Err(err())
        }
        fn holders(&self, _: &Device) -> Vec<Holder> {
            Vec::new()
        }
    }
    impl Ejector for Unsupported {
        fn power_off(&self, _: &Device) -> Result<(), CoreError> {
            Err(err())
        }
    }
    Backend {
        enumerator: Box::new(Unsupported),
        formatter: Box::new(Unsupported),
        mounts: Box::new(Unsupported),
        ejector: Box::new(Unsupported),
        open_raw: |_, _| Err(err()),
    }
}
