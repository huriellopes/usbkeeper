# M4c1 — janela no Windows (UAC por operação, pouco consumo)

Parte 1 de 2 da M4c. A M4c2 trata do `.msi` e do release v0.9.0.

## Objetivo

Fazer a janela (Tauri 2) funcionar no Windows com as mesmas operações da CLI (a M4b deixou a CLI completa):

- a janela abre **sem privilégio**;
- cada operação que toca o disco pede **UAC**, como o `pkexec` no Linux;
- a janela gasta o mínimo de CPU, memória e GPU, também no Linux.

## 1. Elevação por operação

### Quais operações pedem UAC

No Windows, ler `\\.\PhysicalDriveN` também exige administrador. Por isso, toda operação de `OpRequest` pede UAC, exceto `Eject`. Isso é o mesmo que o Linux já faz hoje: `needs_root()` não muda.

O laudo (`save_report`) continua rodando sem privilégio.

### Canais

A janela cria três *named pipes*: `\\.\pipe\usbkeeper-<nonce>-{in,out,err}`.

- O `<nonce>` tem 32 dígitos hex (128 bits) e vem de `BCryptGenRandom`.
- Cada pipe é criado com:
  - `FILE_FLAG_FIRST_PIPE_INSTANCE`, para não reaproveitar um pipe já existente com esse nome;
  - `nMaxInstances = 1`;
  - `PIPE_REJECT_REMOTE_CLIENTS`;
  - o descritor de segurança SDDL `D:P(A;;GA;;;<SID do usuário>)(A;;GA;;;BA)(A;;GA;;;SY)`.
- Direções:
  - `in`: a janela escreve e a CLI lê; é por ele que a janela manda a linha `cancel`;
  - `out`: a CLI escreve o JSON do resultado;
  - `err`: a CLI escreve as linhas de progresso e de erro.

### Execução

A janela chama `ShellExecuteExW` com:

- verbo `runas`;
- arquivo: a CLI;
- parâmetros: `--pipe <nonce>` seguido dos argumentos de `cli::args`, já com aspas aplicadas;
- `SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC`;
- `SW_HIDE`.

Em seguida, a janela espera as conexões com `ConnectNamedPipe` e prazo de 60 s, porque o usuário pode demorar para responder ao UAC. Se o prazo vence, ela encerra o processo e a operação falha com "a CLI não respondeu".

Depois que um cliente conecta, a janela confere que `GetNamedPipeClientProcessId` é igual a `GetProcessId(hProcess)`. Se for diferente, ela desconecta, encerra o processo e a operação falha com "conexão inesperada no canal da operação". Isso impede que outro processo se passe pela CLI.

Do lado da CLI:

- `--pipe <nonce>` é um argumento global oculto, aceito só no Windows.
- É tratado antes do `clap`: abre os três pipes com `CreateFileW` e chama `SetStdHandle` para `STD_INPUT_HANDLE`, `STD_OUTPUT_HANDLE` e `STD_ERROR_HANDLE`.
- O resto da CLI não muda. Isso funciona porque o `std` do Rust consulta `GetStdHandle` a cada uso de stdin/stdout/stderr no Windows.
- Se algum pipe não abrir, a CLI sai com o código 3, sem fazer nada.

O progresso, o cancelamento e o resultado reaproveitam o leitor atual de `ops.rs`. O ponto de origem passa a ser uma abstração `Spawned { stdin: Box<dyn Write+Send>, stdout: Box<dyn Read+Send>, stderr: Box<dyn Read+Send>, wait: Box<dyn FnOnce() -> Option<i32> + Send> }`:

- no Linux e para operações sem privilégio: `std::process::Command`, com ou sem `pkexec`;
- no Windows, para operações com privilégio: o `runas` acima. O `wait` usa `WaitForSingleObject` e depois `GetExitCodeProcess`.

### Erros

- UAC recusado: `ShellExecuteExW` falha com `ERROR_CANCELLED` (1223), e a mensagem é "autorização negada". `exit_message` ganha esse caso por meio de um código interno, sem depender de 126/127.
- A CLI não encontrada continua com a mensagem atual de "não foi possível iniciar".

### Segurança

O `SafetyGuard` continua sendo aplicado dentro da CLI elevada:

- a confirmação pelo número de série (`confirmation_token`) continua exigida;
- nada que vem da janela substitui essas verificações.

O canal só serve para transportar dados. Quem pode escrever nele é só o próprio usuário, os administradores ou o SYSTEM.

## 2. Lista de discos no Windows sem PowerShell a cada 2 s

O `enumerate()` da GUI ganha a versão Windows: `WindowsEnumerator { include_virtual: show_virtual() }`.

A `signature()` passa a ter uma versão por plataforma:

- **Linux:** fica como está (`/proc/partitions`, `/proc/self/mounts`, data do histórico).
- **Windows:** `"{geração}\n{GetLogicalDrives():x}\n{data do histórico}"`, onde:
  - a **geração** é um `AtomicU64` incrementado pelo callback de `CM_Register_Notification`;
  - o callback é registrado para `GUID_DEVINTERFACE_DISK` e `GUID_DEVINTERFACE_VOLUME`, cobrindo chegada e remoção de discos e volumes;
  - o registro é feito uma vez, no `setup` do Tauri;
  - o callback só incrementa o contador: não cria thread, não cria janela e não faz trabalho.

