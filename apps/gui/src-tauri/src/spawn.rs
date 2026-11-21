//! Origem de uma operação: o processo da CLI com stdin/stdout/stderr ligados.
//! No Linux vem de `Command` (com `pkexec` se precisa de root); no Windows,
//! com privilégio, de `elevate` (UAC + named pipes).

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

pub struct Spawned {
    pub stdin: Box<dyn Write + Send>,
    pub stdout: Box<dyn Read + Send>,
    pub stderr: Box<dyn Read + Send>,
    /// Espera o fim e devolve o código de saída.
    pub wait: Box<dyn FnOnce() -> Option<i32> + Send>,
}

pub fn spawn(cli: &Path, args: &[String], elevated: bool) -> Result<Spawned, String> {
    if elevated {
        privileged(cli, args)
    } else {
        plain(Command::new(cli), args, cli)
    }
}

#[cfg(windows)]
fn privileged(cli: &Path, args: &[String]) -> Result<Spawned, String> {
    crate::elevate::spawn_elevated(cli, args)
}

#[cfg(not(windows))]
fn privileged(cli: &Path, args: &[String]) -> Result<Spawned, String> {
    let mut c = Command::new("pkexec");
    c.arg(cli);
    plain(c, args, cli)
}

/// Sem janela de console piscando no Windows (a CLI é um programa de console).
pub fn quiet(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn plain(mut cmd: Command, args: &[String], cli: &Path) -> Result<Spawned, String> {
    let mut child = quiet(&mut cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("não foi possível iniciar {}: {e}", cli.display()))?;
    let stdin = child.stdin.take().expect("stdin piped");
    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");
    Ok(Spawned {
        stdin: Box::new(stdin),
        stdout: Box::new(stdout),
        stderr: Box::new(stderr),
        wait: Box::new(move || child.wait().ok().and_then(|s| s.code())),
    })
}
