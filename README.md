# usbkeeper

Gerenciador multiplataforma de pendrives e HDs externos: listar, formatar, verificar saúde,
detectar pendrives **falsificados** (capacidade adulterada) e medir velocidade, com histórico por número de série.

> Status: **M0**: listagem e inspeção no Linux. Formatação, detecção de falsificados, SMART e
> as versões Windows/macOS chegam nas próximas etapas (ver [Roadmap](#roadmap)).

## Uso

```sh
cargo run -- list            # discos visíveis e se podem ser alvo
cargo run -- list --all      # inclui loop/zram
cargo run -- info sdb        # detalhes, partições, USB e veredito de segurança
cargo run -- info sdb --json
```

`list`/`info` não precisam de root.

O `list` também avisa quando um dispositivo USB 3 negociou só velocidade USB 2
(porta, hub ou cabo limitando).

## Segurança

Antes de qualquer operação destrutiva, o *SafetyGuard* (`crates/core/src/safety.rs`) exige:

- **Disco do sistema nunca é alvo**: qualquer disco com `/`, `/boot`, `/boot/efi`, `/home`, swap… montado,
  inclusive através de LUKS/LVM. Nenhuma flag libera.
- Somente discos **USB**, salvo `--allow-non-usb`.
- Disco **desmontado**, gravável e com mídia.
- Confirmação digitando o **número de série** (ou o caminho, se o dispositivo não tiver série).
- Reconferência da identidade (série + tamanho) imediatamente antes de gravar.

## WSL2

O WSL2 não enxerga USB do Windows. Use [usbipd-win](https://github.com/dorssel/usbipd-win)
(`usbipd attach --wsl --busid <BUSID>`); o `usbkeeper list` mostra a instrução quando detecta WSL sem discos USB.

## Arquitetura

| Crate | Papel |
|---|---|
| `crates/core` | Modelo (`Device`, `UsbInfo`…), trait `Enumerator`, SafetyGuard |
| `crates/platform-linux` | `lsblk -J -b` + sysfs (`/sys/bus/usb/devices/*/speed`), detecção de WSL |
| `apps/cli` | Binário `usbkeeper` (clap) |

## Roadmap

- [x] **M0**: núcleo, SafetyGuard, `list`/`info` no Linux
- [ ] **M1**: `format` (GPT/MBR; FAT32/exFAT/NTFS/ext4), desmontar/ejetar mostrando quem prende o disco
- [ ] **M2**: `fake-check` (rápido estilo f3probe e completo grava+verifica) e `bench`
- [ ] **M3**: `health` (SMART via smartctl + varredura), histórico SQLite, laudo HTML/PDF
- [ ] **M4**: Windows
- [ ] **M5**: GUI Tauri 2
- [ ] **M6**: macOS, gravador de ISO, imagem de disco

## Licença

MIT