`MAX_AGE` passa a ser **120 s no Windows** e continua **30 s no Linux**. Ele é o intervalo em que a lista é relida mesmo sem mudança aparente, para pegar casos como a trava de gravação.

O que mais muda no Windows:

- `open_folder`: aceita a raiz de uma letra do próprio disco (`E:\`) e abre com `explorer.exe`. `allowed_mountpoint` passa a aceitar `X:\` quando esse caminho está em `all_mountpoints()` do disco.
- `mount_device`: usa `WindowsMounts.mount_all`.
- `device_holders`: devolve a lista vazia. No Windows, quem prende o disco aparece no erro do `remover`.
- `cli_path`: usa `usbkeeper.exe` ao lado da janela. Não existe o equivalente ao `/usr/bin` como alternativa.

### Capacidades da plataforma

Um comando novo, `platform()`, devolve `{ ext4: bool, trim: bool }`. No Windows, os dois são `false`.

A interface usa esse comando para:

- esconder o ext4 na formatação e no assistente;
- esconder o método "Descartar (TRIM)" em Apagar.

A CLI já recusa os dois no Windows; esconder na interface só evita oferecer algo que vai falhar.

## 3. Consumo (as duas plataformas)

- **Progresso:** o leitor em `ops.rs` emite `op-progress` no máximo a cada **250 ms**. Uma troca de fase é emitida na hora, e a última linha antes de terminar também é sempre emitida. O controle fica num `Throttle` puro, com testes. A CLI continua mandando a cada 100 ms; quem descarta o excesso é a janela.
- **Janela minimizada:** no evento de janela do Tauri, a janela é considerada oculta quando está minimizada ou não está visível. Nesse caso:
  - `list_devices` devolve o cache sem consultar nada;
  - no Windows, o WebView2 recebe `ICoreWebView2_19::SetMemoryUsageTargetLevel(LOW)` por `with_webview`; ao restaurar, volta para `NORMAL`. Se a interface `_19` não existir (WebView2 antigo), isso é ignorado sem erro.
  - O `document.visibilityState` continua sendo a primeira barreira. A verificação no Rust cobre o caso em que o WebView não fica `hidden` ao minimizar.
- **GPU:** continua sem animações, sem transições e sem desfoque. A aceleração por hardware fica ligada, porque o desenho por software gastaria mais CPU.
- **Perfil de release** no `Cargo.toml` raiz:

  ```toml
  [profile.release]
  lto = "thin"
  codegen-units = 1
  strip = true
  ```

  `panic` continua `unwind`, porque o Tauri e as threads de operação dependem disso.

### Metas, conferidas à mão no Windows pelo Gerenciador de Tarefas

- **Parada e visível:** CPU média < 0,5%.
- **Memória total, somando o processo e o WebView2:** < 150 MB.
- **Minimizada:** nenhuma chamada ao PowerShell, e a memória cai em relação à janela visível.

## 4. Build e CI

- `usbkeeper-gui` ganha dependências só para Windows:
  - `usbkeeper-windows`;
  - `windows` 0.61, com as features que faltam: `Win32_UI_Shell`, `Win32_System_Pipes`, `Win32_Security_Authorization`, `Win32_Security_Cryptography`, `Win32_UI_WindowsAndMessaging` e `Win32_System_Registry` se for preciso;
  - `webview2-com` 0.39, a mesma versão que o Tauri usa.
- O job do Windows no CI passa a:
  - rodar `npm ci && npm run build` em `apps/gui`;
  - rodar `cargo clippy -p usbkeeper-gui -- -D warnings`;
  - rodar `cargo test -p usbkeeper-gui`.
- Um teste de integração `#[ignore]`, `elevated_roundtrip`, roda no e2e do Windows:
  - usa o caminho `runas` + pipes com `usbkeeper.exe list --json` e confere que o JSON chega;
  - também roda um `bench` de leitura no VHD de teste, com progresso no `err` e cancelamento pelo `in`.
  - No runner do GitHub o usuário é administrador e o UAC está desligado, então o `runas` não mostra pedido.

## 5. Testes

**Testes unitários (rodam em qualquer plataforma):**

- `pipe_names(nonce)`;
- `sddl(sid)`;
- formação de nonce, que deve ter 32 caracteres hex;
- montagem dos parâmetros do `ShellExecute`, incluindo aspas em caminhos com espaço;
- `Throttle`: limite de 250 ms, troca de fase imediata e última linha garantida;
- `needs_refresh`, com `MAX_AGE` por plataforma;
- `allowed_mountpoint` com `E:\`;
- `exit_message` para UAC recusado.

**Testes do lado da CLI:** o `--pipe` é removido dos argumentos antes do `clap`, com teste da função pura `split_pipe_arg`.

**Testes manuais no Windows real:**

- formatar um pendrive pela janela, com o UAC aparecendo;
- recusar o UAC e ver "autorização negada";
- cancelar um teste de falsificação;
- conferir a CPU e a memória com a janela parada e minimizada.

## Fora do escopo (M4c2)

- o `.msi` e a CLI no PATH;
- o workflow de release, incluindo o `.deb` no Ubuntu 22.04;
- a versão v0.9.0;
- o README de instalação no Windows.
