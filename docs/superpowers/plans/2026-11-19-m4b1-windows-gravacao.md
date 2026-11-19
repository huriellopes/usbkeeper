# M4b1 — Windows: gravação Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** gravar no disco bruto no Windows com os volumes travados/desmontados, liberando apagar, falsificado, restaurar imagem, gravar ISO e bench com escrita.

**Architecture:** o travamento mora no `RawDevice` (handles vivos durante a gravação); funções puras de caminho/extensões testadas em todo SO; a CLI só remove a trava de plataforma.

**Tech Stack:** Rust, crate `windows` 0.61 (Ioctl, FileSystem), PowerShell no e2e.

Spec: `docs/superpowers/specs/2026-11-19-m4b1-windows-gravacao-design.md`.

## Global Constraints
- Nunca gravar sem os volumes travados; volume em uso = recusar sem gravar.
- Disco do sistema continua recusado (SafetyGuard). Commits sem coautoria, `testes && git commit`.
- Código igual em todos os SOs onde der (`cfg!` em vez de `#[cfg]` nos fluxos da CLI).

### Task 1: funções puras `disk_number` e `extents_on_disk` (+ testes)
### Task 2: `RawDevice` com travar/desmontar/atualizar e `WindowsMounts` (clippy no alvo Windows)
### Task 3: CLI — trava só em format/reparar/montar/remover (M4b2); TRIM recusado no Windows
### Task 4: e2e Windows (em uso, apagar, falsificado, bench, imagem, ISO), README, PR → dev
