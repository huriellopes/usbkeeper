# M6b — Autoteste SMART e apagar com segurança — design

Data: 2026-11-19 · Status: **aprovado no brainstorming** · Versão alvo: 0.7.0

## 1. Autoteste SMART

O próprio HD/SSD testa a si mesmo, no firmware: **curto** (~2 min, testes elétricos e uma
amostra de leitura) e **longo** (lê a superfície inteira; horas). **Não altera dados**.

### Comandos (`crates/smart`)

- Disparar: `smartctl -j -d <tipo> -t short|long <disco>`. O tipo é o mesmo que trouxe o SMART
  no `read()` (auto/sat/usbjmicron/…); sem SMART legível → erro explicando (ponte USB, pendrive).
- Acompanhar: `smartctl -j -d <tipo> -c -l selftest <disco>` a cada 10 s.
  - ATA: `ata_smart_data.self_test.status` (`value` 240–249 = em andamento;
    `remaining_percent`), `ata_smart_data.self_test.polling_minutes.{short,extended}`;
    log em `ata_smart_self_test_log.standard.table[]` (`type.string`, `status.string`,
    `status.passed`, `lifetime_hours`).
  - NVMe: `nvme_self_test_log.current_self_test_operation.value` (≠ 0 = em andamento),
    `current_self_test_completion_percent`; log em `nvme_self_test_log.table[]`
    (`self_test_code.string`, `self_test_result.value` 0 = passou, `power_on_hours`).
- Abortar: `smartctl -d <tipo> -X <disco>`.

### Tipos (core-agnóstico, em `crates/smart/src/selftest.rs`, puro + execução)

```rust
pub enum SelfTestKind { Short, Long }
pub struct SelfTestEntry { kind: String, status: String, passed: Option<bool>, hours: Option<u64> }
pub struct SelfTestState { running: bool, remaining_percent: Option<u8>,
                           polling_minutes: Option<u64>, log: Vec<SelfTestEntry> }
pub fn parse_state(json: &str) -> Result<SelfTestState, String>
```

O resultado do teste que acabou é a **primeira entrada do log** (a mais recente).

### CLI

`usbkeeper autoteste <disco> (--curto | --longo) [--esperar] [--json]` (nome `selftest`).
Sem `--esperar`: dispara e informa a duração estimada. Com `--esperar`: progresso
(`--progress-json` emite fase `selftest`) até terminar; Ctrl+C / `cancel` no stdin →
**aborta o teste no disco** e sai com erro “cancelado”. Root necessário; qualquer disco (só lê).
Histórico: `kind = "selftest"`, veredito `passed` | `failed`, resumo com tipo e resultado.

`health` passa a mostrar os **3 últimos autotestes** do log do disco (quando houver).

### Interface

Aba Saúde: “Autoteste curto (~2 min)” e “Autoteste longo (pode levar horas)”; progresso pela
barra de operação, Cancelar aborta; resultado (passou/falhou + linha do log) e tabela dos
últimos autotestes. `OpRequest::SelfTest { device, long }` → `autoteste <disco> --curto|--longo
--esperar --json --progress-json --cancel-stdin`.

## 2. Apagar com segurança

### Métodos

| Método | Como | Para quê | Garantia |
|---|---|---|---|
| Sobrescrever com zeros (padrão) | grava zeros no disco inteiro | HD; uso geral | HD: total. Flash: áreas de reserva do controlador podem manter dados antigos |
| Sobrescrever com dados aleatórios | grava bytes pseudoaleatórios | quem quer evitar padrão reconhecível | idem |
| Passadas (1–3) | repete a sobrescrita | HDs antigos; exigência de política | idem |
| Verificar | relê tudo e confere que só restou o padrão da última passada (zeros) | prova de que gravou | — |
| Descartar (TRIM) | `blkdiscard <disco>` | SSD/NVMe, muito rápido | depende do firmware; várias pontes USB recusam |

Fora do escopo: *secure erase* de firmware (ATA Security Erase, NVMe Format/Sanitize) — pode
travar ou inutilizar o disco se interrompido, e as pontes USB costumam bloquear.

### Algoritmo (`crates/core/src/wipe.rs`, sobre `BlockDevice`)

