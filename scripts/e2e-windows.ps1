# e2e do usbkeeper no Windows (M4a, só leitura). Roda como administrador
# (o runner do GitHub já é): cria e anexa um VHD de 256 MB, formata com as
# ferramentas do próprio Windows e confere o que o usbkeeper enxerga.
#   pwsh scripts/e2e-windows.ps1 -Bin target\debug\usbkeeper.exe
param([Parameter(Mandatory)] [string] $Bin)
$ErrorActionPreference = 'Stop'
$Bin = (Resolve-Path $Bin).Path
$script:ok = 0; $script:bad = 0
function Ok($m)  { $script:ok++;  Write-Host "ok   $m" }
function Bad($m) { $script:bad++; Write-Host "FALHA $m" -ForegroundColor Red }
function Check($cond, $m) { if ($cond) { Ok $m } else { Bad $m } }
function Usbk { & $Bin @args 2>&1 | ForEach-Object { "$_" } }
function UsbkJson { $out = & $Bin @args 2>$null; if ($LASTEXITCODE -ne 0) { return $null }; ($out -join "`n") | ConvertFrom-Json }

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

  # ---------------------------------------------------------------- destrutivos
  $out = Usbk apagar "PhysicalDrive$n" --allow-non-usb --yes-i-know x
  Check ($LASTEXITCODE -ne 0 -and ($out -join ' ') -match 'M4b') "apagar responde que chega na M4b (sem gravar)"
  $out = Usbk format "PhysicalDrive$n" --allow-non-usb --yes-i-know x
  Check ($LASTEXITCODE -ne 0 -and ($out -join ' ') -match 'M4b') "format responde que chega na M4b"
  Check ((Get-Volume -DriveLetter $letter).FileSystemLabel -eq 'E2E USBK') "VHD intacto depois das recusas"

  $hist = UsbkJson history --json
  Check ($null -ne $hist) "histórico em %LOCALAPPDATA%"
} finally {
  "select vdisk file=`"$vhd`"`ndetach vdisk" | Set-Content (Join-Path $work 'detach.txt') -Encoding ascii
  diskpart /s (Join-Path $work 'detach.txt') | Out-Null
}
Write-Host "`n$script:ok ok, $script:bad falhas"
if ($script:bad -gt 0) { exit 1 }
