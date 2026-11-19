# M6a — Tema, assistente de formatação e verificar/reparar — design

Data: 2026-11-19 · Status: **aprovado no brainstorming**

Parte de uma rodada dividida em três etapas: **M6a** (este documento), **M6b** (autoteste SMART e
apagar com segurança), **M6c** (imagem do disco e gravar ISO). Cada uma tem spec, plano e PR próprios.

## 1. Tema: Sistema / Claro / Escuro

- Seletor segmentado de três posições no rodapé da barra lateral, acima de “Ajuda e guia”.
- Preferência em `localStorage` (`usbkeeper.theme` = `system` | `light` | `dark`), lida com
  try/catch; sem acesso, vale `system`.
- Aplicação: atributo `data-theme` no `<html>` (`light`/`dark`); em `system` o atributo é removido.
  Um script inline no `index.html` aplica a preferência **antes** do React, para não piscar.
- CSS: os tokens de `:root` continuam valendo para o claro; o escuro é definido em
  `:root[data-theme="dark"]` e em `@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) }`.

## 2. Assistente de formatação

### Regras (Rust, `crates/core/src/advice.rs`, puro)

Entradas:

```rust
pub enum Usage { Mixed, WindowsOnly, LinuxOnly, Appliance, Console }
pub enum BigFiles { Yes, No, Unknown }
pub struct DiskFacts {
    pub size_bytes: u64,
    pub removable: bool,         // pendrive/cartão × HD/SSD
    pub fake_real_bytes: Option<u64>,      // último fake-check deu falso
    pub suggested_max_bytes: Option<u64>,
    pub health: Option<String>,  // "good" | "warning" | "failing" | "unknown"
    pub usb_bottleneck: bool,
}
```

Saída: `Advice { filesystem: Filesystem, table: PartitionTable, max_bytes: Option<u64>, reasons: Vec<String>, warnings: Vec<String> }`.

Regras, nesta ordem:

1. **Sistema de arquivos por uso:**
   - `Appliance` (TV/som/carro/aparelho antigo): **FAT32**. Se `BigFiles::Yes`: continua FAT32 e
     avisa que arquivos acima de 4 GB não cabem e que só alguns aparelhos leem exFAT.
   - `Mixed` (Windows + macOS + Linux): **exFAT**.
   - `WindowsOnly`: **NTFS** se não removível (HD/SSD); **exFAT** se removível (pendrive/cartão:
     NTFS é mais lento e gasta mais a flash com o journaling).
   - `LinuxOnly`: **ext4**.
   - `Console`: **exFAT** (motivo: consoles atuais leem exFAT; os antigos podem exigir FAT32).
2. **Falso:** se `fake_real_bytes` e `suggested_max_bytes` existem → `max_bytes = suggested_max_bytes`,
   tabela **MBR**, motivo explicando, aviso para não guardar a única cópia de nada nele.
3. **Tabela:** sem falso, **GPT** se `size_bytes > 2 TiB`, senão **MBR**. Com GPT e `Appliance`,
   avisa que muitos aparelhos não leem discos acima de 2 TB.
4. **Avisos adicionais:** `health == "failing"` → “o SMART indica falha iminente: não guarde nada
   importante”; `"warning"` → “sinais de desgaste: mantenha backup”; `usb_bottleneck` → “está
   conectado como USB 2.0; troque de porta para ficar mais rápido”.
5. **Motivos** sempre em frases curtas que citam o uso escolhido e o efeito (ex.: “exFAT funciona
   em Windows, macOS e Linux e aceita arquivos acima de 4 GB”).

### CLI

`usbkeeper recomendar <disco> --uso <misto|windows|linux|aparelho|videogame> [--arquivos-grandes <sim|nao|nao-sei>] [--json]`
(nome em inglês `advise`, apelido `recomendar`). Lê os fatos do disco (lsblk/sysfs) e do
histórico (último fake-check e última saúde). Saída: recomendação, motivos, avisos e o comando
`usbkeeper format …` pronto.

### Interface

- Botão **“Me ajude a escolher”** no topo da aba Formatar → cartão com as duas perguntas (botões
  de opção) → comando Tauri `advise(device, usage, big_files)` → mostra recomendação, motivos e
  avisos → **“Usar esta configuração”** preenche o formulário (sistema de arquivos, tabela, limite).
- O `FormatTab` passa a aceitar valores iniciais (`initial: { filesystem, table, maxBytes }`).

## 3. Verificar e reparar

### Plano de comandos (Linux, `crates/platform-linux/src/repair.rs`)

Por partição com sistema de arquivos suportado; sem partições, o próprio disco se tiver `fstype`.

