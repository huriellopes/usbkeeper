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
    use usbkeeper_windows::{
        RawDevice, WindowsEjector, WindowsEnumerator, WindowsFormatter, WindowsMounts,
        WindowsRepairer,
    };
    Backend {
        enumerator: Box::new(WindowsEnumerator { include_virtual }),
        formatter: Box::new(WindowsFormatter {
            // O FAT32 acima de 32 GiB é gravado pela própria CLI (subcomando oculto).
            self_exe: std::env::current_exe()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "usbkeeper.exe".into()),
        }),
        mounts: Box::new(WindowsMounts),
        ejector: Box::new(WindowsEjector),
        repairer: Box::new(WindowsRepairer),
        open_raw: |path, write| Ok(Box::new(RawDevice::open(path, write)?)),
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

#[cfg(not(any(target_os = "linux", windows)))]
pub mod unsupported {
    use usbkeeper_core::{
        CoreError, Device, Ejector, Enumerator, Formatter, Holder, Mounts, Repairer, Step,
        format::FormatRequest,
    };

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
