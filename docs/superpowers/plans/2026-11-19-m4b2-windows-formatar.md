# M4b2 — Windows: formatar, reparar, montar, remover — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** paridade da CLI do Windows com a do Linux (menos ext4/TRIM), incluindo FAT32 > 32 GB.

**Architecture:** formatador FAT32 puro no núcleo (testado no Linux com fsck.fat); planos PowerShell no `WindowsFormatter`; chkdsk no `WindowsRepairer`; cfgmgr32 para remover.

**Tech Stack:** Rust, crate `windows` (+ DeviceAndDriverInstallation), PowerShell Storage, chkdsk, dosfstools nos testes.

Spec: `docs/superpowers/specs/2026-11-19-m4b2-windows-formatar-design.md`.

## Global Constraints
- Nunca gravar sem os volumes travados (M4b1). Plano visível no `--dry-run`.
- Commits sem coautoria, `testes && git commit`; `cfg!` nos fluxos da CLI.

### Task 1: núcleo `fat32.rs` (layout + format) com testes e fsck.fat
### Task 2: núcleo `Tool::Chkdsk`; `WindowsRepairer`
### Task 3: `WindowsFormatter` + subcomando oculto `__fat32`
### Task 4: montar (letra) e remover (cfgmgr32); tirar bloqueios M4b2
### Task 5: e2e Windows (formatos, FAT32 40 GB, max-size, reparar, montar, remover), README, PR
