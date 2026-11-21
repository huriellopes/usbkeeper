# M4b2 — Windows: formatar (inclusive FAT32 > 32 GB), reparar, montar e remover

Terceira entrega do Windows (M4a ✔ → M4b1 gravação ✔ → **M4b2** → M4c janela + .msi + 0.9.0).
Ao fim, a CLI do Windows faz tudo o que a do Linux faz, exceto ext4 e TRIM.

## 1. FAT32 próprio no núcleo (`crates/core/src/fat32.rs`)

O Windows (`Format-Volume`, `format.com`) recusa FAT32 acima de 32 GB. O núcleo ganha um
formatador FAT32 puro, sobre `BlockDevice`, testável no Linux:

- `Fat32Layout::compute(len_bytes, sector) -> Result<Fat32Layout, String>`: cluster por tamanho
  (tabela da Microsoft: ≤ 8 GiB → 4 KiB, ≤ 16 GiB → 8 KiB, ≤ 32 GiB → 16 KiB, acima → 32 KiB),
  32 setores reservados, 2 FATs, tamanho da FAT pela fórmula da especificação, recusa volumes
  com menos de 65 525 clusters (seria FAT16) ou acima de 2 TiB.
- `format(dev, offset, len, label, volume_id, progress, cancel) -> Result<Fat32Layout, CoreError>`:
  setor de boot (com `hidden_sectors = offset / setor`), FSInfo, cópias de backup nos setores 6
  e 7, as duas FATs zeradas com as entradas 0, 1 e 2 (raiz = cluster 2, fim de cadeia), cluster
  da raiz zerado com a entrada de rótulo do volume. Progresso na fase `Phase::Write`.
- Rótulo: `normalize_label(Fat32, …)` já existente (11 caracteres, maiúsculas).

Testes: layout para 256 MiB, 31 GiB e 64 GiB (cluster e setores da FAT); imagem formatada
passa no `fsck.fat -n` e tem o rótulo certo no `fatlabel` (pulado se não instalados; o CI Linux
instala `dosfstools`).

## 2. `WindowsFormatter` (planos com PowerShell, exibidos no `--dry-run`)

- Passo 1: limpar (`Clear-Disk -RemoveData -RemoveOEM`, só se o disco não for RAW).
- Passo 2: `Initialize-Disk -PartitionStyle GPT|MBR` (`resolve_table` do núcleo: com
  `--max-size` só MBR).
- Passo 3: `New-Partition -DiskNumber N (-Size X | -UseMaximumSize) -AssignDriveLetter`
  (MBR com FAT32 → `-MbrType FAT32`).
- Passo 4: `Format-Volume -FileSystem FAT32|exFAT|NTFS -NewFileSystemLabel …` na partição de
  dados; **FAT32 acima de 32 GiB**: em vez do `Format-Volume`, o passo chama a própria CLI
  (`usbkeeper __fat32 \\.\PhysicalDriveN`, subcomando oculto), que acha a partição de dados,
  trava os volumes do disco e grava o FAT32 com o formatador do núcleo.
- ext4: recusado (“o Windows não lê ext4; use exFAT para trocar arquivos com o Linux”).

## 3. Reparar (`WindowsRepairer`)
- Por partição FAT32/exFAT/NTFS: `chkdsk <letra ou \\?\Volume{…}>` (verificar) ou
  `chkdsk … /f /x` (corrigir, com nova verificação depois, como no Linux).
- Núcleo: `Tool::Chkdsk` com os códigos do chkdsk (0 limpo, 1 corrigido, 2 limpeza → limpo,
  3 erros/sem verificar, outros → falha).

## 4. Montar e remover
- `mount_all`: atribui letra (`Add-PartitionAccessPath -AssignDriveLetter`) às partições com
  sistema de arquivos conhecido e sem letra; devolve as letras.
- `power_off`: “Remover hardware com segurança” via `CM_Locate_DevNodeW` +
  `CM_Request_Device_EjectW` no nó PnP do disco (id derivado do `Path` do `Get-Disk`, função
  pura testada). Veto do Windows → erro com o nome de quem vetou.
- O bloqueio “M4b2” sai de `format`, `reparar`, `montar` e `remover`.

## Testes
- Unitários: FAT32 (layout + fsck), `Tool::Chkdsk`, id PnP a partir do caminho do disco.
- e2e Windows (VHD): `format` exFAT, NTFS e FAT32 com rótulo (Windows confirma sistema de
  arquivos e rótulo); `--max-size` limita a partição; **FAT32 num VHD dinâmico de 40 GB**
  (Windows reconhece FAT32 com ~40 GB, `chkdsk` limpo, arquivo grava e lê); `reparar --check`
  e `reparar` limpos; tirar a letra → `montar` devolve uma letra; `remover` recusado sem USB;
  ext4 recusado.
- Manual (usuário): formatar um pendrive real em FAT32 e removê-lo com `usbkeeper remover`.
