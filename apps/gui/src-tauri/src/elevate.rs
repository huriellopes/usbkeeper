//! Windows: a CLI elevada pelo UAC, ligada à janela por canais privados
//! (ver `usbkeeper_windows::pipes`).

use std::path::Path;

use crate::spawn::Spawned;

pub fn spawn_elevated(cli: &Path, args: &[String]) -> Result<Spawned, String> {
    let e = usbkeeper_windows::pipes::run_elevated(cli, args)?;
    let process = e.process;
    Ok(Spawned {
        stdin: Box::new(e.stdin),
        stdout: Box::new(e.stdout),
        stderr: Box::new(e.stderr),
        wait: Box::new(move || process.wait()),
    })
}
