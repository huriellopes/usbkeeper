# M5 — Interface gráfica (Tauri 2) — design

Data: 2026-11-18 · Status: **proposto, aguardando aprovação**

## Objetivo

Dar ao usbkeeper uma janela para quem não usa terminal: ver os discos ao vivo (plugou, apareceu),
entender a situação de cada um num relance (sistema? montado? USB 2 limitando? falsificado da
última vez?) e executar os testes e a formatação com confirmação e progresso visíveis. Tudo o que a
CLI faz hoje, com a mesma segurança, sem duplicar a lógica.

Fora do escopo da M5: bandeja/notificação ao plugar, Windows/macOS, gravar ISO.

## Decisões

### 1. A GUI nunca roda como root; operações privilegiadas executam a própria CLI via `pkexec`

| Operação | Como roda |
|---|---|
| listar, detalhes, processos que usam o disco, histórico | direto, como usuário (crates `core`/`linux`/`history`) |
| fake-check, bench, health, format, eject* | `pkexec /usr/bin/usbkeeper <cmd> … --json --progress-json --cancel-stdin` |

\* `eject` funciona sem root via udisks; só cai no pkexec se o udisks recusar.

Por que a CLI e não um helper novo: **todas as travas já estão nela e testadas** (SafetyGuard,
confirmação pela série, reconferência de identidade, O_EXCL, restauração no Ctrl+C). A GUI pede a
confirmação na tela e passa `--yes-i-know <série>`, mas a CLI confere tudo de novo, então um bug na
GUI não consegue formatar o disco errado.

Polkit: uma action `dev.usbkeeper.cli` com `exec.path=/usr/bin/usbkeeper` e **`auth_admin_keep`**:
pede a senha uma vez e não pergunta de novo por 5 minutos (testar e depois formatar o mesmo pendrive
não pede senha duas vezes). Em desenvolvimento usa `target/debug/usbkeeper` e a action genérica do
pkexec, que pede a senha toda vez.

### 2. Novos modos da CLI para ser dirigida por programa

- `--progress-json`: em vez da barra, uma linha JSON por atualização no stderr
  (`{"phase":"probe","label":"gravando blocos de teste","done":12,"total":129,"bytes":false}`).
- `--cancel-stdin`: uma thread lê o stdin; a linha `cancel` liga a mesma flag do Ctrl+C. Necessário
  porque o usuário não pode mandar sinal a um processo root. Fim do stdin é ignorado.
- Histórico com `pkexec`: hoje só `SUDO_USER` é reconhecido; passa a aceitar também `PKEXEC_UID`
  (home pelo `/etc/passwd`), senão o histórico iria para `/root`.

### 3. Atualização da lista

Consulta `lsblk` a cada 2 s **só com a janela visível** (custo medido de poucos ms). Simples e
suficiente; trocar por netlink/udev só se o orçamento pedir.

### 4. Telas

Janela 1100×720, barra lateral + painel, tema claro/escuro do sistema, pt-BR.

- **Barra lateral: discos.** USB primeiro; cada item com modelo, tamanho, selo de velocidade USB
  (`USB 2.0 ⚠` quando um USB 3 negociou USB 2) e o último veredito do histórico (`FALSO`,
  `genuíno`, `atenção`). Disco do sistema aparece apagado com cadeado e não abre ações destrutivas.
  Sem nenhum USB: estado vazio “Conecte um pendrive ou HD externo” (no WSL, a dica do usbipd).
- **Painel do disco**, com abas:
  - *Visão geral*: identificação, partições, link USB, quem está usando o disco, por que (não) pode
    ser alvo — a mesma lista de recusas da CLI, em frases.
  - *Verificar*: fake-check rápido (padrão) ou completo, com o aviso do que cada um faz; resultado
    em cartão grande (capacidade real × anunciada, barra proporcional) e, se falso, botão
    “Reformatar só a parte real” que abre a aba Formatar já com `--max-size` preenchido.
  - *Saúde*: SMART (veredito + motivos + tabela de atributos recolhível) e botão “Varrer superfície”.
  - *Velocidade*: leitura (padrão) e escrita (opcional, destrutiva); números grandes + teto do link.
  - *Formatar*: sistema de arquivos com a dica de cada um, rótulo validado ao digitar (mesmas
    regras do core), tabela, limite de tamanho; prévia do que será apagado.
  - *Histórico*: linha do tempo + “Salvar laudo HTML…” (diálogo de salvar, gerado pela CLI sem root).
- **Operação em andamento**: faixa no topo do painel com etapa, barra, velocidade/ETA e
  **Cancelar**; trocar de disco não interrompe. Uma operação por disco por vez.
- **Confirmação destrutiva**: diálogo que lista o que será apagado e exige digitar a série (ou o
  caminho) — mesmo token da CLI; o botão só habilita quando confere.

### 5. Estrutura

```
apps/gui/              # React 19 + TS + Vite + Tailwind 4 + zustand (padrão do Vigia)
  src/                 # componentes, store, bindings gerados
  src-tauri/           # crate usbkeeper-gui (membro do workspace)
packaging/             # dev.usbkeeper.policy, usbkeeper.desktop, ícone
```

Tipos Rust → TS com `ts-rs` (como no Vigia), para a GUI e o core não divergirem.

Comandos Tauri: `list_devices`, `device_detail`, `history_devices`, `history_events`,
`start_operation(op)` → emite eventos `op-progress` / `op-finished` (com o JSON da CLI),
`cancel_operation(id)`, `save_report(key, path)`.

### 6. Empacotamento

`.deb` com `/usr/bin/usbkeeper` (CLI), `/usr/bin/usbkeeper-gui`, a policy em
`/usr/share/polkit-1/actions/` e o `.desktop`. Depends: `pkexec`, `util-linux`, `udisks2`;
Recommends: `smartmontools`, `dosfstools`, `exfatprogs`, `ntfs-3g`, `e2fsprogs`.

### 7. Orçamento

Mesma régua do Vigia: < 250 MB de PSS com a janela aberta, < 1% de CPU parado; compositing do
WebKit desligado por padrão.

## Testes

- Rust: `--progress-json` (formato das linhas), `--cancel-stdin` (cancela e, no fake-check rápido,
  restaura), resolução de `PKEXEC_UID`, runner que parseia o stream da CLI.
- Frontend (vitest + Testing Library, `invoke` simulado): confirmação só habilita com a série
  certa; disco do sistema sem ações destrutivas; validação do rótulo; “Reformatar só a parte real”
  preenche o limite.
- e2e existente continua cobrindo a CLI; o fluxo `pkexec` é testado manualmente por você numa lista
  de verificação no PR (loop + falso do device-mapper, pela GUI), mais a medição de orçamento.

## Entregas (um PR)

1. Modos `--progress-json` / `--cancel-stdin` e `PKEXEC_UID` na CLI.
2. Esqueleto Tauri + lista ao vivo + Visão geral (somente leitura).
3. Runner pkexec + faixa de progresso + cancelar.
4. Abas Verificar, Saúde, Velocidade, Formatar (com confirmação) e Histórico/laudo.
5. Empacotamento `.deb` + policy + medição de orçamento.
