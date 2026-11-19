# M6b — Autoteste SMART e apagar com segurança Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Autoteste SMART (curto/longo) e apagar com segurança (sobrescrita com verificação, ou descarte), na CLI e na janela.

**Architecture:** Leitura do estado do autoteste pura em `crates/smart/src/selftest.rs` (fixtures); execução com `smartctl` no mesmo crate. Algoritmo de apagar em `crates/core/src/wipe.rs` sobre `BlockDevice` (testado no `FakeDrive`); descarte via `blkdiscard` no backend Linux. CLI `autoteste`/`apagar` com o fluxo de segurança existente; GUI via `OpRequest` + pkexec.

**Tech Stack:** Rust 1.99, smartctl 7.x, util-linux (blkdiscard), Tauri 2, React 19, vitest.

Spec: `docs/superpowers/specs/2026-11-19-m6b-autoteste-apagar-design.md`.

## Global Constraints

- Versão **0.7.0**. Commits sem coautoria do Claude.
- Autoteste: qualquer disco (só lê), root; cancelar = `smartctl -X` no disco.
- Apagar: mesmas travas do `format` + aviso de irreversível; O_EXCL; passadas 1..=3.
- Textos honestos sobre flash: sobrescrever não garante apagar áreas de reserva.

---

### Task 1: `crates/smart/src/selftest.rs` — estado, log e execução

**Interfaces — Produces:** `SelfTestKind { Short, Long }`; `SelfTestEntry { kind: String, status: String, passed: Option<bool>, hours: Option<u64> }` (Serialize, TS); `SelfTestState { running: bool, remaining_percent: Option<u8>, polling_minutes: Option<u64>, log: Vec<SelfTestEntry> }` (Serialize, TS); `parse_state(json: &str, kind: SelfTestKind) -> Result<SelfTestState, String>`; `start(device: &str, dtype: &str, kind) -> Result<(), String>`; `poll(device, dtype, kind) -> Result<SelfTestState, String>`; `abort(device, dtype) -> Result<(), String>`; `HealthReport.device_type` reaproveitado como `dtype`.

- [ ] **Step 1: fixtures + testes** em `crates/smart/tests/fixtures/selftest-*.json`:
  - `selftest-ata-running.json`: `{"ata_smart_data":{"self_test":{"status":{"value":249,"string":"in progress, 90% of test remaining","remaining_percent":90},"polling_minutes":{"short":2,"extended":110}}},"ata_smart_self_test_log":{"standard":{"table":[]}}}`
  - `selftest-ata-done.json`: status `{"value":0,"string":"completed without error","passed":true}` e log `[{"type":{"value":1,"string":"Short offline"},"status":{"value":0,"string":"Completed without error","passed":true},"lifetime_hours":13451}]`
  - `selftest-ata-failed.json`: log com `{"type":{"string":"Extended offline"},"status":{"value":119,"string":"Completed: read failure","passed":false},"lifetime_hours":20000}`
  - `selftest-nvme-running.json`: `{"nvme_self_test_log":{"current_self_test_operation":{"value":1,"string":"Short self-test in progress"},"current_self_test_completion_percent":30,"table":[]}}`
  - `selftest-nvme-done.json`: `{"nvme_self_test_log":{"current_self_test_operation":{"value":0},"table":[{"self_test_code":{"string":"Short"},"self_test_result":{"value":0,"string":"Completed without error"},"power_on_hours":6121}]}}`

```rust
// crates/smart/tests/selftest.rs
use usbkeeper_smart::selftest::{SelfTestKind, parse_state};

#[test]
fn ata_running_reports_remaining() {
    let s = parse_state(include_str!("fixtures/selftest-ata-running.json"), SelfTestKind::Short).unwrap();
    assert!(s.running);
    assert_eq!(s.remaining_percent, Some(90));
    assert_eq!(s.polling_minutes, Some(2));
}

#[test]
fn ata_done_and_failed_come_from_log() {
    let ok = parse_state(include_str!("fixtures/selftest-ata-done.json"), SelfTestKind::Short).unwrap();
    assert!(!ok.running);
    assert_eq!(ok.log[0].passed, Some(true));
    assert_eq!(ok.log[0].hours, Some(13451));
    let bad = parse_state(include_str!("fixtures/selftest-ata-failed.json"), SelfTestKind::Long).unwrap();
    assert_eq!(bad.log[0].passed, Some(false));
    assert!(bad.log[0].status.contains("read failure"));
}

#[test]
fn nvme_running_and_done() {
    let r = parse_state(include_str!("fixtures/selftest-nvme-running.json"), SelfTestKind::Short).unwrap();
    assert!(r.running);
    assert_eq!(r.remaining_percent, Some(70));
    let d = parse_state(include_str!("fixtures/selftest-nvme-done.json"), SelfTestKind::Short).unwrap();
    assert!(!d.running);
    assert_eq!(d.log[0].passed, Some(true));
}

#[test]
fn no_selftest_data_is_an_error() {
    assert!(parse_state("{}", SelfTestKind::Short).is_err());
}
```

