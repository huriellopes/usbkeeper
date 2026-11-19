//! Escolhe as implementações da plataforma atual.

use usbkeeper_core::{BlockDevice, CoreError, Ejector, Enumerator, Formatter, Mounts, Repairer};

pub type OpenRaw = fn(&str, bool) -> Result<Box<dyn BlockDevice>, CoreError>;

pub struct Backend {
    pub enumerator: Box<dyn Enumerator>,
    pub formatter: Box<dyn Formatter>,
    pub mounts: Box<dyn Mounts>,
    pub ejector: Box<dyn Ejector>,
    pub repairer: Box<dyn Repairer>,
    /// Abre o dispositivo bruto (caminho, escrita?).
    pub open_raw: OpenRaw,
}

#[cfg(target_os = "linux")]
pub fn backend(include_virtual: bool) -> Backend {
    use usbkeeper_linux::{
        LinuxEjector, LinuxEnumerator, LinuxFormatter, LinuxMounts, LinuxRepairer, RawDevice,
    };
    Backend {
        enumerator: Box::new(LinuxEnumerator {
            include_virtual,
            ..Default::default()
        }),
        formatter: Box::new(LinuxFormatter::default()),
        mounts: Box::new(LinuxMounts),
        ejector: Box::new(LinuxEjector),
        repairer: Box::new(LinuxRepairer::default()),
        open_raw: |path, write| Ok(Box::new(RawDevice::open(path, write)?)),
    }
}

#[cfg(windows)]
pub fn backend(include_virtual: bool) -> Backend {
    use usbkeeper_windows::{RawDevice, WindowsEnumerator};
    let next = || Box::new(unsupported::Unsupported(unsupported::M4B));
    Backend {
        enumerator: Box::new(WindowsEnumerator { include_virtual }),
        formatter: next(),
        mounts: next(),
        ejector: next(),
        repairer: next(),
        open_raw: |path, write| {
            if write {
                return Err(CoreError::Backend(unsupported::M4B.into()));
            }
            Ok(Box::new(RawDevice::open(path, false)?))
        },
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
pub fn backend(_include_virtual: bool) -> Backend {
    let none = || Box::new(unsupported::Unsupported(unsupported::LATER));
    Backend {
        enumerator: none(),
        formatter: none(),
        mounts: none(),
        ejector: none(),
        repairer: none(),
        open_raw: |_, _| Err(CoreError::Backend(unsupported::LATER.into())),
    }
}

#[cfg(not(target_os = "linux"))]
pub mod unsupported {
    use usbkeeper_core::{
        CoreError, Device, Ejector, Enumerator, Formatter, Holder, Mounts, Repairer, Step,
        format::FormatRequest,
    };

    #[cfg_attr(not(windows), allow(dead_code))]
    pub const M4B: &str = "no Windows esta operação chega na próxima etapa (M4b); \
         por enquanto funcionam list, info, health, bench (leitura), history, report, recomendar e guia";
    #[cfg_attr(windows, allow(dead_code))]
    pub const LATER: &str = "plataforma ainda não suportada (macOS chega numa próxima etapa)";

    pub struct Unsupported(pub &'static str);

    impl Unsupported {
        fn err(&self) -> CoreError {
            CoreError::Backend(self.0.into())
        }
    }
    impl Enumerator for Unsupported {
        fn list(&self) -> Result<Vec<Device>, CoreError> {
            Err(self.err())
        }
    }
    impl Formatter for Unsupported {
        fn plan(&self, _: &Device, _: &FormatRequest) -> Result<Vec<Step>, CoreError> {
            Err(self.err())
        }
        fn execute(&self, _: &[Step]) -> Result<(), CoreError> {
            Err(self.err())
        }
    }
    impl Mounts for Unsupported {
        fn unmount_all(&self, _: &Device) -> Result<(), CoreError> {
            Err(self.err())
        }
        fn holders(&self, _: &Device) -> Vec<Holder> {
            Vec::new()
        }
        fn mount_all(&self, _: &Device) -> Result<Vec<String>, CoreError> {
            Err(self.err())
        }
    }
    impl Repairer for Unsupported {
        fn repair(
            &self,
            _: &Device,
            _: bool,
            _: &mut dyn FnMut(&str, u64, u64),
        ) -> Result<usbkeeper_core::repair::RepairReport, CoreError> {
            Err(self.err())
        }
    }
    impl Ejector for Unsupported {
        fn power_off(&self, _: &Device) -> Result<(), CoreError> {
            Err(self.err())
        }
    }
}
