//! A CLI pelo caminho da janela: UAC (`runas`) + canais privados. Roda no
//! e2e do Windows (no runner o usuário é administrador e o UAC não pergunta):
//!   USBKEEPER_E2E_BIN=…\usbkeeper.exe USBKEEPER_E2E_DISK=PhysicalDrive3 \
//!   cargo test -p usbkeeper-windows --test elevated -- --ignored
#![cfg(windows)]

use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use usbkeeper_windows::pipes::run_elevated;

fn bin() -> PathBuf {
    PathBuf::from(std::env::var("USBKEEPER_E2E_BIN").expect("USBKEEPER_E2E_BIN"))
}

fn disk() -> String {
    std::env::var("USBKEEPER_E2E_DISK").expect("USBKEEPER_E2E_DISK")
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

#[test]
#[ignore]
fn list_json_arrives_through_pipes() {
    let mut e = run_elevated(&bin(), &s(&["list", "--all", "--json"])).expect("run_elevated");
    let mut out = String::new();
    e.stdout.read_to_string(&mut out).unwrap();
    let code = e.process.wait();
    assert_eq!(code, Some(0), "saída: {out}");
    let v: serde_json::Value = serde_json::from_str(out.trim()).expect("JSON da lista");
    let name = disk();
    assert!(
        v.as_array()
            .unwrap()
            .iter()
            .any(|d| d["name"] == name.as_str()),
        "{name} não está na lista: {out}"
    );
}

#[test]
#[ignore]
fn progress_and_cancel_through_pipes() {
    let d = disk();
    let token = format!(r"\\.\{d}");
    let args = s(&[
        "fake-check",
        &d,
        "--full",
        "--allow-non-usb",
        "--unmount",
        "--yes-i-know",
        &token,
        "--progress-json",
        "--cancel-stdin",
        "--json",
    ]);
    let e = run_elevated(&bin(), &args).expect("run_elevated");
    let mut stdin = e.stdin;
    let mut stdout = e.stdout;
    let out = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stdout.read_to_string(&mut s);
        s
    });
    let started = Instant::now();
    let mut progress = 0;
    let mut lines = Vec::new();
    for line in BufReader::new(e.stderr).lines().map_while(Result::ok) {
        if line.contains("\"progress\"") || line.contains("\"phase\"") {
            progress += 1;
            if progress == 1 {
                stdin.write_all(b"cancel\n").unwrap();
                stdin.flush().unwrap();
            }
        }
        lines.push(line);
    }
    let code = e.process.wait();
    let _ = out.join();
    assert!(progress >= 1, "nenhum progresso chegou: {lines:?}");
    assert_ne!(code, Some(0), "deveria ter sido cancelado: {lines:?}");
    assert!(
        lines.iter().any(|l| l.contains("cancelad")),
        "sem mensagem de cancelamento: {lines:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(120));
}
