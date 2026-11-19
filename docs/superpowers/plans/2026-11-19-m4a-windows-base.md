# M4a — Base do Windows Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** usbkeeper lista, identifica e verifica (só leitura) discos no Windows.

**Architecture:** script PowerShell → JSON → `parse` puro (testado em todo SO); I/O bruto nativo via crate `windows`; backend da CLI escolhido por `cfg`.

**Tech Stack:** Rust 1.99, crate `windows` 0.61, PowerShell 5.1 (Storage cmdlets), GitHub Actions windows-latest + diskpart.

Spec: `docs/superpowers/specs/2026-11-19-m4a-windows-base-design.md`.

## Global Constraints
- Disco do sistema nunca é alvo. Commits sem coautoria do Claude, sempre `testes && git commit`.
- Código Linux intocado no comportamento; CI Linux/macOS/Windows verde a cada task.

### Task 1: core — `Device.system_disk`
- [ ] Teste em `safety.rs`: device com `system_disk = true` e montado em `E:\` → `Refusal::SystemDisk`; Linux sem flag continua igual.
- [ ] Campo `pub system_disk: bool` (serializado, binding TS regenerado), `false` no lsblk e fixtures; `system_mountpoint` considera a flag. Commit.

### Task 2: `usbkeeper-windows::storage` (puro)
**Produces:** `parse(&str) -> Result<Vec<Device>, CoreError>`, `find(&[Device], &str) -> Option<&Device>`, `SCRIPT: &str`.
- [ ] Fixtures (pendrive USB, NVMe sistema, VHD, objeto único, nulos) e testes de mapeamento/`find`.
- [ ] Implementação. Commit.

### Task 3: enumerador, I/O bruto e admin (`cfg(windows)`)
- [ ] `WindowsEnumerator`, `RawDevice`, `is_elevated`; compila no CI Windows (clippy). Commit.

### Task 4: SMART e CLI no Windows
- [ ] smartctl por plataforma (`/dev/pdN`, caminhos de instalação), teste de nome do disco.
- [ ] `backend.rs` Windows; destrutivos → "M4b"; `bench` escrita recusada no Windows. Commit.

### Task 5: e2e Windows no CI, README
- [ ] `scripts/e2e-windows.ps1` + passo no job windows do `ci.yml`.
- [ ] README (seção Windows: o que funciona, administrador, smartmontools). PR → dev.
