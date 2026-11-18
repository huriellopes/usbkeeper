# usbkeeper

Gerenciador multiplataforma de pendrives e HDs externos: listar, formatar, verificar saúde,
detectar pendrives **falsificados** (capacidade adulterada) e medir velocidade, com histórico por número de série.

> Status: **M2**: listar, inspecionar, formatar, ejetar, **detectar falsificados** e medir velocidade no Linux.
> SMART, histórico e as versões Windows/macOS chegam nas próximas etapas (ver [Roadmap](#roadmap)).

## Uso

```sh
cargo run -- list            # discos visíveis e se podem ser alvo
cargo run -- list --all      # inclui loop/zram
cargo run -- info sdb        # detalhes, partições, USB e veredito de segurança
cargo run -- info sdb --json  # inclui os processos usando o disco

# Formatar (precisa de root). Sempre veja o plano antes com --dry-run:
cargo run -- format sdb --fs exfat -l "Fotos" --dry-run
sudo target/debug/usbkeeper format sdb --fs exfat -l "Fotos" --unmount
#   --fs fat32|exfat|ntfs|ext4   --table gpt|mbr (padrão: mbr até 2 TiB)
#   --max-size 7.8G|7640MiB      só o início do disco (pendrive falso); força MBR
#   --yes-i-know <SERIE>         confirma sem perguntar (scripts)

# Pendrive falsificado? (precisa de root)
sudo target/debug/usbkeeper fake-check sdb          # rápido (~segundos), restaura os blocos originais
sudo target/debug/usbkeeper fake-check sdb --full   # definitivo: grava e lê tudo, APAGA o disco
sudo target/debug/usbkeeper fake-check sdb --json   # laudo em JSON (prova para reembolso)
# Falso? Reaproveite só a parte real (o fake-check sugere o valor, com 2% de margem):
sudo target/debug/usbkeeper format sdb --fs exfat --max-size 7640MiB

# Velocidade
sudo target/debug/usbkeeper bench sdb               # leitura sequencial + 4K aleatória (não destrutivo)
sudo target/debug/usbkeeper bench sdb --write       # inclui escrita (destrutivo)

# Remoção segura: desmonta e desliga; se algo estiver usando o disco, mostra o processo
cargo run -- eject sdb
```

`list`/`info`/`eject` não precisam de root (o `eject` usa udisks/polkit).
O `format` resolve todas as ferramentas (`wipefs`, `sfdisk`, `mkfs.*`) **antes** de apagar
qualquer coisa e informa o pacote que falta.

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

## Como a detecção de falsificados funciona

Pendrives falsos têm o firmware alterado para anunciar mais capacidade do que existe. O que passa do
limite real é **descartado** (lê zeros/lixo) ou **dá a volta** (o endereço 9 GB grava em cima do 1 GB).

Cada setor de 512 B gravado no teste carrega um carimbo com **o próprio endereço** e uma semente
única da execução, seguidos de dados pseudoaleatórios derivados dos dois. Na leitura, um setor pode estar:
íntegro; com lixo; ou com o carimbo íntegro **de outro endereço** (prova de que os dois apontam para a
mesma célula física).

- **Completo** (`--full`): grava e verifica todos os setores, como o H2testw/f3write. Definitivo.
- **Rápido** (padrão): grava ~128–256 blocos de 4 KiB em grade de potência de 2 e faz busca binária
  até o limite real (precisão de 1 MiB), no espírito do f3probe. Salva os blocos originais e os
  restaura do maior para o menor endereço, o que é correto mesmo quando endereços compartilham a mesma
  célula; Ctrl+C também restaura. Detecta descarte e "volta" com módulo potência de 2 (o caso comum);
  para módulos arbitrários, use o completo.

Com `--max-size` o `format` cria uma única partição que termina antes do limite real, sempre em
**MBR**: a GPT grava uma cópia do cabeçalho no último setor *anunciado*, que num falso não existe.

O disco é aberto com `O_DIRECT` (a leitura vem do pendrive, não do cache) e `O_EXCL` (o kernel recusa
se houver algo montado).

## Testes

```sh
cargo test --workspace                 # unitários + fixtures, sem tocar em discos
cargo build && sudo scripts/e2e-loop.sh  # format/eject/fake-check/bench reais em loop devices;
                                          # inclui pendrives falsos emulados com device-mapper
```

## WSL2

O WSL2 não enxerga USB do Windows. Use [usbipd-win](https://github.com/dorssel/usbipd-win)
(`usbipd attach --wsl --busid <BUSID>`); o `usbkeeper list` mostra a instrução quando detecta WSL sem discos USB.

## Arquitetura

| Crate | Papel |
|---|---|
| `crates/core` | Modelo, traits de plataforma, SafetyGuard, algoritmos de fake-check e benchmark sobre `BlockDevice`, `FakeDrive` (falso emulado para testes) |
| `crates/platform-linux` | `lsblk -J -b` + sysfs (velocidade USB), formatação (wipefs/sfdisk/mkfs), desmontagem via udisks, processos que prendem o disco (`/proc`), `RawDevice` (O_DIRECT/O_EXCL), WSL |
| `apps/cli` | Binário `usbkeeper` (clap) |

## Roadmap

- [x] **M0**: núcleo, SafetyGuard, `list`/`info` no Linux
- [x] **M1**: `format` (GPT/MBR; FAT32/exFAT/NTFS/ext4), desmontar/ejetar mostrando quem prende o disco
- [x] **M2**: `fake-check` (rápido estilo f3probe e completo grava+verifica) e `bench`
- [x] **M2b**: `format --max-size` para reaproveitar só a capacidade real de um falso (como o f3fix)
- [ ] **M3**: `health` (SMART via smartctl + varredura), histórico SQLite, laudo HTML/PDF
- [ ] **M4**: Windows
- [ ] **M5**: GUI Tauri 2
- [ ] **M6**: macOS, gravador de ISO, imagem de disco

## Licença

MIT
