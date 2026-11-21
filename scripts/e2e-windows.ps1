# e2e do usbkeeper no Windows (M4a leitura, M4b1 gravação, M4b2 formatar/reparar/montar/remover, M4c1 canais da janela). Roda como administrador
# (o runner do GitHub já é): cria e anexa um VHD de 256 MB, formata com as
# ferramentas do próprio Windows e confere o que o usbkeeper enxerga.
#   pwsh scripts/e2e-windows.ps1 -Bin target\debug\usbkeeper.exe
param([Parameter(Mandatory)] [string] $Bin)
$ErrorActionPreference = 'Stop'
$Bin = (Resolve-Path $Bin).Path
$script:ok = 0; $script:bad = 0
function Ok($m)  { $script:ok++;  Write-Host "ok   $m" }
function Bad($m) {
  $script:bad++; Write-Host "FALHA $m" -ForegroundColor Red
  if ($script:last) { Write-Host "      saída: $(($script:last -join ' ') -replace '\s+', ' ')" }
}
function Check($cond, $m) { if ($cond) { Ok $m } else { Bad $m }; $script:last = $null }
function Usbk {
  $script:last = @(& $Bin @args 2>&1 | ForEach-Object { "$_" })
  $script:last
}
function UsbkJson {
  $err = New-TemporaryFile
  $out = & $Bin @args 2>$err
  $code = $LASTEXITCODE
  $script:last = @("exit=$code") + @(Get-Content $err) + @($out | Select-Object -First 5)
  Remove-Item $err
  if ($code -ne 0) { return $null }
  ($out -join "`n") | ConvertFrom-Json
}

$work = Join-Path $env:RUNNER_TEMP 'usbk-e2e'
if (-not $env:RUNNER_TEMP) { $work = Join-Path $env:TEMP 'usbk-e2e' }
New-Item -ItemType Directory -Force $work | Out-Null
$vhd = Join-Path $work 'pen.vhdx'
Remove-Item $vhd -ErrorAction SilentlyContinue

"create vdisk file=`"$vhd`" maximum=256 type=fixed`nselect vdisk file=`"$vhd`"`nattach vdisk" |
  Set-Content (Join-Path $work 'attach.txt') -Encoding ascii
