//! Execução das operações: `pkexec <cli> …` com stdin/stderr ligados, eventos
//! de progresso e cancelamento pela linha `cancel`.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{ChildStdin, Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use ts_rs::TS;

use crate::cli::{self, OpProgress, OpRequest, StreamLine};
use crate::devices::show_virtual;

#[derive(Default)]
pub struct Ops {
    next: AtomicU64,
    running: Mutex<HashMap<String, (u64, ChildStdin)>>,
}

#[derive(Clone, Serialize, TS)]
#[ts(export)]
pub struct OpEvent {
    #[ts(type = "number")]
    pub id: u64,
    pub device: String,
    pub progress: OpProgress,
}

#[derive(Clone, Serialize, TS)]
#[ts(export)]
pub struct OpFinished {
    #[ts(type = "number")]
    pub id: u64,
    pub device: String,
    pub kind: String,
    pub ok: bool,
    #[ts(type = "any")]
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    /// Processos que prendiam o disco, quando a falha foi "em uso".
    pub holders: Vec<usbkeeper_core::Holder>,
}

/// CLI usada: USBKEEPER_CLI, depois `usbkeeper` ao lado do executável da GUI
/// (target/debug em desenvolvimento), depois /usr/bin/usbkeeper.
pub fn cli_path() -> PathBuf {
    if let Some(p) = std::env::var_os("USBKEEPER_CLI") {
        return PathBuf::from(p);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let sibling = dir.join("usbkeeper");
        if sibling.is_file() {
            return sibling;
        }
    }
    PathBuf::from("/usr/bin/usbkeeper")
}

#[tauri::command]
pub fn start_operation(
    app: AppHandle,
    ops: State<'_, Ops>,
    request: OpRequest,
    token: String,
) -> Result<u64, String> {
    let device = request.device().to_string();
    let mut running = ops.running.lock().map_err(|e| e.to_string())?;
    if running.contains_key(&device) {
        return Err("já existe uma operação em andamento neste disco".into());
    }
    let cli = cli_path();
    let args = cli::args(&request, &token, show_virtual());
    let mut cmd = if request.needs_root() {
        let mut c = Command::new("pkexec");
        c.arg(&cli);
        c
    } else {
        Command::new(&cli)
    };
    let mut child = cmd
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("não foi possível iniciar {}: {e}", cli.display()))?;
    let id = ops.next.fetch_add(1, Ordering::Relaxed) + 1;
    let stdin = child.stdin.take().expect("stdin piped");
    let stderr = child.stderr.take().expect("stderr piped");
    let mut stdout = child.stdout.take().expect("stdout piped");
    running.insert(device.clone(), (id, stdin));
    drop(running);

    let kind = request.kind().to_string();
    std::thread::spawn(move || {
        // stdout é lido em paralelo: se o resultado JSON for grande e
        // ninguém ler, o processo trava com o pipe cheio.
        let reader = std::thread::spawn(move || {
            let mut out = String::new();
            let _ = stdout.read_to_string(&mut out);
            out
        });
        let mut last_error: Option<String> = None;
        let mut last_holders: Vec<usbkeeper_core::Holder> = Vec::new();
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            match cli::parse_line(&line) {
                StreamLine::Progress(progress) => {
                    let _ = app.emit(
                        "op-progress",
                        OpEvent {
                            id,
                            device: device.clone(),
                            progress,
                        },
                    );
                }
                StreamLine::Error { message, holders } => {
                    last_error = Some(message);
                    last_holders = holders;
                }
                StreamLine::Log(_) => {}
            }
        }
        let out = reader.join().unwrap_or_default();
        let status = child.wait().ok().and_then(|s| s.code());
        if let Some(state) = app.try_state::<Ops>()
            && let Ok(mut r) = state.running.lock()
        {
            r.remove(&device);
        }
        let error = cli::exit_message(status, last_error.as_deref());
        let _ = app.emit(
            "op-finished",
            OpFinished {
                id,
                device,
                kind,
                ok: error.is_none(),
                result: serde_json::from_str(out.trim()).ok(),
                error,
                holders: last_holders,
            },
        );
    });
    Ok(id)
}

#[tauri::command]
pub fn cancel_operation(ops: State<'_, Ops>, device: String) -> Result<(), String> {
    let mut running = ops.running.lock().map_err(|e| e.to_string())?;
    match running.get_mut(&device) {
        Some((_, stdin)) => stdin
            .write_all(b"cancel\n")
            .and_then(|_| stdin.flush())
            .map_err(|e| e.to_string()),
        None => Err("nenhuma operação em andamento neste disco".into()),
    }
}

/// Gera o laudo HTML pela CLI, como usuário (o histórico é do usuário).
#[tauri::command]
pub fn save_report(key: String, path: String) -> Result<(), String> {
    let out = Command::new(cli_path())
        .args(["report", &key, "-o", &path])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr)
            .trim()
            .trim_start_matches("erro: ")
            .to_string())
    }
}
