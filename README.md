# usbkeeper

Gerenciador multiplataforma de pendrives e HDs externos: listar, formatar, verificar saúde,
detectar pendrives **falsificados** (capacidade adulterada) e medir velocidade, com histórico por número de série.

> Status: **Linux completo** (CLI e janela): listar, formatar, ejetar, **detectar falsificados**, velocidade,
> **saúde (SMART + varredura)**, **verificar/reparar**, **montar**, **autoteste SMART**, **apagar com segurança**,
> **imagem do disco**, **gravar ISO**, **guia de ajuda**, **assistente de formatação**, histórico, laudo HTML e
> tema claro/escuro. **Windows em andamento** (CLI de consulta); macOS depois (ver [Roadmap](#roadmap)).

## Instalar

| Sistema | Situação | Como instalar |
|---|---|---|
| **Linux** (Debian, Ubuntu, Mint) | completo: CLI + janela | repositório APT (abaixo) |
| **Windows** 10/11 | CLI completa (a janela vem na próxima versão) | compilar com `cargo` (abaixo); instalador `.msi` na próxima versão |
| **macOS** | ainda não suportado | previsto no [Roadmap](#roadmap) |
| **WSL2** | via Linux, com o pendrive repassado pelo `usbipd` | ver [WSL2](#wsl2) |

### Linux (Debian, Ubuntu, Mint)

Pelo repositório APT, que atualiza junto com o sistema no `sudo apt upgrade`:

```sh
sudo install -d -m 0755 /etc/apt/keyrings
curl -fsSL https://huriellopes.github.io/usbkeeper/apt/usbkeeper.gpg | sudo tee /etc/apt/keyrings/usbkeeper.gpg >/dev/null
echo "deb [arch=amd64 signed-by=/etc/apt/keyrings/usbkeeper.gpg] https://huriellopes.github.io/usbkeeper/apt stable main" \
  | sudo tee /etc/apt/sources.list.d/usbkeeper.list
sudo apt update && sudo apt install usbkeeper
```

Instala a CLI (`usbkeeper`) e a janela (menu de aplicativos → usbkeeper). Requer amd64 com
Ubuntu 24.04+, Mint 22+ ou Debian 13+. Impressão digital da chave:
`70E1 1B52 B3A3 FBB9 C0EE A95F 4695 DA6D E72E 8FAF`.

Sem o repositório: baixe o `.deb` e o `.sha256` de um [release](https://github.com/huriellopes/usbkeeper/releases) e

```sh
sha256sum -c usbkeeper_*_amd64.deb.sha256
sudo apt install ./usbkeeper_*_amd64.deb
```

Remover: `sudo apt remove usbkeeper` (o histórico em `~/.local/share/usbkeeper` fica).

### Windows 10/11 (em andamento)

A **CLI** faz o mesmo que no Linux: listar, info, saúde (SMART e varredura), velocidade,
teste de falsificado, **formatar** (FAT32, exFAT, NTFS), reparar (`chkdsk`), apagar, imagem do
disco, gravar ISO, montar (atribuir letra) e **remover com segurança**. A janela e o instalador
`.msi` vêm na próxima versão. Não existem no Windows: ext4 (o Windows não lê) e o descarte (TRIM).

**FAT32 acima de 32 GB:** o Windows se recusa a formatar, mas TVs, carros e videogames só leem
FAT32. O usbkeeper tem um formatador FAT32 próprio e formata pendrives e HDs de qualquer tamanho
até 2 TB: `usbkeeper format E: --fs fat32 --unmount`.

Antes de gravar, o usbkeeper trava e desmonta todos os volumes do disco e os mantém travados até
o fim. Se algum estiver em uso (janela do Explorer aberta, arquivo aberto), ele recusa **sem gravar
nada** e pede para fechar. Como no Linux, gravar num pendrive com letra exige `--unmount`. O
`remover` faz o mesmo que o ícone "Remover hardware com segurança"; se o Windows recusar, diz quem
está usando o dispositivo.

Compilar e instalar (PowerShell):

```powershell
winget install Rustlang.Rustup
winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
# feche e abra o terminal, depois:
cargo install --git https://github.com/huriellopes/usbkeeper --locked usbkeeper
winget install smartmontools.smartmontools      # opcional: SMART de HD/SSD
```

Use num **terminal como administrador** (ler o disco exige), ou `sudo usbkeeper …` no
Windows 11 24H2+ com o sudo ativado. O disco aceita várias formas de nome:

```powershell
usbkeeper list
usbkeeper info E:              # pela letra; também: info 1, info PhysicalDrive1
usbkeeper health E: --scan     # SMART + leitura de toda a superfície
```

O disco do sistema (C:) nunca é aceito como alvo. Histórico em `%LOCALAPPDATA%\usbkeeper`.
Remover: `cargo uninstall usbkeeper`.

### macOS

Ainda não suportado: compila, mas responde que a plataforma não é suportada. Está no
[Roadmap](#roadmap), depois do Windows.

### Compilar do código (Linux)

```sh
cargo install --git https://github.com/huriellopes/usbkeeper --locked usbkeeper   # só a CLI
# CLI + janela (.deb): precisa de Node 22 e das libs do WebKitGTK
sudo apt install libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev
git clone https://github.com/huriellopes/usbkeeper && cd usbkeeper/apps/gui
npm ci && npx tauri build      # gera target/release/bundle/deb/usbkeeper_<versão>_amd64.deb
sudo apt install ../../target/release/bundle/deb/usbkeeper_*_amd64.deb
```

## Interface gráfica (Linux)

A janela lista os discos ao vivo (USB primeiro; o disco do sistema aparece com 🔒 e sem ações
destrutivas) e tem abas para **Verificar** (fake-check), **Saúde**, **Velocidade**, **Formatar** e
**Histórico**, além de **Ejetar**.

A interface **nunca roda como root**: para testar ou formatar ela executa a própria CLI via
`pkexec /usr/bin/usbkeeper …`, então todas as travas de segurança da CLI valem também na janela.
A senha é pedida uma vez e vale por alguns minutos (polkit `auth_admin_keep`). A confirmação
destrutiva exige digitar a série, como na CLI, e a CLI confere tudo de novo.

Desenvolvimento: `cd apps/gui && npx tauri dev`. `USBKEEPER_SHOW_VIRTUAL=1` lista loop devices e
libera alvos não-USB; `USBKEEPER_CLI=…` escolhe a CLI usada. Orçamento: `scripts/measure-gui.sh`
(< 250 MB de PSS, < 1% de CPU parado).

## Guia de ajuda

Quinze tópicos explicados em português, iguais na CLI e na janela: primeiros passos, tipos de
dispositivo, USB e velocidade, **MBR × GPT**, **sistemas de arquivos** (FAT32, exFAT, NTFS, ext4),
o que formatar faz, **pendrives falsificados**, **saúde/SMART**, velocidade, **remoção segura**,
**reparar**, **apagar com segurança**, **imagem do disco e ISO de boot**, o que o usbkeeper
protege e glossário.

```sh
usbkeeper guia              # lista os tópicos
usbkeeper guia mbr          # aceita apelidos: gpt, exfat, fat32, smart, fake, eject…
```

Na janela: **? Ajuda e guia** na barra lateral (com busca) e botões **?** ao lado de cada decisão
(sistema de arquivos, tabela, teste rápido × completo, SMART…). O conteúdo fica em
`crates/guide/topics/*.md`.

## Me ajude a escolher (qual formato usar)

```sh
usbkeeper recomendar sdb --uso tv --arquivos-grandes sim     # misto | windows | linux | aparelho | videogame
```

Cruza o uso pretendido com o que se sabe do disco (tamanho, se o último teste deu falso, saúde,
USB 2 limitando) e recomenda sistema de arquivos, tabela e limite, **explicando o porquê**. Na janela,
“Me ajude a escolher” na aba Formatar preenche o formulário.

## Verificar e reparar

```sh
sudo usbkeeper reparar sdb --check --unmount    # só verifica, não altera nada
sudo usbkeeper reparar sdb --unmount            # corrige e verifica de novo (pede a série)
```

FAT32 (`fsck.vfat`), exFAT (`fsck.exfat`), NTFS (`ntfsfix`, básico; para reparo completo use
`chkdsk /f` no Windows) e ext2/3/4 (`e2fsck`). O status final do reparo vem de uma **nova
verificação** feita logo depois. A tabela de partições é só verificada (`sfdisk --verify`).

## Montar e abrir no gerenciador de arquivos

`usbkeeper montar sdb` (ou **Montar** / **Abrir pasta** na janela) monta via udisks, sem senha,
no mesmo lugar que o gerenciador de arquivos usaria. No Windows e no macOS o próprio sistema monta.

## Autoteste SMART

```sh
sudo usbkeeper autoteste sda --curto --esperar   # ~2 min; --longo lê a superfície inteira (horas)
```

O próprio disco se testa no firmware, sem alterar dados. Cancelar aborta o teste no disco. O
resultado entra no histórico e o `health` mostra os últimos autotestes. Muitas pontes USB e
pendrives não suportam (o usbkeeper explica).

## Apagar com segurança

```sh
sudo usbkeeper apagar sdb --verificar --unmount              # zeros, 1 passada, relê e confere
sudo usbkeeper apagar sdb --aleatorio --passadas 2 --unmount # dados aleatórios
sudo usbkeeper apagar sdb --descartar --unmount              # TRIM: rápido em SSD/NVMe que aceitem
```

Mesmas travas do formatar (série, disco do sistema nunca). Em pendrive, cartão e SSD, sobrescrever
**não garante** apagar as áreas de reserva do controlador: para garantia total, criptografe desde o
início (veja `usbkeeper guia apagar`). O *secure erase* de firmware não é usado (pode inutilizar o
disco se interrompido).

## Imagem do disco (backup) e gravar ISO

```sh
sudo usbkeeper imagem criar sdb -o pendrive.img.zst        # só lê; zstd + .json com SHA-256
usbkeeper imagem verificar pendrive.img.zst                 # o arquivo continua íntegro?
sudo usbkeeper imagem restaurar pendrive.img.zst sdb --unmount   # grava e confere o hash
sudo usbkeeper gravar-iso ubuntu.iso sdb --unmount          # hash de --sha256, .sha256 ou SHA256SUMS
```

A ISO é conferida antes (se houver hash) e depois de gravar. ISOs do **Windows** são recusadas
(gravadas byte a byte não dão boot; precisam de preparo especial). Na janela: aba **Backup e ISO**.

## Tema

Na janela, **Sistema / Claro / Escuro** no rodapé da barra lateral; a escolha é lembrada.

## Remover com segurança

`usbkeeper remover sdb` (ou o botão **⏏ Remover com segurança**) termina as gravações pendentes,
desmonta e desliga o dispositivo. Se algum programa estiver usando o disco, mostra **qual** (PID,
nome e arquivo aberto) para você fechar e tentar de novo. Se o dispositivo não aceitar ser
desligado (alguns leitores de cartão), ele é desmontado e o aviso diz para tirar quando o LED parar.

## Uso da linha de comando

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

# Saúde (precisa de root)
sudo target/debug/usbkeeper health sdb              # SMART: HD/SSD, inclusive atrás de ponte USB
sudo target/debug/usbkeeper health sdb --scan       # + lê o disco todo atrás de blocos ilegíveis (não grava)

# Histórico e laudo (fake-check, bench e health são registrados automaticamente)
usbkeeper history                                   # todos os dispositivos já testados
usbkeeper history sdb                               # linha do tempo de um (nome, chave ou série)
usbkeeper report sdb -o laudo.html                  # laudo HTML autocontido (imprima para PDF)

# Remoção segura: desmonta e desliga; se algo estiver usando o disco, mostra o processo
cargo run -- eject sdb
```

`list`/`info`/`eject` não precisam de root (o `eject` usa udisks/polkit).
O `format` resolve todas as ferramentas (`wipefs`, `sfdisk`, `mkfs.*`) **antes** de apagar
qualquer coisa e informa o pacote que falta.

O `list` também avisa quando um dispositivo USB 3 negociou só velocidade USB 2
(porta, hub ou cabo limitando).

## Saúde e histórico

- **SMART** via `smartctl -j`, tentando `auto`, `sat`, `usbjmicron`, `usbprolific` e `usbsunplus` até
  um trazer dados. Veredito: saudável / atenção (realocados, pendentes, incorrigíveis, CRC, vida útil
  NVMe ≥ 90%, temperatura ≥ 60 °C) / falhando (autoteste reprovado, atributo abaixo do limite, alerta
  crítico NVMe). Pendrives e cartões quase nunca têm SMART: use `--scan`.
- **Varredura** somente leitura em blocos de 1 MiB; quando um falha, relê em blocos de 4 KiB para
  localizar o defeito. Trechos lentos (> 500 ms por MiB) são reportados como regiões degradadas.
- **Histórico** em SQLite (`~/.local/share/usbkeeper/history.db`; com `sudo`, na home de quem chamou).
  Chave: `VID:PID:série` (pendrives baratos repetem séries), `sem-usb:série` para discos internos e,
  sem série, o tamanho anunciado no lugar dela.
  `--no-history` desliga; `USBKEEPER_DB` muda o arquivo.

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
| `crates/smart` | `smartctl -j`: detecção da ponte USB, interpretação ATA/NVMe, veredito |
| `crates/history` | SQLite (rusqlite *bundled*): dispositivos e eventos |
| `crates/guide` | Guia em Markdown embutido; HTML para a janela e texto para o terminal |
| `apps/cli` | Binário `usbkeeper` (clap): comandos, barra de progresso, laudo HTML; `--progress-json`/`--cancel-stdin` para a interface |
| `apps/gui` | Interface Tauri 2 (React + TS): lê discos/histórico como usuário, opera via `pkexec usbkeeper` |

## Roadmap

- [x] **M0**: núcleo, SafetyGuard, `list`/`info` no Linux
- [x] **M1**: `format` (GPT/MBR; FAT32/exFAT/NTFS/ext4), desmontar/ejetar mostrando quem prende o disco
- [x] **M2**: `fake-check` (rápido estilo f3probe e completo grava+verifica) e `bench`
- [x] **M2b**: `format --max-size` para reaproveitar só a capacidade real de um falso (como o f3fix)
- [x] **M3**: `health` (SMART via smartctl + varredura), histórico SQLite, laudo HTML/PDF
- [ ] **M4** (M4a base ✔, M4b1 gravação ✔, M4b2 formatar/reparar/montar/remover ✔; falta a janela + .msi): Windows
- [x] **M5**: GUI Tauri 2
- [x] **M5b**: guia de ajuda (CLI e janela) e remoção segura em destaque
- [x] **M6a**: tema, assistente de formatação, verificar/reparar, montar
- [x] **M6b**: autoteste SMART e apagar com segurança
- [x] **M6c**: imagem do disco e gravar ISO de boot
- [ ] **M7**: macOS

## Licença

MIT
