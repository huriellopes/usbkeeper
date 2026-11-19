# M6c — Imagem do disco e gravar ISO Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Backup/restauração do disco inteiro (zstd + SHA-256) e gravação de ISO de boot com verificação, na CLI e na janela.

**Architecture:** Motor de cópia com hash no core (`image.rs`, testado no FakeDrive) e inspeção de ISO pura (`iso.rs`); compressão (zstd) e metadados na CLI; GUI via `OpRequest` + pkexec e diálogos de arquivo.

**Tech Stack:** Rust 1.99, sha2 0.10, zstd 0.13, Tauri 2 (+ dialog open), React, vitest; xorriso no e2e.

Spec: `docs/superpowers/specs/2026-11-19-m6c-imagem-iso-design.md`.

## Global Constraints

- Versão 0.8.0. Commits sem coautoria do Claude, sempre `testes && git commit`.
- Restaurar/gravar: mesmas travas do format; criar imagem: só leitura, sem série.
- Arquivos criados como root têm a posse devolvida a quem chamou.
- ISO do Windows: recusada salvo `--forcar`, com explicação.

### Task 1: core — `image.rs` (cópia + hash) e `iso.rs` (inspeção)

**Produces:** `CopyReport { bytes, sha256, seconds }`; `device_to_writer(dev: &mut dyn BlockDevice, len: u64, out: &mut dyn Write, phase: Phase, progress, cancel) -> Result<CopyReport, CoreError>`; `reader_to_device(src: &mut dyn Read, dev, progress, cancel, total_hint: u64) -> Result<CopyReport, CoreError>`; `hash_device(dev, len, progress, cancel) -> Result<String, CoreError>`; `Phase::Image`; `iso::IsoInfo { is_iso, hybrid, volume_id, windows }`, `iso::inspect(head: &[u8]) -> IsoInfo`, `iso::HEAD_LEN = 34_816`.

- [ ] Testes (FakeDrive): ida e volta disco→Vec→disco com hash igual; `len` = 3 MiB + 1000 (não múltiplo de 512) preservado e hash só dos bytes reais; fonte maior que o disco → erro antes de gravar além do fim; cancelar → `Cancelled`; `inspect`: zeros → nada; `55 AA` em 510 → hybrid; `CD001` em 32769 + volume `UBUNTU 24_04` → iso com volume; volume `CCCOMA_X64FRE_PT-BR_DV9` → windows.
- [ ] Implementação conforme a spec (sha2 `Sha256`, hex manual). Commit: `feat(core): motor de cópia com SHA-256 e inspeção de ISO`.

### Task 2: CLI — `imagem criar/verificar/restaurar` e `gravar-iso`

**Produces:** `Command::Image { action: ImageAction }` (`name = "image"`, `alias = "imagem"`), `ImageAction::{Create { device, output, sem_compressao, sobrescrever, json }, Verify { image, json }, Restore { image, device, guard, json }}` (apelidos `criar`, `verificar`, `restaurar`); `Command::BurnIso { iso, device, sha256: Option<String>, forcar, guard, json }` (`name = "burn-iso"`, `alias = "gravar-iso"`); `apps/cli/src/image.rs` com `ImageMeta { format: u8, model, serial, size_bytes, sha256, compressed, created_at, usbkeeper_version }`, `is_zstd(path) -> bool`, `expected_sha256(iso: &Path, given: Option<&str>) -> Result<Option<String>>`; history kinds `image_create`, `image_restore`, `iso_write`.

- [ ] Testes: parse dos subcomandos; `expected_sha256` lê `<iso>.sha256` e `SHA256SUMS`; ida e volta criar→verificar→restaurar usando **arquivos comuns** (RawDevice) com e sem compressão (funções internas `create_from`, `restore_into` que recebem `&mut dyn BlockDevice`).
- [ ] Implementação; `store::give_back` nos arquivos criados. Commit: `feat(cli): imagem do disco (criar/verificar/restaurar) e gravar ISO com verificação`.

### Task 3: Guia — tópico `imagem` (15 tópicos)

- [ ] Fatos obrigatórios: `backup`, `zstd`, `hash`, `SHA-256`, `restaurar`, `híbrida`, `Windows`, `tamanho`. Apelidos `iso`, `backup`, `image`. Commit: `docs(guide): imagem do disco e ISO de boot`.

### Task 4: Interface — aba "Backup e ISO"

- [ ] `OpRequest::{ImageCreate { device, output }, ImageRestore { device, image }, BurnIso { device, iso, sha256 }}` com testes de argumentos (restaurar/gravar com série; criar sem); `ImageTab` com as três seções, diálogos (`save`/`open`), campo de SHA-256, resultado; permissão `dialog:allow-open`; aba destrutiva; vitest. Commit: `feat(gui): aba Backup e ISO`.

### Task 5: e2e, README, 0.8.0, PR

- [ ] e2e: dados aleatórios no loop → `imagem criar` → `apagar` → `imagem restaurar` → `cmp` com o original salvo; `imagem verificar` ok; `xorriso -as mkisofs -o x.iso -V TESTE -isohybrid-mbr …` (ou ISO + `55 AA` escritos à mão se o isohybrid não estiver disponível) → `gravar-iso --sha256 <certo>` → `cmp -n`; hash errado recusado; ISO com volume `CCCOMA_X64FRE` recusada sem `--forcar`; guia com 15 tópicos.
- [ ] README, versão 0.8.0, `.deb`, PR → dev.