```rust
pub enum WipePattern { Zeros, Random }
pub struct WipeOptions { pattern: WipePattern, passes: u8 /* 1..=3 */, verify: bool, seed: u64 }
pub struct WipeReport { bytes: u64, passes: u8, pattern: WipePattern, verified: Option<bool>,
                        mismatched_bytes: u64, seconds: f64 }
pub fn wipe(dev, opts, progress, cancel) -> Result<WipeReport, CoreError>
```

- Blocos de 1 MiB, O_DIRECT; `flush` ao fim de cada passada.
- Todas as passadas usam o padrão escolhido. No `Random`, cada bloco de 1 MiB é gerado por um
  XorShift com semente `seed ^ passada ^ offset`, então o conteúdo de qualquer bloco pode ser
  **regenerado** sem guardar nada.
- Verificação (depois da última passada): relê tudo e compara com o padrão da última passada
  (zeros, ou a mesma sequência pseudoaleatória regenerada); `mismatched_bytes` conta as
  diferenças e `verified = mismatched_bytes == 0`.
- Fases de progresso: `Phase::Wipe` (“apagando, passada N de M”) e `Phase::Verify`.
- Cancelar: para no próximo bloco e devolve `Cancelled` (o disco fica parcialmente apagado; a
  mensagem diz isso).

### Descarte

`crates/platform-linux`: `discard(device) -> Result<(), CoreError>` via `blkdiscard -f <disco>`;
erro “não suportado” traduzido para “este dispositivo (ou a ponte USB) não aceita descarte; use
sobrescrever”.

### CLI

`usbkeeper apagar <disco> [--aleatorio] [--passadas 1..3] [--verificar] [--descartar]
[--unmount] [--allow-non-usb] [--yes-i-know SERIE] [--json]` (nome `wipe`). Mesmo fluxo de
segurança do `format` (precheck → confirmação pela série → desmontar → reconferir) e um aviso
extra: “isto apaga **tudo**, de forma irreversível”. Abre o disco com O_EXCL. Ao fim, sugere
formatar. Histórico: `kind = "wipe"`, veredito `wiped` | `verify_failed` | `discarded`.

### Interface

Aba **Apagar** (destrutiva; indisponível para o disco do sistema): escolha do método com
explicação de cada um, aviso destacado sobre flash, confirmação pela série com a lista do que
será apagado; progresso e Cancelar; resultado (e, se verificou, “verificado: só zeros”) com
botão “Formatar agora” que leva à aba Formatar. `OpRequest::Wipe { device, random, passes,
verify, discard }`.

## 3. Guia

- Tópico novo `apagar` (“Apagar com segurança”): por que formatar não apaga; HD × flash e áreas
  de reserva; zeros × aleatório × passadas (uma passada basta em HD moderno); verificação;
  descarte/TRIM; por que o usbkeeper não usa secure erase de firmware; criptografia desde o
  início como a garantia real; o que fazer antes de vender/doar. Apelidos `wipe`, `apagar-tudo`.
- Tópico `saude` ganha a seção **Autoteste** (curto × longo, o que o resultado significa).

## Testes

- `selftest`: fixtures ATA (em andamento 90%, concluído sem erro, falha de leitura) e NVMe
  (em andamento, concluído), ponte sem suporte; montagem dos comandos.
- `wipe` (FakeDrive): zeros deixam só zeros; aleatório + verify regenera e confere; passadas
  contadas no progresso; verificação acusa setor que não aceitou gravação (FakeDrive com setor
  podre); cancelar devolve `Cancelled`.
- CLI: parse de `autoteste` e `apagar` (passadas fora de 1..3 rejeitadas).
- Interface: botões de autoteste disparam o pedido certo; aba Apagar exige série, mostra o aviso
  de flash e oferece “Formatar agora”.
- e2e: `apagar --verificar` num loop cheio de dados aleatórios → `verified`, e `cmp` com
  `/dev/zero` confirma byte a byte; `apagar --aleatorio --passadas 2 --verificar`;
  `apagar --descartar` num loop (o kernel aceita discard em loop) → sucesso;
  `E2E_SELFTEST=1` roda um autoteste curto real no primeiro disco SATA/NVMe e confere o log.