| fstype (lsblk) | Verificar (não altera) | Reparar |
|---|---|---|
| `vfat` | `fsck.vfat -n <part>` | `fsck.vfat -a -w <part>` |
| `exfat` | `fsck.exfat -n <part>` | `fsck.exfat -y <part>` |
| `ntfs` | `ntfsfix -n <part>` | `ntfsfix -d <part>` |
| `ext2`/`ext3`/`ext4` | `e2fsck -f -n <part>` | `e2fsck -f -p <part>` |
| outros | — (não suportado) | — |

Todas executadas com `LC_ALL=C`. Ferramenta ausente → `MissingTool` antes de começar.

### Interpretação

`RepairStatus` por partição: `Clean`, `Fixed`, `ErrorsRemain`, `Unsupported`, `Failed`.

Os códigos de saída **não** são iguais entre as ferramentas, então cada uma tem sua tabela:

| Ferramenta | Sem problemas | Problemas (verificar) / alterou (reparar) | Erros restantes | Falha da ferramenta |
|---|---|---|---|---|
| `e2fsck` | 0 | 1, 2 | 4 | 8 ou mais |
| `fsck.exfat` | 0 | 1, 2 | 4 | 8 ou mais |
| `fsck.vfat` | 0 | 1 | — | 2 (erro de uso) ou mais |
| `ntfsfix` | 0 | — | ≠ 0 | — |

- **Verificar:** “sem problemas” → `Clean`; “problemas” ou “erros restantes” → `ErrorsRemain`;
  falha → `Failed`.
- **Reparar** sempre termina com **uma nova verificação** da mesma partição: se ela sair limpa e o
  reparo tiver alterado algo → `Fixed`; limpa sem alteração → `Clean`; senão → `ErrorsRemain`
  (ou `Failed` se a ferramenta falhou). Assim o status final não depende só do código do reparo.
- NTFS com `ErrorsRemain`: nota recomendando `chkdsk /f` no Windows (o `ntfsfix` só corrige o básico).
- O relatório guarda as últimas 30 linhas da saída de cada ferramenta.

Tabela de partições: `sfdisk --verify <disco>`; saída não vazia além do cabeçalho → aviso
“a tabela tem inconsistências” (sem reparo automático).

### Fluxo e segurança

- CLI: `usbkeeper repair <disco> [--check] [--unmount] [--allow-non-usb] [--yes-i-know SERIE] [--json]`,
  apelido `reparar`.
- `--check` não exige confirmação pela série, mas exige root e disco desmontado (`--unmount`).
- Reparar passa por `precheck` + `confirm_and_take` (mesmas travas do formatar). Disco do sistema
  é recusado mesmo para `--check` (fsck em sistema montado não é confiável e o usuário não deve
  mexer nisso pelo usbkeeper).
- Resultado registrado no histórico (`kind = "repair"`, veredito `clean` | `fixed` | `errors`).
- Com `--progress-json`: uma linha `{"phase":"repair","label":"verificando /dev/sdb1",...}` por partição.

### Interface

Aba **Reparar** (destrutiva para o disco do sistema, como Verificar/Formatar):
- “Verificar (não altera nada)” e “Reparar” (com confirmação pela série e aviso de que arquivos
  danificados podem ir para pastas `FSCK*.REC`/`lost+found`).
- Resultado por partição com selo e o log recolhível; aviso da tabela quando houver.
- “?” para o novo tópico do guia `reparar`.

### Guia

Novo tópico `reparar` (13º): o que é inconsistência de sistema de arquivos, verificar × reparar,
o que cada status significa, `FSCK*.REC`/`lost+found`, quando reparar não basta (defeito físico →
ver Saúde; NTFS → `chkdsk /f`), e sempre fazer backup antes de reparar. Apelidos: `fsck`, `repair`.

## Testes

- `advice`: tabela de casos (aparelho 64 GB → FAT32/MBR; misto → exFAT; Windows HD → NTFS;
  Windows pendrive → exFAT; Linux → ext4; 3 TB → GPT; 3 TB + aparelho → aviso; falso → limite +
  MBR + aviso; failing → aviso; USB 2 → aviso).
- `repair`: plano de comandos por fstype; não suportado; interpretação de códigos por ferramenta e
  modo; ferramenta ausente aborta antes.
- CLI: parse de `advise/recomendar` e `repair/reparar`.
- Interface: tema persiste e aplica `data-theme`; assistente preenche o Formatar; Reparar exige
  a série e não aparece habilitado para disco do sistema.
- e2e: FAT32 e ext4 em loop **corrompidos de propósito** → verificar acusa, reparar corrige,
  nova verificação limpa; `recomendar` para TV num loop de 600 MB → FAT32/MBR.
- Guia: 13 tópicos; fatos obrigatórios do `reparar`.

## Fora do escopo

Reparar tabela de partições; recuperação de partições apagadas; HFS+/APFS/btrfs/xfs.