- [ ] **Step 2: implementação** — `parse_state` lê ATA (`ata_smart_data.self_test.status.value` 240..=249 → `running`, `remaining_percent`; `polling_minutes.short|extended` conforme o tipo; log de `ata_smart_self_test_log.standard.table`) ou NVMe (`current_self_test_operation.value != 0` → `running`, `remaining = 100 - completion_percent`; log com `passed = self_test_result.value == 0`); sem nenhum dos dois → `Err("o disco não informou o estado do autoteste (ponte USB sem suporte?)")`. `start`/`poll`/`abort` executam `smartctl` (`-j -d <tipo> -t short|long`, `-j -d <tipo> -c -l selftest`, `-d <tipo> -X`); `start` falha com a primeira mensagem de erro do `smartctl.messages` quando `exit_status & 0b11 != 0`.
- [ ] **Step 3:** `cargo test -q -p usbkeeper-smart` ok. **Commit:** `feat(smart): estado e log do autoteste (ATA e NVMe)`

### Task 2: `crates/core/src/wipe.rs` — sobrescrita com verificação

**Interfaces — Produces:** `WipePattern { Zeros, Random }`; `WipeOptions { pattern, passes: u8, verify: bool, seed: u64 }`; `WipeReport { bytes: u64, passes: u8, pattern: WipePattern, verified: Option<bool>, mismatched_bytes: u64, seconds: f64 }` (Serialize, TS); `wipe(dev: &mut dyn BlockDevice, opts, progress, cancel) -> Result<WipeReport, CoreError>`; `Phase::Wipe` (id `"wipe"`, rótulo `"apagando"`, conta bytes).

- [ ] **Step 1: testes** (FakeDrive):

```rust
#[test]
fn zeros_leave_only_zeros_and_verify() {
    let mut d = FakeDrive::genuine(8 << 20);
    d.seed_user_data(&[0, 4096, 5 << 20]);
    let r = wipe(&mut d, WipeOptions { pattern: WipePattern::Zeros, passes: 1, verify: true, seed: 1 }, &mut Silent, &AtomicBool::new(false)).unwrap();
    assert_eq!(r.verified, Some(true));
    assert!(d.physical_snapshot().iter().all(|(_, c)| c.iter().all(|&b| b == 0)));
}

#[test]
fn random_passes_are_counted_and_verified_by_regeneration() {
    let mut d = FakeDrive::genuine(4 << 20);
    let mut phases = Vec::new();
    struct Rec<'a>(&'a mut Vec<(Phase, u64, u64)>);
    impl Progress for Rec<'_> { fn report(&mut self, p: Phase, d: u64, t: u64) { self.0.push((p, d, t)) } }
    let r = wipe(&mut d, WipeOptions { pattern: WipePattern::Random, passes: 2, verify: true, seed: 7 }, &mut Rec(&mut phases), &AtomicBool::new(false)).unwrap();
    assert_eq!(r.verified, Some(true));
    assert_eq!(phases.iter().filter(|(p, d, t)| *p == Phase::Wipe && d == t).count(), 2);
    assert!(d.physical_snapshot().iter().any(|(_, c)| c.iter().any(|&b| b != 0)), "aleatório não deixa só zeros");
}

#[test]
fn verify_catches_sector_that_does_not_store() {
    let mut d = FakeDrive::genuine(4 << 20).with_rotten_sector(1 << 20);
    let r = wipe(&mut d, WipeOptions { pattern: WipePattern::Zeros, passes: 1, verify: true, seed: 1 }, &mut Silent, &AtomicBool::new(false)).unwrap();
    assert_eq!(r.verified, Some(false));
    assert_eq!(r.mismatched_bytes, 512);
}

#[test]
fn cancel_stops() {
    let err = wipe(&mut FakeDrive::genuine(4 << 20), WipeOptions { pattern: WipePattern::Zeros, passes: 1, verify: false, seed: 1 }, &mut Silent, &AtomicBool::new(true)).unwrap_err();
    assert!(matches!(err, CoreError::Cancelled));
}

#[test]
fn passes_must_be_one_to_three() {
    assert!(wipe(&mut FakeDrive::genuine(1 << 20), WipeOptions { pattern: WipePattern::Zeros, passes: 0, verify: false, seed: 1 }, &mut Silent, &AtomicBool::new(false)).is_err());
}
```

