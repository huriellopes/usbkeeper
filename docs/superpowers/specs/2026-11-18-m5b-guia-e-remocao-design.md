# M5b — Guia de ajuda e remoção segura — design

Data: 2026-11-18 · Status: **aprovado no brainstorming**

## Objetivo

Quem usa o usbkeeper precisa entender o que está escolhendo antes de apagar um disco: o que é
MBR × GPT, qual sistema de arquivos usar, o que o fake-check prova, o que significa um atributo
SMART, por que arrancar o pendrive corrompe dados. A M5b adiciona:

1. **Um guia** em português, didático e com exemplos, mostrado igual na CLI e na interface.
2. **Remoção segura em destaque**: fácil de achar, bloqueada no momento errado, explicando o que
   impede a remoção e dizendo claramente quando pode tirar.

## Decisões

### 1. Conteúdo único em Markdown (crate `usbkeeper-guide`)

- Um arquivo por tópico em `crates/guide/topics/<id>.md`, embutido com `include_str!`.
- Cabeçalho do arquivo: primeira linha `# Título`, segunda linha não vazia = resumo (uma frase).
- API: `topics() -> &'static [Topic]` (`id`, `title`, `summary`, `markdown`), `find(id_ou_apelido)`,
  `to_html(md) -> String` (via `pulldown-cmark`, tabelas habilitadas), `to_terminal(md) -> String`
  (texto: títulos em maiúsculas com sublinhado, listas com "•", tabelas alinhadas, links como texto).
- O conteúdo é nosso e compilado no binário, então o HTML pode ser inserido na janela sem
  sanitização; mesmo assim o renderizador desliga HTML bruto vindo do Markdown (`Options` sem HTML
  passthrough: eventos `Html`/`InlineHtml` são descartados).

### 2. Tópicos (ids estáveis, usados pelos links "?" e pela CLI)

| id | Título | Cobre |
|---|---|---|
| `comecar` | Primeiros passos e regras de ouro | backup antes de tudo; nunca o disco do sistema; conferir a série; o que pede senha |
| `tipos` | Pendrive, cartão, HD externo, SSD e NVMe | como cada um guarda dados, durabilidade, SMART, gavetas/adaptadores |
| `usb` | USB, cabos e velocidade | 2.0 × 3.x × Gen2 × 2x2, porta azul, cabo/hub limitando, alimentação de HD externo |
| `mbr-gpt` | Tabela de partições: MBR × GPT | o que é, limites (2 TiB, 4 primárias), BIOS/UEFI, TVs, quando usar cada |
| `sistemas-de-arquivos` | FAT32, exFAT, NTFS e ext4 | tabela de compatibilidade (Windows/macOS/Linux/TV/carro/videogame), limite de 4 GB, journaling, recomendação por uso |
| `formatar` | O que formatar faz (e não faz) | apaga o índice, não os dados; formatar ≠ apagar com segurança; checklist antes |
| `falsificados` | Pendrives e cartões falsificados | como a fraude funciona (volta/descarte), sinais, rápido × completo, laudo, reembolso, `--max-size` |
| `saude` | Saúde do disco e SMART | cada atributo (realocados, pendentes, incorrigíveis, CRC, horas, vida útil NVMe), por que pendrive não tem SMART, varredura, quando trocar |
| `velocidade` | Velocidade: sequencial × 4K | MB/s × IOPS, cache SLC, o que é bom para cada uso, por que o pendrive barato trava |
| `remover` | Remover com segurança | cache de escrita, o que "em uso" significa, como achar e fechar o programa, LED |
| `protecoes` | O que o usbkeeper protege | disco do sistema, só USB, confirmação pela série, O_EXCL, restauração do teste rápido |
| `glossario` | Glossário | bloco, setor, partição, montar, sistema de arquivos, firmware, TRIM, LBA, … |