diskpart /s (Join-Path $work 'attach.txt') | Out-Null
try {
  $disk = Get-Disk | Where-Object { $_.Location -eq $vhd } | Select-Object -First 1
  if (-not $disk) { throw "VHD não apareceu no Get-Disk" }
  $n = $disk.Number
  Initialize-Disk -Number $n -PartitionStyle GPT
  $part = New-Partition -DiskNumber $n -UseMaximumSize -AssignDriveLetter
  Format-Volume -Partition $part -FileSystem NTFS -NewFileSystemLabel 'E2E USBK' -Confirm:$false | Out-Null
  $letter = (Get-Partition -DiskNumber $n | Where-Object DriveLetter | Select-Object -First 1).DriveLetter
  Write-Host "VHD anexado: PhysicalDrive$n, letra ${letter}:"

  # ---------------------------------------------------------------- list
  Write-Host "--- list --all --json (para fixtures) ---"
  & $Bin list --all --json
  $all = UsbkJson list --all --json
  $v = @($all) | Where-Object { $_.name -eq "PhysicalDrive$n" }
  Check ($v -and $v.transport -eq 'virtual') "list --all mostra o VHD como virtual"
  Check ($v -and [math]::Abs([double]$v.size_bytes - 256MB) -lt 1MB) "tamanho do VHD (~256 MiB)"
  $p = @($v.partitions) | Where-Object { $_.label -eq 'E2E USBK' }
  Check ($p -and $p.fstype -eq 'ntfs' -and ($p.mountpoints -contains "${letter}:\")) "partição NTFS com rótulo e letra"
  Check ($v -and $v.partition_table -eq 'gpt') "tabela GPT"
  $plain = UsbkJson list --json
  Check (-not (@($plain) | Where-Object { $_.name -eq "PhysicalDrive$n" })) "list sem --all esconde o virtual"

  # ---------------------------------------------------------------- info
  $i = UsbkJson info "${letter}:" --json
  Check ($i -and $i.device.name -eq "PhysicalDrive$n") "info pela letra ${letter}: acha o disco"
  $i = UsbkJson info "$n" --json
  Check ($i -and $i.device.name -eq "PhysicalDrive$n") "info pelo número $n"
  $c = UsbkJson info 'C:' --json
  Check ($c -and $c.device.system_disk -eq $true) "disco do C: marcado como sistema"
  Check ($c -and (@($c.refusals) | Where-Object { $_.kind -eq 'system_disk' })) "C: recusado como alvo"

  # ---------------------------------------------------------------- leitura
  $h = UsbkJson health "PhysicalDrive$n" --scan --json
  Check ($h -and $h.scan -and [double]$h.scan.scanned_bytes -ge 255MB -and $h.scan.unreadable_blocks -eq 0) "varredura só leitura do VHD inteiro"
  $b = UsbkJson bench "PhysicalDrive$n" --sample-mb 32 --json
  Check ($b -and $b.bench.seq_read_mbps -gt 0 -and $b.bench.rand_read_iops -gt 0 -and $null -eq $b.bench.seq_write_mbps) "bench de leitura"

  # ---------------------------------------------------------------- M4b1: gravar
  $dp = "\\.\PhysicalDrive$n"
  $Guard = @('--allow-non-usb', '--unmount', '--yes-i-know', $dp)

  # volume em uso → recusa sem gravar
  $fh = [IO.File]::Open("${letter}:\em-uso.txt", 'Create', 'ReadWrite', 'None')
  try { $out = Usbk apagar "PhysicalDrive$n" @Guard } finally { $fh.Close() }
  Check ($LASTEXITCODE -ne 0 -and ($out -join ' ') -match 'em uso') "volume em uso: apagar recusa sem gravar"
  Check ((Get-Volume -DriveLetter $letter).FileSystemLabel -eq 'E2E USBK') "volume intacto depois da recusa"

  $r = UsbkJson fake-check "PhysicalDrive$n" @Guard --json
  Check ($r -and $r.report.verdict -eq 'genuine') "fake-check rápido: genuíno"
  $r = UsbkJson fake-check "PhysicalDrive$n" --full @Guard --json
  Check ($r -and $r.report.verdict -eq 'genuine') "fake-check completo: genuíno"
  $b = UsbkJson bench "PhysicalDrive$n" --write --sample-mb 32 @Guard --json
  Check ($b -and $b.bench.seq_write_mbps -gt 0 -and $b.bench.rand_write_iops -gt 0) "bench com escrita"

  $w = UsbkJson apagar "PhysicalDrive$n" --verificar @Guard --json
  Check ($w -and $w.report.verified -eq $true -and $w.report.mismatched_bytes -eq 0) "apagar com zeros e verificar"
  Update-Disk -Number $n
  Check ((Get-Disk -Number $n).PartitionStyle -eq 'RAW') "depois de apagar o Windows vê o disco vazio (RAW)"
  $out = Usbk apagar "PhysicalDrive$n" --descartar @Guard
  Check ($LASTEXITCODE -ne 0 -and ($out -join ' ') -match 'TRIM') "TRIM recusado no Windows antes da série"

  # imagem: cria volume com arquivo, imagem → apagar → restaurar
  Initialize-Disk -Number $n -PartitionStyle GPT
  $part = New-Partition -DiskNumber $n -UseMaximumSize -AssignDriveLetter
  Format-Volume -Partition $part -FileSystem NTFS -NewFileSystemLabel 'E2E IMG' -Confirm:$false | Out-Null
  $letter = (Get-Partition -DiskNumber $n | Where-Object DriveLetter | Select-Object -First 1).DriveLetter
  Set-Content "${letter}:\leia.txt" 'usbkeeper imagem' -NoNewline
  Write-VolumeCache -DriveLetter $letter
  $img = Join-Path $work 'pen.img.zst'
  Remove-Item $img, "$img.json" -ErrorAction SilentlyContinue
  $m = UsbkJson imagem criar "PhysicalDrive$n" -o $img --json
  Check ($m -and $m.meta.compressed -and $m.meta.size_bytes -eq 256MB) "imagem criada (zstd, 256 MiB)"
  UsbkJson apagar "PhysicalDrive$n" @Guard --json | Out-Null
  $rr = UsbkJson imagem restaurar $img "PhysicalDrive$n" --allow-non-usb --yes-i-know $dp --json
  Check ($rr -and $rr.verified -eq $true) "restaurar confere o SHA-256"
  Update-Disk -Number $n
  $p2 = Get-Partition -DiskNumber $n | Where-Object { (Get-Volume -Partition $_ -ErrorAction SilentlyContinue).FileSystemLabel -eq 'E2E IMG' } | Select-Object -First 1
  if ($p2 -and -not $p2.DriveLetter) { $p2 | Add-PartitionAccessPath -AssignDriveLetter; $p2 = Get-Partition -DiskNumber $n -PartitionNumber $p2.PartitionNumber }
  Check ($p2 -and (Get-Content "$($p2.DriveLetter):\leia.txt" -Raw) -eq 'usbkeeper imagem') "volume e arquivo de volta depois de restaurar"

  # ISO híbrida montada à mão
  $iso = Join-Path $work 'teste.iso'
  $bytes = [byte[]]::new(1MB)
  $bytes[510] = 0x55; $bytes[511] = 0xAA
  [Text.Encoding]::ASCII.GetBytes('CD001').CopyTo($bytes, 32769)
  [Text.Encoding]::ASCII.GetBytes('TESTE_LINUX'.PadRight(32)).CopyTo($bytes, 32808)
  for ($k = 40000; $k -lt $bytes.Length; $k++) { $bytes[$k] = [byte]($k % 251) }
  [IO.File]::WriteAllBytes($iso, $bytes)
  $hash = (Get-FileHash $iso -Algorithm SHA256).Hash.ToLower()
  $g = UsbkJson gravar-iso $iso "PhysicalDrive$n" --sha256 $hash @Guard --json
  Check ($g -and $g.verified -eq $true -and $g.report.sha256 -eq $hash) "gravar ISO híbrida conferindo o hash"
  $win = Join-Path $work 'win.iso'
  [Text.Encoding]::ASCII.GetBytes('CCCOMA_X64FRE_PT-BR_DV9'.PadRight(32)).CopyTo($bytes, 32808)
  [IO.File]::WriteAllBytes($win, $bytes)
  $out = Usbk gravar-iso $win "PhysicalDrive$n" @Guard
  Check ($LASTEXITCODE -ne 0 -and ($out -join ' ') -match 'Windows') "ISO do Windows recusada"

  # ---------------------------------------------------------------- M4b2: formatar
  foreach ($c in @(@('exfat', 'EXFAT E2E', 'exFAT'), @('ntfs', 'NTFS E2E', 'NTFS'), @('fat32', 'FAT E2E', 'FAT32'))) {
    $fs, $lb, $winfs = $c
    $f = UsbkJson format "PhysicalDrive$n" --fs $fs --label $lb @Guard --json
    Update-Disk -Number $n
    $vol = Get-Partition -DiskNumber $n | Get-Volume -ErrorAction SilentlyContinue | Where-Object FileSystemLabel -eq $lb
    Check ($f -and $vol -and $vol.FileSystem -eq $winfs -and $vol.DriveLetter) "format ${fs}: Windows vê $winfs, rótulo e letra"
  }
  $f = UsbkJson format "PhysicalDrive$n" --fs exfat --max-size 128MiB @Guard --json
  Update-Disk -Number $n
  $dp1 = Get-Partition -DiskNumber $n | Sort-Object Offset | Select-Object -Last 1
  Check ($f -and $f.table -eq 'mbr' -and ($dp1.Offset + $dp1.Size) -le 128MB) "format --max-size: MBR terminando em até 128 MiB"
  $out = Usbk format "PhysicalDrive$n" --fs ext4 @Guard
  Check ($LASTEXITCODE -ne 0 -and ($out -join ' ') -match 'exFAT') "ext4 recusado no Windows, sugerindo exFAT"
  $out = Usbk format "PhysicalDrive$n" --fs fat32 --dry-run @Guard
  Check ($LASTEXITCODE -eq 0 -and ($out -join ' ') -match 'Format-Volume -FileSystem FAT32') "--dry-run mostra os comandos"

  # ---------------------------------------------------------------- reparar (chkdsk)
  UsbkJson format "PhysicalDrive$n" --fs fat32 --label 'REPARO' @Guard --json | Out-Null
  $r = UsbkJson reparar "PhysicalDrive$n" --check @Guard --json
  Check ($r -and $r.report.partitions[0].status -eq 'clean') "reparar --check (chkdsk): limpo"
  $r = UsbkJson reparar "PhysicalDrive$n" @Guard --json
  Check ($r -and $r.report.partitions[0].status -in 'clean', 'fixed') "reparar (chkdsk /f /x) e nova verificação"

  # ---------------------------------------------------------------- montar
  $pl = Get-Partition -DiskNumber $n | Where-Object { $_.DriveLetter -and [int][char]$_.DriveLetter -ne 0 } | Select-Object -First 1
  $pl | Remove-PartitionAccessPath -AccessPath "$($pl.DriveLetter):\"
  $mt = UsbkJson montar "PhysicalDrive$n" --allow-non-usb --json
  $back = Get-Partition -DiskNumber $n -PartitionNumber $pl.PartitionNumber
  Check ($mt -and @($mt.mounted).Count -eq 1 -and $back.DriveLetter) "montar devolve uma letra à partição"
  $mt = UsbkJson montar "PhysicalDrive$n" --allow-non-usb --json
  Check ($mt -and @($mt.mounted).Count -eq 0 -and @($mt.mountpoints).Count -ge 1) "montar de novo não duplica"

  # ---------------------------------------------------------------- remover
  $out = Usbk remover "PhysicalDrive$n"
  Check ($LASTEXITCODE -ne 0 -and ($out -join ' ') -match 'não é USB') "remover recusa disco que não é USB"
  $out = Usbk remover "PhysicalDrive$n" --allow-non-usb --no-power-off
  Check ($LASTEXITCODE -ne 0 -and ($out -join ' ') -match 'só desmontar') "--no-power-off não se aplica no Windows"

  # ---------------------------------------------------------------- FAT32 de 40 GB (o Windows recusaria)
  $big = Join-Path $work 'grande.vhdx'
  Remove-Item $big -ErrorAction SilentlyContinue
  "create vdisk file=`"$big`" maximum=40960 type=expandable`nselect vdisk file=`"$big`"`nattach vdisk" |
    Set-Content (Join-Path $work 'big.txt') -Encoding ascii
  diskpart /s (Join-Path $work 'big.txt') | Out-Null
  $bd = Get-Disk | Where-Object { $_.Location -eq $big } | Select-Object -First 1
  $m = $bd.Number
  $out = Usbk format "PhysicalDrive$m" --fs fat32 --dry-run --allow-non-usb
  Check (($out -join ' ') -match '__fat32') "FAT32 > 32 GB usa o formatador do usbkeeper"
  $f = UsbkJson format "PhysicalDrive$m" --fs fat32 --label 'GRANDE40' --allow-non-usb --yes-i-know "\\.\PhysicalDrive$m" --json
  Update-Disk -Number $m
  $bv = Get-Partition -DiskNumber $m | Get-Volume -ErrorAction SilentlyContinue | Where-Object FileSystemLabel -eq 'GRANDE40'
  Check ($f -and $bv -and $bv.FileSystem -eq 'FAT32' -and $bv.Size -gt 39GB) "Windows reconhece FAT32 de ~40 GB"
  if ($bv -and $bv.DriveLetter) {
    $bl = $bv.DriveLetter
    $payload = [byte[]]::new(8MB); (New-Object Random 42).NextBytes($payload)
    [IO.File]::WriteAllBytes("${bl}:\dados.bin", $payload)
    New-Item -ItemType Directory "${bl}:\pasta\sub" | Out-Null
    Set-Content "${bl}:\pasta\sub\leia.txt" 'FAT32 do usbkeeper' -NoNewline
    Write-VolumeCache -DriveLetter $bl
    $same = [Linq.Enumerable]::SequenceEqual([byte[]][IO.File]::ReadAllBytes("${bl}:\dados.bin"), $payload)
    Check ($same -and (Get-Content "${bl}:\pasta\sub\leia.txt" -Raw) -eq 'FAT32 do usbkeeper') "grava e lê arquivos e pastas no FAT32 de 40 GB"
    chkdsk "${bl}:" | Out-Null
    Check ($LASTEXITCODE -eq 0) "chkdsk do Windows: FAT32 de 40 GB sem erros"
  } else { Bad "FAT32 de 40 GB sem letra" }
  "select vdisk file=`"$big`"`ndetach vdisk" | Set-Content (Join-Path $work 'bigdet.txt') -Encoding ascii
  diskpart /s (Join-Path $work 'bigdet.txt') | Out-Null

  # ---------------------------------------------------------------- M4c1: caminho da janela (UAC + canais privados)
  $env:USBKEEPER_E2E_BIN = $Bin
  $env:USBKEEPER_E2E_DISK = "PhysicalDrive$n"
  $script:last = @(cargo test -q -p usbkeeper-windows --test elevated -- --ignored --test-threads=1 2>&1 | ForEach-Object { "$_" })
  Check ($LASTEXITCODE -eq 0) "CLI elevada pelos canais: lista, progresso e cancelamento"

  $hist = UsbkJson history --json
  Check ($null -ne $hist) "histórico em %LOCALAPPDATA%"
} finally {
  "select vdisk file=`"$vhd`"`ndetach vdisk" | Set-Content (Join-Path $work 'detach.txt') -Encoding ascii
  diskpart /s (Join-Path $work 'detach.txt') | Out-Null
}
Write-Host "`n$script:ok ok, $script:bad falhas"
if ($script:bad -gt 0) { exit 1 }