- [ ] **Step 2: implementação** — blocos de 1 MiB (`AlignedBuf`), `fill(buf, pattern, seed ^ pass ^ offset)` (zeros ou XorShift), `flush` por passada, `Phase::Wipe` com `done/total` da passada; verificação relê, regenera o esperado da última passada e conta bytes diferentes (`Phase::Verify`). `passes` fora de 1..=3 → `CoreError::Backend("passadas devem ser de 1 a 3")`.
- [ ] **Step 3:** testes ok. **Commit:** `feat(core): apagar com sobrescrita (zeros/aleatório, passadas, verificação)`

### Task 3: descarte no Linux e CLI `autoteste` / `apagar`

**Interfaces:** `usbkeeper_linux::discard(device_path) -> Result<(), CoreError>` (`blkdiscard -f`; erro com “not supported”/“Operation not supported” → mensagem de sem suporte); `Command::SelfTest { device, curto: bool, longo: bool, esperar: bool, json }` (`name = "selftest"`, `alias = "autoteste"`, `curto`/`longo` em grupo exclusivo obrigatório); `Command::Wipe { device, aleatorio: bool, passadas: u8 (1..=3, value_parser range), verificar: bool, descartar: bool, guard, json }` (`name = "wipe"`, `alias = "apagar"`); `history::kind::{SELFTEST, WIPE}`; `health` imprime os 3 últimos autotestes (via `selftest::poll` quando há SMART).

- [ ] **Step 1: testes de parse** (`autoteste sdb --curto --esperar`; `apagar sdb --passadas 4` rejeitado; `--curto --longo` rejeitado).
- [ ] **Step 2: implementação** — `apps/cli/src/selftest.rs` (dispara, informa duração; com `--esperar`, laço de `poll` a cada 10 s reportando `Phase::SelfTest` com `100 - remaining`; cancel → `abort` + erro “cancelado: autoteste abortado no disco”; registra no histórico com o veredito da 1ª entrada do log) e `apps/cli/src/wipe.rs` (precheck → aviso irreversível → `confirm_and_take` → descarte ou `RawDevice::open(write)` + `core::wipe::wipe` com `Reporter` e `cancel_flag`; registra; sugere `usbkeeper format`). `Phase::SelfTest` (id `"selftest"`, rótulo `"autoteste"`, não conta bytes).
- [ ] **Step 3:** clippy/testes ok; smoke: `apagar nvme0n1 --allow-non-usb` recusado (sistema). **Commit:** `feat(cli): autoteste SMART e apagar com segurança`

### Task 4: Guia — `apagar` e seção Autoteste em `saude`

- [ ] Tópico `apagar` (fatos obrigatórios: `formatar`, `reserva`, `zeros`, `aleatório`, `passada`, `verifica`, `TRIM`, `criptografia`, `secure erase`), apelidos `wipe`, `apagar-tudo`; 14 tópicos; `saude` ganha seção “Autoteste” (fatos `curto`, `longo`, `autoteste`); `guideLinks.ts` `wipe: "apagar"`. **Commit:** `docs(guide): apagar com segurança e autoteste`

### Task 5: Interface

- [ ] `cli.rs`: `OpRequest::SelfTest { device, long }` → `["selftest", dev, "--longo"|"--curto", "--esperar", "--json", "--progress-json", "--cancel-stdin"]` (sem confirmação); `OpRequest::Wipe { device, random, passes, verify, discard }` → `["wipe", dev, …flags, "--unmount", "--yes-i-know", token, …]` (teste Rust de ambos).
- [ ] Aba Saúde: botões de autoteste e resultado (`SelfTestPanel`), com teste vitest.
- [ ] Aba **Apagar** (`WipeTab`): método (zeros/aleatório/descartar), passadas, verificar, aviso de flash, confirmação; resultado com “Formatar agora”; teste vitest (pedido certo, aviso presente, confirmação exigida). Aba destrutiva (indisponível no disco do sistema).
- [ ] **Commit:** `feat(gui): autoteste na aba Saúde e aba Apagar`

### Task 6: e2e, README, 0.7.0, PR

- [ ] e2e: encher o loop de aleatório; `apagar --verificar` → `verified` e `cmp -n <tamanho> $LOOP /dev/zero`; `apagar --aleatorio --passadas 2 --verificar` → `verified`; `apagar --descartar` → ok; `E2E_SELFTEST=1` → autoteste curto real no primeiro disco não-USB, log com nova entrada; guia com 14 tópicos.
- [ ] README (seções Autoteste e Apagar com segurança; roadmap M6b), versão 0.7.0, `.deb`, PR → dev com checklist manual.
