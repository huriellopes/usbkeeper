# M6c — Imagem do disco e gravar ISO — design

Data: 2026-11-19 · Status: **aprovado no brainstorming** · Versão alvo: 0.8.0 (seguida de release)

## Motor comum (`crates/core/src/image.rs`)

Cópia disco ↔ fluxo com SHA-256 calculado durante a cópia, progresso, cancelar e verificação.

```rust
pub struct CopyReport { bytes: u64, sha256: String /* hex */, seconds: f64 }
pub fn device_to_writer(dev, len, out: &mut dyn Write, phase: Phase, progress, cancel) -> Result<CopyReport>
pub fn reader_to_device(src: &mut dyn Read, dev, progress, cancel, total_hint: u64) -> Result<CopyReport>
pub fn hash_device(dev, len, progress, cancel) -> Result<String>
```

- Blocos de 1 MiB, `AlignedBuf`/O_DIRECT. O último bloco pode não ser múltiplo de 512 (ISOs são
  múltiplos de 2048; imagens têm o tamanho do disco): na gravação completa com zeros até 512 e o
  hash cobre só os bytes reais; na leitura lê alinhado e corta.
- `reader_to_device` recusa (antes de exceder) se a fonte for maior que o disco.
- Fases: `Phase::Image` (“copiando”), `Phase::Verify` (“verificando”).

## ISO (`crates/core/src/iso.rs`, puro)

`inspect(head: &[u8]) -> IsoInfo { is_iso, hybrid, volume_id: Option<String>, windows: bool }` a
partir dos primeiros 34 816 bytes: `hybrid` = assinatura `55 AA` no byte 510 (setor de boot MBR,
necessário para dar boot como pendrive); ISO 9660 = `CD001` no offset 32 769; `volume_id` = 32
bytes no offset 32 808, sem espaços; `windows` = volume começando com `CCCOMA_`, `CPBA_`,
`CENA_`, `CCSA_`, `GRMC` ou igual a `ESD-ISO`.

## Imagem do disco (CLI `image`, apelido `imagem`)

- `imagem criar <disco> -o <arquivo> [--sem-compressao] [--json]`: lê o disco inteiro (somente
  leitura, sem confirmação; root), comprime com zstd nível 3 (padrão) e grava `<arquivo>`;
  grava `<arquivo>.json` com `{format: 1, model, serial, size_bytes, sha256, compressed,
  created_at, usbkeeper_version}`. Arquivos criados com a posse devolvida a quem chamou
  (sudo/pkexec). Recusa sobrescrever arquivo existente sem `--sobrescrever`.
- `imagem verificar <arquivo>`: descomprime e calcula o SHA-256; compara com o `.json`.
- `imagem restaurar <arquivo> <disco> [guard] [--json]`: destrutivo (precheck → série →
  desmontar → reconferir); recusa disco menor que `size_bytes`; descomprime → grava; relê
  `size_bytes` do disco e compara o hash com o do `.json` (ou com o calculado na gravação, se não
  houver `.json`).
- Compressão detectada pelo número mágico do zstd (`28 B5 2F FD`), não pela extensão.
- Histórico: `image_create` (`created`), `image_restore` (`restored` | `verify_failed`).

## Gravar ISO (CLI `burn-iso`, apelido `gravar-iso`)

`gravar-iso <arquivo.iso> <disco> [--sha256 <hex>] [guard] [--json]`:
1. Inspeciona: não é ISO 9660 nem tem setor de boot → recusa (“não parece uma imagem de disco”);
   ISO do Windows → **recusa** com explicação (gravada assim não dá boot; precisa de preparo
   especial, fora do escopo) — `--forcar` grava mesmo assim; ISO não híbrida → aviso.
2. Hash esperado: `--sha256`, ou `<iso>.sha256` / `SHA256SUMS` na mesma pasta (linha com o
   nome do arquivo); havendo esperado, calcula o hash da ISO **antes** e recusa se não bater.
3. Fluxo destrutivo de sempre; grava; relê o tamanho da ISO e compara o hash.
- Histórico: `iso_write` (`written` | `verify_failed`).

## Interface

Aba **Backup e ISO**, marcada como destrutiva: fica indisponível para o disco do sistema (a
imagem do disco do sistema em uso não seria consistente, e restaurar/gravar nele é proibido).
- Criar imagem: diálogo “salvar como” (`*.img.zst`) → `OpRequest::ImageCreate { device, output }`.
- Restaurar: diálogo “abrir” → mostra o `.json` (modelo, tamanho, data) → confirmação pela
  série → `OpRequest::ImageRestore { device, image }`.
- Gravar ISO: diálogo “abrir” (`*.iso`) e campo opcional de SHA-256 → `OpRequest::BurnIso {
  device, iso, sha256 }`.
- Resultado com hash e verificação; permissão `dialog:allow-open`.

## Guia

Tópico `imagem` (“Imagem do disco e ISO de boot”): backup antes de testar/formatar/reparar;
o que a imagem contém (tudo, inclusive espaço vazio, que a compressão reduz); restaurar exige
disco do mesmo tamanho ou maior; o que é hash e como conferir; ISO híbrida; por que ISO do
Windows não dá boot gravada direto. Apelidos `iso`, `backup`, `image`.

## Testes

- core: cópia disco→memória→disco preserva bytes e hash (FakeDrive); tamanho não múltiplo de 512;
  fonte maior que o disco recusada; cancelar; `inspect` com bytes montados (híbrida, ISO 9660,
  Windows, lixo).
- CLI: ida e volta criar/verificar/restaurar sobre **arquivos comuns** como “disco” (RawDevice
  abre arquivo), comprimido e sem compressão; `.json` gerado; hash de ISO errado recusado;
  leitura de `SHA256SUMS`.
- GUI: pedidos corretos para as três ações (vitest).
- e2e: criar imagem de um loop com dados aleatórios → apagar o loop → restaurar → `cmp`;
  `xorriso` cria uma ISO híbrida pequena → gravar com hash certo → `cmp`; hash errado
  recusado; “ISO do Windows” (volume `CCCOMA_X64FRE`) recusada.
