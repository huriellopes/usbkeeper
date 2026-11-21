# M4c2: instalador .msi e release v0.9.0

Esta é a parte 2 de 2 da M4c. A parte 1 é a M4c1, a janela no Windows.

## Objetivo

- Publicar a v0.9.0 com um **`.msi` para Windows** e um **`.deb` para Linux**, ambos gerados pelo CI.
- Gerar o `.deb` no **Ubuntu 22.04**, para que funcione também no Ubuntu 22.04, no Mint 21 e no Debian 12 (glibc 2.35).
- O repositório APT continua sendo atualizado sozinho quando o release é publicado (workflow `apt.yml`).

## 1. O `.msi`

Um arquivo novo, `apps/gui/src-tauri/tauri.windows.conf.json`. O Tauri só o junta à configuração principal quando o build é no Windows, então o Linux não é afetado.

```json
{
  "bundle": {
    "targets": ["msi"],
    "resources": { "../../../target/release/usbkeeper.exe": "usbkeeper.exe" },
    "windows": {
      "webviewInstallMode": { "type": "downloadBootstrapper" },
      "wix": {
        "language": "pt-BR",
        "fragmentPaths": ["../../../packaging/windows/path.wxs"],
        "componentRefs": ["UsbkeeperPath"]
      }
    }
  }
}
```

**O que o `.msi` instala**
- A janela `usbkeeper-gui.exe` e a CLI `usbkeeper.exe`, juntas em `C:\Program Files\usbkeeper\`.
- A janela já procura a CLI ao lado dela (`cli_path`, feito na M4c1).

**CLI no PATH**
- O fragmento WiX `packaging/windows/path.wxs` acrescenta `[INSTALLDIR]` ao final do PATH do sistema: `Environment`, com `System="yes"` e `Part="last"`.
- A desinstalação remove essa entrada.
- O componente usa um valor em `HKLM\Software\usbkeeper` como `KeyPath`.

**Instalação**
- Por máquina, que é o padrão do Tauri. O próprio `.msi` pede UAC uma vez, ao instalar.

**WebView2**
- O Windows 10 e o 11 já trazem o WebView2.
- Se estiver faltando, o *bootstrapper* baixa o WebView2 durante a instalação.

**Assinatura**
- O `.msi` não terá assinatura digital, porque não há certificado.
- O README vai explicar o aviso do SmartScreen: "Mais informações" → "Executar assim mesmo".
- O README também vai mostrar como conferir o `.sha256`.

**Atualização**
- O `UpgradeCode` do Tauri é fixo, derivado do `identifier`. Por isso, instalar a v0.10 por cima substitui a v0.9.

## 2. Workflow de release: `.github/workflows/release.yml`

O workflow roda quando uma tag `v*` é enviada, ou manualmente pelo `workflow_dispatch`, informando a tag. Ele tem três jobs.

**1. `deb`, em `ubuntu-22.04`**
- Instala as dependências do WebKit, executa `npm ci` e depois `npx tauri build --bundles deb`.
- O `beforeBuildCommand` já compila a CLI em modo release.
- Confere com `objdump -T` que o `.deb` não exige glibc acima de 2.35.

**2. `msi`, em `windows-latest`**
- Executa `npm ci` e depois `npx tauri build --bundles msi`.
- Instala o `.msi` em modo silencioso (`msiexec /i … /qn`) e confere três coisas:
  - `"C:\Program Files\usbkeeper\usbkeeper.exe" --version` responde com a versão da tag;
  - o PATH do sistema contém a pasta de instalação;
  - `usbkeeper-gui.exe` existe.
- Desinstala (`msiexec /x … /qn`) e confere que a pasta e a entrada do PATH sumiram.

**3. `release`**
- Baixa os artefatos dos dois jobs.
- Gera um `<arquivo>.sha256` para cada um, no formato do `sha256sum`, que o `apt.yml` já confere.
- Cria o release como **rascunho**: `gh release create <tag> --draft --generate-notes`.
- Você revisa e publica o rascunho. A publicação dispara o `apt.yml`, que coloca o `.deb` no repositório APT.

**Nomes dos arquivos**

Ficam como o Tauri gera:
- `usbkeeper_0.9.0_amd64.deb`, que é o que o `apt.yml` procura (`*_amd64.deb*`);
- `usbkeeper_0.9.0_x64_pt-BR.msi`.

**Permissões e concorrência**
- `permissions: contents: write`, só no job `release`.
- `concurrency` por tag.

## 3. Versão e documentação

- A versão vai para **0.9.0** no workspace e no `tauri.conf.json`. Será um commit separado, no PR de release `dev` → `main`.
- README:
  - na tabela "Instalar", o Windows passa a ser "janela + CLI (instalador .msi)";
  - a seção do Windows ganha o passo a passo do `.msi`, o aviso do SmartScreen, a conferência do `.sha256`, o UAC por operação e a desinstalação pelo "Aplicativos instalados";
  - a seção do Linux passa a citar Ubuntu 22.04+, Mint 21+ e Debian 12+;
  - o `cargo install` continua como alternativa no Windows.
- O `CHANGELOG`/notas do release reúnem a M4a, a M4b1, a M4b2, a M4c1 e a M4c2.

## 4. Testes

- O job `msi` faz o ciclo completo: instalar, rodar a CLI pelo PATH e desinstalar.
- O job `deb` confere o limite da glibc.
- Também roda uma vez no PR, por `workflow_dispatch` com uma tag de teste (`v0.9.0-rc1`), e o rascunho é apagado em seguida. Isso dá para fazer antes do release de verdade, porque o rascunho não dispara o `apt.yml`.
- Teste manual no seu Windows: instalar o `.msi` do rascunho, abrir a janela pelo menu Iniciar, formatar um pendrive (com o UAC) e usar `usbkeeper list` num terminal novo.

## Fora do escopo

- Assinatura de código (Authenticode).
- `winget` e `.exe` NSIS.
- macOS (M7).
- Atualização automática dentro da janela.
