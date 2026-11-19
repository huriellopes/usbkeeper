# M4a — Base do Windows (só leitura)

Primeira das três entregas do Windows (M4a base → M4b destrutivas → M4c janela + .msi).
Ao fim da M4a, no Windows funcionam: `list`, `info`, `health` (SMART + varredura só
leitura), `bench` (só leitura), `history`, `report`, `recomendar`, `guia`. Os comandos
destrutivos respondem que chegam na M4b. A janela continua só Linux até a M4c.
Versão continua 0.8.x (0.9.0 sai na M4c, com o instalador).

## Decisões

- **Listar via PowerShell em JSON** (`Get-Disk`, `Get-Partition`, `Get-Volume`), papel
  igual ao `lsblk -J` no Linux: um só processo `powershell.exe -NoProfile -NonInteractive`
  roda um script embutido que imprime **um** objeto JSON `{disks, partitions, volumes,
  parents}`. A interpretação é pura e testada com fixtures (compila e roda em qualquer SO).
- **Nativo só onde é preciso** (crate `windows`): I/O bruto e tamanho exato.
- **Disco do sistema é sempre protegido**: `Device` ganha `system_disk: bool` (Windows:
  `IsSystem || IsBoot` ou contém a `%SystemDrive%`; Linux: sempre `false`, continua pelos
  pontos de montagem). `safety::system_mountpoint` passa a recusar também quando a flag
  está ligada (devolve a letra do sistema, ex. `C:\`).

## Crate `crates/platform-windows` (`usbkeeper-windows`)

### `storage.rs` (puro, sem `cfg`)
- `parse(json: &str) -> Result<Vec<Device>, CoreError>`; tolera objeto único no lugar de
  lista (o `ConvertTo-Json` desembrulha arrays de 1 elemento) e campos nulos.
- Mapeamento por disco (`Number = N`):
  - `name = "PhysicalDriveN"`, `path = "\\.\PhysicalDriveN"`, `size_bytes = Size`.
  - `transport` por `BusType`: `USB`→Usb, `SATA`/`ATA`→Sata, `NVMe`→Nvme,
    `SD`/`MMC`→Mmc, `SCSI`/`SAS`/`RAID`/`iSCSI`→Scsi, `File Backed Virtual`/`Virtual`→Virtual,
    outro→Other (aceita número ou texto do enum).
  - `removable = hotplug = transport ∈ {Usb, Mmc}`; `read_only = IsReadOnly`.
  - `model = FriendlyName` (ou `Model`), `vendor = Manufacturer` não vazio,
    `serial = SerialNumber` aparado, vazio→None.
  - `partition_table`: `GPT`→`"gpt"`, `MBR`→`"dos"`, `RAW`→None.
  - setores: `LogicalSectorSize`/`PhysicalSectorSize` (padrão 512).
  - `system_disk = IsSystem || IsBoot || alguma letra == SystemDrive`.
  - `usb`: para USB, `UsbInfo { vendor_id, product_id }` vindos do pai PnP
    (`USB\VID_xxxx&PID_yyyy\...`) quando houver; `speed = Unknown`.
- Partições: `name = "PhysicalDriveN-pK"`, `path` = `\\?\Volume{...}\` do volume quando
  houver, `size_bytes = Size`; `fstype` (`FAT32`→`vfat`, `exFAT`→`exfat`, `NTFS`→`ntfs`,
  `ReFS`→`refs`, demais em minúsculas), `label = FileSystemLabel`,
  `mountpoints` = `["E:\"]` + `AccessPaths` que não sejam `\\?\Volume`.
- `find(devices, query)`: aceita `PhysicalDrive1`, `\\.\PhysicalDrive1`, `1`, `pd1`
  e letra (`E:`, `e:`, `E:\`) → disco que contém a letra.

### `enumerate.rs` (`#[cfg(windows)]`)
- `WindowsEnumerator { include_virtual }`: roda o script, `parse`, filtra virtuais
  (salvo `include_virtual`) e implementa `Enumerator` com `get` via `find`.
- Erros do PowerShell viram `CoreError::Backend` com a primeira linha do stderr.

### `rawdev.rs` (`#[cfg(windows)]`)
- `RawDevice::open(path, write)`: `CreateFileW` com `FILE_SHARE_READ|FILE_SHARE_WRITE`,
  `FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH`; tamanho por
  `IOCTL_DISK_GET_LENGTH_INFO`; `read_at`/`write_at` com `OVERLAPPED.Offset` (sem seek).
  Escrita existe mas só é usada na M4b (lá entram travar/desmontar volumes).
- Acesso negado → `CoreError::Backend("precisa de administrador: abra o terminal como
  administrador (ou use sudo usbkeeper … no Windows 11 24H2+)")`.

### `admin.rs`
- `is_elevated()` (token `TokenElevation`); usado só para mensagens.

## SMART
- `usbkeeper-smart` aceita o executável e o nome do disco por plataforma: no Windows,
  `smartctl` no PATH ou `C:\Program Files\smartmontools\bin\smartctl.exe`, disco
  `/dev/pdN`. Ausente → relatório `unknown` explicando como instalar o smartmontools.

## CLI
- `backend.rs`: implementação Windows com `WindowsEnumerator` e `open_raw`; `Formatter`,
  `Mounts`, `Ejector`, `Repairer` respondem "chega na próxima etapa (M4b)".
- `bench` só leitura (a escrita já exige flag explícita; no Windows recusa por enquanto).
- Histórico em `%LOCALAPPDATA%\usbkeeper\history.db` (já suportado).

## Testes
- Fixtures em `crates/platform-windows/tests/fixtures/`: pendrive USB (FAT32, letra E:),
  NVMe do sistema (C:, `IsBoot`), VHD (`File Backed Virtual`), objeto único em vez de
  lista, campos nulos. Testes de mapeamento, `find` por número/letra, `system_disk`.
- core: `system_mountpoint` recusa quando `system_disk`.
- **e2e no CI (runner Windows, administrador)**: `scripts/e2e-windows.ps1` cria e anexa
  um VHD de 256 MB com `diskpart`, cria partição NTFS com rótulo e letra, e confere:
  `list --all --json` traz o VHD como `virtual` com o tamanho e o rótulo; `info <letra>`
  acha o disco pela letra; o disco do C: tem `system_disk = true`; varredura só leitura
  e `bench` (leitura) no VHD terminam; comando destrutivo responde "M4b". Desanexa no fim.
- Manual (usuário, Windows real): `usbkeeper list`, `info E:`, `health` com pendrive.

## Fora do escopo
Formatar/apagar/imagem/ISO/reparar/montar/remover (M4b); janela e `.msi` (M4c);
velocidade USB negociada no Windows.
