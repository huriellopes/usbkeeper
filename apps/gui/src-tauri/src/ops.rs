//! Execução das operações: a CLI com privilégio (`pkexec` no Linux, UAC no
//! Windows), eventos de progresso e cancelamento pela linha `cancel`.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use ts_rs::TS;

use crate::cli::{self, OpProgress, OpRequest, StreamLine};
use crate::devices::show_virtual;
use crate::spawn;
use crate::throttle::{self, Throttle};

/// Canal de cancelamento; `None` enquanto a CLI ainda não começou (UAC).
type Cancel = Option<Box<dyn Write + Send>>;

#[derive(Default)]
pub struct Ops {
    next: AtomicU64,
    running: Mutex<HashMap<String, (u64, Cancel)>>,
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

/// CLI usada: USBKEEPER_CLI, depois `usbkeeper` (`usbkeeper.exe`) ao lado do
/// executável da GUI (target/debug em desenvolvimento, pasta do instalador no
/// Windows), depois /usr/bin/usbkeeper.
pub fn cli_path() -> PathBuf {
    if let Some(p) = std::env::var_os("USBKEEPER_CLI") {
        return PathBuf::from(p);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let sibling = dir.join(format!("usbkeeper{}", std::env::consts::EXE_SUFFIX));
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
    let id = ops.next.fetch_add(1, Ordering::Relaxed) + 1;
    // Reserva o disco já; o canal de cancelamento chega depois de iniciar
    // (no Windows, só depois de o usuário responder ao UAC).
    running.insert(device.clone(), (id, None));
    drop(running);

    let cli = cli_path();
    let args = cli::args(&request, &token, show_virtual());
    let elevated = request.needs_root();
    let kind = request.kind().to_string();
    std::thread::spawn(move || {
        let outcome = spawn::spawn(&cli, &args, elevated).map(|p| {
            if let Some(state) = app.try_state::<Ops>()
                && let Ok(mut r) = state.running.lock()
            {
                r.insert(device.clone(), (id, Some(p.stdin)));
            }
            follow(&app, id, &device, p.stdout, p.stderr, p.wait)
        });
        if let Some(state) = app.try_state::<Ops>()
            && let Ok(mut r) = state.running.lock()
        {
            r.remove(&device);
        }
        let (result, error, holders) = match outcome {
            Ok(f) => f,
            Err(e) => (None, Some(e), Vec::new()),
        };
        let _ = app.emit(
            "op-finished",
            OpFinished {
                id,
                device,
                kind,
                ok: error.is_none(),
                result,
                error,
                holders,
            },
        );
    });
    Ok(id)
}

type Followed = (
    Option<serde_json::Value>,
    Option<String>,
    Vec<usbkeeper_core::Holder>,
);

/// Lê o progresso (com limite de taxa) até a CLI terminar.
fn follow(
    app: &AppHandle,
    id: u64,
    device: &str,
    mut stdout: Box<dyn Read + Send>,
    stderr: Box<dyn Read + Send>,
    wait: Box<dyn FnOnce() -> Option<i32> + Send>,
) -> Followed {
    // stdout é lido em paralelo: se o resultado JSON for grande e
    // ninguém ler, o processo trava com o pipe cheio.
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        let _ = stdout.read_to_string(&mut out);
        out
    });
    let emit = |progress: OpProgress| {
        let _ = app.emit(
            "op-progress",
            OpEvent {
                id,
                device: device.to_string(),
                progress,
            },
        );
    };
    let mut throttle = Throttle::new(throttle::EVERY);
    let mut held: Option<OpProgress> = None;
    let mut last_error: Option<String> = None;
    let mut last_holders: Vec<usbkeeper_core::Holder> = Vec::new();
    for line in BufReader::new(stderr).lines().map_while(Result::ok) {
        match cli::parse_line(&line) {
            StreamLine::Progress(progress) => {
                if throttle.pass(&progress.phase, Instant::now()) {
                    held = None;
                    emit(progress);
                } else {
                    held = Some(progress);
                }
            }
            StreamLine::Error { message, holders } => {
                last_error = Some(message);
                last_holders = holders;
            }
            StreamLine::Log(_) => {}
        }
    }
    // O último progresso retido sempre chega (ex.: 100%).
    if let Some(p) = held {
        emit(p);
    }
    let out = reader.join().unwrap_or_default();
    let status = wait();
    let error = cli::exit_message(status, last_error.as_deref());
    (serde_json::from_str(out.trim()).ok(), error, last_holders)
}

#[tauri::command]
pub fn cancel_operation(ops: State<'_, Ops>, device: String) -> Result<(), String> {
    let mut running = ops.running.lock().map_err(|e| e.to_string())?;
    match running.get_mut(&device) {
        Some((_, Some(stdin))) => stdin
            .write_all(b"cancel\n")
            .and_then(|_| stdin.flush())
            .map_err(|e| e.to_string()),
        Some((_, None)) => Err("a operação ainda está aguardando autorização".into()),
        None => Err("nenhuma operação em andamento neste disco".into()),
    }
}

/// Gera o laudo HTML pela CLI, como usuário (o histórico é do usuário).
#[tauri::command]
pub fn save_report(key: String, path: String) -> Result<(), String> {
    let out = spawn::quiet(&mut Command::new(cli_path()))
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
