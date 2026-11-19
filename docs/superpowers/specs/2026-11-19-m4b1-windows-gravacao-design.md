# M4b1 — Windows: gravar no disco (apagar, falsificado, imagem, ISO)

Segunda entrega do Windows (M4a base ✔ → **M4b1 gravação** → M4b2 formatar/reparar/montar/
remover → M4c janela + .msi). Ao fim da M4b1, no Windows funcionam também: `apagar`,
`fake-check` (rápido e completo), `imagem restaurar`, `gravar-iso` e `bench --write`.
`format`, `reparar`, `montar` e `remover` continuam respondendo que chegam na M4b2.

## Gravar com o disco liberado

O Windows recusa escrita em setores de volume montado. `RawDevice::open(path, write=true)`
para `\\.\PhysicalDriveN`:

1. Acha os volumes do disco N por API nativa: `FindFirstVolumeW`/`FindNextVolumeW` e, para
   cada `\\?\Volume{…}`, `IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS` (qualquer extensão no disco N
   conta; volumes sem letra também).
2. Abre cada volume (sem a barra final), `FSCTL_LOCK_VOLUME` (até 5 tentativas, 200 ms) e
   `FSCTL_DISMOUNT_VOLUME`. Os handles ficam guardados no `RawDevice` até ele ser descartado.
3. Volume que não trava → `CoreError::Backend("o volume E: está em uso: feche as janelas do
   Explorer e os programas que usam o pendrive e tente de novo")` sem gravar nada (o Windows
   não informa quem usa; `holders` continua vazio).
4. No `flush` e ao descartar: `IOCTL_DISK_UPDATE_PROPERTIES` no disco, para o Windows reler a
   tabela de partições (equivale ao `udevadm settle`).

Funções puras (testadas em todo SO): `disk_number(path) -> Option<u32>` (aceita
`\\.\PhysicalDriveN`, maiúsculas/minúsculas) e `extents_on_disk(extents, n) -> bool`.

### `WindowsMounts`
- `unmount_all`: trava+desmonta e solta (verificação antecipada, com a mesma mensagem de
  "em uso"), para recusar antes de pedir a série; `holders` vazio; `mount_all` → M4b2.
- No Windows a letra continua atribuída ao volume desmontado: por isso, como no Linux, gravar
  num disco com letra exige `--unmount` (o pendrive comum sempre tem letra).

## CLI
- A trava "chega depois" sai do `precheck` e vai só para `format`, `reparar`, `montar` e
  `remover` (mensagem passa a citar M4b2).
- `apagar --discard` (TRIM) no Windows: recusado com explicação (fica para depois).
- `take` (releitura e conferência de identidade) inalterado: o enumerador do Windows já acha o
  disco pelo caminho `\\.\PhysicalDriveN`.

## Testes
- Unitários: `disk_number`, `extents_on_disk`.
- **e2e no CI (VHD de 256 MB, NTFS com letra)**, todos com `--allow-non-usb --unmount`:
  - gravar com volume em uso (arquivo aberto no volume) → recusado com "em uso", disco intacto;
  - `apagar --verify` → só zeros (a verificação do próprio apagar), letra/volume somem;
  - `fake-check --quick` e `--full` → genuíno;
  - `bench --write` → mede escrita;
  - formata de novo com as ferramentas do Windows, grava um arquivo, `imagem criar` →
    `apagar` → `imagem restaurar` → `verified`; volume com o rótulo e o arquivo de volta;
  - `gravar-iso` de uma ISO híbrida montada à mão (bytes `55 AA` + `CD001`) com `--sha256` →
    `verified`; ISO com volume `CCCOMA_X64FRE` recusada;
  - `format` responde M4b2.
- Manual (usuário): `apagar` e `fake-check --quick` num pendrive real.
