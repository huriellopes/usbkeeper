# M4c1 — plano de implementação

Spec: `docs/superpowers/specs/2026-11-21-m4c1-windows-janela-design.md`. Execução inline, na branch `feat/m4c1-windows-janela`.

## Restrições globais

- A CLI continua aplicando o `SafetyGuard`. A janela nunca dispensa essas verificações.
- Nada de `#[cfg]` que deixe código morto em outra plataforma. Prefira `cfg!()` em tempo de execução, ou módulos inteiros com `cfg`.
- `cargo clippy --workspace -- -D warnings` limpo no Linux.
- `cargo clippy -p usbkeeper-windows --target x86_64-pc-windows-gnu -- -D warnings` limpo. O restante do Windows é conferido no CI.

## Tarefa 1 — partes puras e consumo, válidas para as duas plataformas

Arquivos: `apps/gui/src-tauri/src/{ops.rs,cli.rs,devices.rs,lib.rs}`, `Cargo.toml` e a interface (`WipeTab`, formatação e assistente).

- `Throttle { every: Duration, last: Option<(String, Instant)> }`, com `fn pass(&mut self, phase: &str, now: Instant) -> bool`.
  - Passa quando a fase mudou ou quando já se passaram pelo menos `every` desde a última emissão.
  - O leitor guarda o último progresso retido e o emite antes de `op-finished`.
  - Testes: limite de 250 ms, troca de fase imediata e emissão final.
- `exit_message`: ganha a constante `UAC_DENIED = -1223`, que leva a "autorização negada".
- `allowed_mountpoint`: passa a aceitar `X:\` quando está nos pontos de montagem do disco. Testes.
- `MAX_AGE`: 120 s com `cfg!(windows)` e 30 s no Linux. Teste.
- Comando `platform() -> Platform { ext4, trim }`, exportado com `ts-rs`.
  - A interface esconde o ext4 e o TRIM quando esses campos são `false`.
  - Testes da interface com o `platform` mockado.
- `Hidden(AtomicBool)`, atualizado pelo `on_window_event` (`Resized`/`Focused`) com `is_minimized() || !is_visible()`.
  - Quando a janela está oculta, `list_devices` devolve o cache, se houver.
- `[profile.release]` com `lto = "thin"`, `codegen-units = 1` e `strip = true`.

## Tarefa 2 — `--pipe` na CLI

Arquivo: `apps/cli/src/main.rs`.

- Função pura `split_pipe_arg(args: Vec<OsString>) -> (Option<String>, Vec<OsString>)`.
  - Remove `--pipe <nonce>` logo depois do nome do programa.
  - Só aceita um nonce de 32 caracteres hex; qualquer outro valor é erro.
  - Testes.
- Somente no Windows: `pipe::attach(nonce)`.
  - Chama `CreateFileW` nos pipes `in` (leitura), `out` (escrita) e `err` (escrita) e depois `SetStdHandle`.
  - Se algum falhar, a CLI sai com o código 3.
- Em outras plataformas, `--pipe` termina com erro.

## Tarefa 3 — a janela no Windows

Arquivos: `apps/gui/src-tauri/src/{elevate.rs,winwatch.rs,ops.rs,devices.rs,lib.rs}` e `Cargo.toml`.

- `elevate.rs`:
  - Partes puras: `pipe_name(nonce, which)`, `sddl(sid)`, `nonce_hex(bytes)` e `quote_args(args)` (regras do `CommandLineToArgvW`). Testes.
  - `#[cfg(windows)] fn spawn_elevated(cli, args) -> Result<Spawned, String>`, que cobre:
    - o `BCryptGenRandom` para o nonce e o SID do token;
    - a criação dos pipes com o SDDL;
    - o `ShellExecuteExW` com `runas`;
    - o `ConnectNamedPipe` com overlapped e prazo de 60 s;
    - a conferência do PID;
    - o `wait` com `GetExitCodeProcess`.
- `Spawned`: é a origem de stdin, stdout, stderr e `wait`.
  - No Linux, e em operações sem privilégio, vem de `Command`.
  - O leitor de `ops.rs` passa a consumir só `Spawned`.
- `winwatch.rs`, só no Windows:
  - registra `CM_Register_Notification` para disco e volume;
  - o callback incrementa `GENERATION`;
  - a `signature()` do Windows usa esse contador e `GetLogicalDrives`.
- `devices.rs`:
  - `enumerate`, `mount_device` e `open_folder` (`explorer.exe`) passam a ter versão Windows;
  - `cli_path` usa `usbkeeper.exe`.
- WebView2: `with_webview` e `ICoreWebView2_19::SetMemoryUsageTargetLevel`, chamados na transição do `Hidden`.

## Tarefa 4 — CI e e2e

Arquivos: `.github/workflows/ci.yml` e `apps/gui/src-tauri/tests/elevated.rs`.

- No job do Windows: `npm ci`, `npm run build`, `cargo clippy -p usbkeeper-gui` e `cargo test -p usbkeeper-gui`.
- `elevated_roundtrip`, marcado com `#[ignore]` e só no Windows. Roda no passo do e2e com `--ignored` e cobre:
  - `list --json` pelo caminho `runas`;
  - um `bench` no VHD com progresso e cancelamento.