Apelidos aceitos pela CLI: `mbr`, `gpt` → `mbr-gpt`; `fat32`, `exfat`, `ntfs`, `ext4`, `fs` →
`sistemas-de-arquivos`; `fake`, `falso` → `falsificados`; `smart` → `saude`; `ejetar`, `eject` →
`remover`.

### 3. CLI

- `usbkeeper guia` lista os tópicos (id, título, resumo); `usbkeeper guia <tópico>` mostra o texto.
  `--html` imprime o HTML (útil para exportar).
- Dicas existentes apontam o tópico: recusa por montado/“em uso” → `usbkeeper guia remover`;
  veredito falso → `usbkeeper guia falsificados`; SMART em atenção/falhando → `usbkeeper guia saude`;
  `format` sem `--fs` válido e erro de rótulo → `usbkeeper guia sistemas-de-arquivos`.
- `eject` ganha o apelido `remover`.

### 4. Interface

- **Ajuda** fixa no rodapé da barra lateral → painel de ajuda no lugar do painel do disco: índice
  à esquerda (título + resumo), busca por texto, conteúdo à direita; botão Voltar ao disco.
- **Botões “?”** que abrem o tópico direto: aba Verificar (`falsificados`), Saúde (`saude`),
  Velocidade (`velocidade`), Formatar → sistema de arquivos (`sistemas-de-arquivos`), tabela
  (`mbr-gpt`), limite (`falsificados`); Remover (`remover`); Visão geral → recusas (`protecoes`).
- Comando Tauri `guide_topics() -> Vec<GuideTopic { id, title, summary, html }>` (carregado uma vez).

### 5. Remover com segurança

- “Ejetar” vira **“Remover com segurança”** no painel; na barra lateral, botão ⏏ em cada disco USB
  que não é do sistema.
- Desabilitado enquanto há operação naquele disco (tooltip explica).
- **Em uso**: em vez do texto cru do erro, um cartão “Não dá para remover ainda” com a lista de
  processos (PID, programa, arquivo) — a CLI já devolve isso; a GUI passa a pedir `--json` com os
  detentores estruturados — orientação (“feche o programa ou a janela do gerenciador de arquivos
  aberta no pendrive”) e **Tentar de novo**.
- **Sucesso**: aviso no topo da janela, fora do painel do disco (que some quando o dispositivo
  desliga): “✔ Pode tirar o *Modelo* (sdb)”. Se o desligamento não for suportado mas a
  desmontagem funcionou: “Desmontado. Pode tirar quando o LED parar de piscar.” O aviso some
  sozinho em 15 s ou ao clicar.
- CLI: com `--progress-json`, o erro final de qualquer comando que falhe por disco em uso passa a
  ser `{"error": "...", "holders": [{pid, command, path}, …]}`; a GUI chama
  `eject <disco> --json --progress-json` para receber isso estruturado. Quando o desligamento
  falha mas a desmontagem funcionou, `eject --json` devolve `"powered_off": false` e
  `"note": "<motivo>"` com sucesso (código 0).

## Testes

- Guia (Rust): todo tópico tem título e resumo não vazios e id único; todo apelido resolve; tabelas
  viram `<table>`; HTML bruto no Markdown é descartado; `to_terminal` não deixa `**`/`#`/`|`.
- Consistência: lista de ids referenciados pela GUI (arquivo `apps/gui/src/lib/guideLinks.ts`)
  checada por um teste Rust que lê o arquivo e confere que cada id existe.
- CLI: `guia` lista 12 tópicos; `guia mbr` mostra o tópico `mbr-gpt`; tópico inexistente sai com
  erro e sugere a lista.
- GUI (vitest): “?” abre o painel no tópico certo; busca filtra; botão Remover desabilitado com
  operação em andamento; cartão “em uso” mostra os processos e Tentar de novo; aviso “Pode tirar”
  aparece depois de o disco sumir da lista.
- e2e: `eject --json` em uso devolve `holders`; `guia` funciona sem root.

## Fora do escopo

Tradução para outros idiomas; aviso de desconexão sem ejetar; vídeos/imagens no guia.
