#!/usr/bin/env bash
# Teste ponta a ponta do `format`/`eject` em um loop device (arquivo de imagem).
# Nunca toca em disco real. Precisa de root:
#   cargo build && sudo scripts/e2e-loop.sh
set -euo pipefail
[[ $EUID -eq 0 ]] || { echo "rode com sudo: sudo $0"; exit 1; }

BIN=${BIN:-$(dirname "$0")/../target/debug/usbkeeper}
WORK=$(mktemp -d)
IMG=$WORK/pen.img
export USBKEEPER_DB=$WORK/history.db   # histórico isolado do real
truncate -s 600M "$IMG"
command -v jq >/dev/null && command -v dmsetup >/dev/null || { echo "precisa de jq e dmsetup"; exit 1; }
LOOP=$(losetup -fP --show "$IMG")
HOLDER=""
FAKE_TOPS=() FAKE_DMS=() FAKE_BACKS=()
cleanup() {
  [[ -n $HOLDER ]] && kill "$HOLDER" 2>/dev/null || true
  umount "$WORK/mnt" 2>/dev/null || true
  umount "$WORK/fake" 2>/dev/null || true
  for l in "${FAKE_TOPS[@]}"; do losetup -d "$l" 2>/dev/null || true; done
  # losetup -d é assíncrono: espera o loop de topo soltar o dm antes de removê-lo.
  udevadm settle 2>/dev/null || true
  for d in "${FAKE_DMS[@]}"; do dmsetup remove --retry "$d" 2>/dev/null || echo "aviso: não removi $d"; done
  for l in "${FAKE_BACKS[@]}"; do losetup -d "$l" 2>/dev/null || true; done
  losetup -d "$LOOP" 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT
# Sobras de uma rodada interrompida: holders (loop de topo) → dm → loops de baixo.
for link in /dev/mapper/usbkeeper-e2e-*; do
  [[ -e $link ]] || continue
  dm=$(basename "$(readlink -f "$link")")
  for h in /sys/block/"$dm"/holders/*; do [[ -e $h ]] && losetup -d "/dev/${h##*/}" 2>/dev/null; done
  slaves=$(ls "/sys/block/$dm/slaves")
  udevadm settle 2>/dev/null || true
  dmsetup remove --retry "${link##*/}" && echo "limpei sobra: ${link##*/}"
  for sl in $slaves; do losetup -d "/dev/$sl" 2>/dev/null || true; done
done
echo "loop de teste: $LOOP ($IMG)"

pass=0 fail=0
ok()  { echo "ok   $1"; pass=$((pass + 1)); }
bad() { echo "FAIL $1"; fail=$((fail + 1)); }

format() { "$BIN" format "$LOOP" --allow-non-usb --yes-i-know "$LOOP" "$@"; }

check_fs() { # fs table label tipo_esperado rotulo_esperado pttype_esperado
  local out
  if ! out=$(format --fs "$1" --table "$2" -l "$3" 2>&1); then
    bad "$1/$2: $out"; return
  fi
  local t l pt
  t=$(blkid -p -s TYPE -o value "${LOOP}p1" || true)
  l=$(blkid -p -s LABEL -o value "${LOOP}p1" || true)
  pt=$(blkid -p -s PTTYPE -o value "$LOOP" || true)
  if [[ $t == "$4" && $l == "$5" && $pt == "$6" ]]; then
    ok "$1/$2 rótulo=\"$l\""
  else
    bad "$1/$2: tipo=$t rótulo=$l tabela=$pt"
  fi
}

check_fs fat32 mbr "meu pen"    vfat  "MEU PEN"    dos
check_fs exfat gpt "Fotos 2026" exfat "Fotos 2026" gpt
check_fs ntfs  mbr "Dados"      ntfs  "Dados"      dos
check_fs ext4  gpt "linux"      ext4  "linux"      gpt

# Confirmação errada não pode alterar nada.
before=$(blkid -p -s TYPE -o value "${LOOP}p1")
if "$BIN" format "$LOOP" --fs fat32 --allow-non-usb --yes-i-know errado >/dev/null 2>&1; then
  bad "confirmação errada foi aceita"
elif [[ $(blkid -p -s TYPE -o value "${LOOP}p1") == "$before" ]]; then
  ok "confirmação errada recusada sem alterar o disco"
else
  bad "confirmação errada alterou o disco"
fi

# Montado + em uso: recusa sem --unmount, mostra o processo com --unmount.
mkdir -p "$WORK/mnt"
mount "${LOOP}p1" "$WORK/mnt"
echo dado > "$WORK/mnt/arquivo.txt"
sleep 300 < "$WORK/mnt/arquivo.txt" &
HOLDER=$!

out=$(format --fs exfat 2>&1) && bad "formatou disco montado sem --unmount" \
  || { [[ $out == *"--unmount"* ]] && ok "montado recusado sem --unmount" || bad "mensagem sem dica: $out"; }

out=$(format --fs exfat --unmount 2>&1) && bad "formatou disco em uso" \
  || { [[ $out == *"PID $HOLDER"* ]] && ok "disco em uso mostra o PID $HOLDER (sleep)" || bad "holder não listado: $out"; }

kill "$HOLDER"; wait "$HOLDER" 2>/dev/null || true; HOLDER=""

if out=$(format --fs exfat -l "Depois" --unmount 2>&1) \
   && [[ $(blkid -p -s LABEL -o value "${LOOP}p1") == "Depois" ]] \
   && ! findmnt -rn "$WORK/mnt" >/dev/null; then
  ok "--unmount desmonta e formata"
else
  bad "--unmount: $out"
fi

# Eject: recusa sem --allow-non-usb; desmonta com --no-power-off.
mount "${LOOP}p1" "$WORK/mnt"
"$BIN" eject "$LOOP" --no-power-off >/dev/null 2>&1 && bad "ejetou não-USB sem flag" || ok "eject recusa não-USB sem flag"
if "$BIN" eject "$LOOP" --no-power-off --allow-non-usb >/dev/null 2>&1 && ! findmnt -rn "$WORK/mnt" >/dev/null; then
  ok "eject --no-power-off desmonta"
else
  bad "eject não desmontou"
fi

# ---------------------------------------------------------------- M2
# Pendrives falsos de verdade via device-mapper, expostos como loop de topo:
#   wrap:    N cópias lineares do mesmo trecho (endereço dá a volta)
#   discard: trecho real + alvo "zero" (escritas somem, leituras dão zero)
make_fake() { # nome tipo anunciado_MiB real_MiB  → define TOP e BACK_FILE
  local name=$1 kind=$2 adv=$(( $3 * 2048 )) real=$(( $4 * 2048 ))
  BACK_FILE=$WORK/$name.img
  head -c "$(( $4 << 20 ))" /dev/urandom > "$BACK_FILE"
  local back table="" off=0 len
  back=$(losetup -f --show "$BACK_FILE"); FAKE_BACKS+=("$back")
  if [[ $kind == badblock ]]; then
    # 4 KiB que devolvem EIO em 10 MiB (setor ilegível de verdade).
    local at=$(( 10 * 2048 ))
    table="0 $at linear $back 0"$'\n'"$at 8 error"$'\n'"$(( at + 8 )) $(( real - at - 8 )) linear $back $(( at + 8 ))"$'\n'
  elif [[ $kind == wrap ]]; then
    while (( off < adv )); do
      len=$(( adv - off < real ? adv - off : real ))
      table+="$off $len linear $back 0"$'\n'; off=$(( off + len ))
    done
  else
    table="0 $real linear $back 0"$'\n'"$real $(( adv - real )) zero"$'\n'
  fi
  printf '%s' "$table" | dmsetup create "usbkeeper-e2e-$name"; FAKE_DMS+=("usbkeeper-e2e-$name")
  # --direct-io=on: sem ele o loop grava no page cache do dm, indexado pelo
  # endereço lógico, e a volta/descarte só ocorreria no writeback (invisível
  # para a leitura). Pendrive real não tem essa camada de cache.
  TOP=$(losetup -f --show -P --direct-io=on "/dev/mapper/usbkeeper-e2e-$name"); FAKE_TOPS+=("$TOP")
  [[ $(cat "/sys/block/${TOP#/dev/}/loop/dio") == 1 ]] || { echo "loop sem direct-io; abortando"; exit 1; }
}

fake_check() { # dispositivo [args...] → JSON do relatório
  local dev=$1; shift
  "$BIN" fake-check "$dev" --allow-non-usb --yes-i-know "$dev" --json "$@" | jq -c .report || echo null
}

expect() { # descrição json filtro-jq
  if jq -e "$3" <<<"$2" >/dev/null; then ok "$1"; else bad "$1: $2"; fi
}

MIB=1048576
sum_before=$(sha256sum "$IMG" | cut -d' ' -f1)
r=$(fake_check "$LOOP")
expect "rápido: genuíno" "$r" '.verdict == "genuine"'
[[ $(sha256sum "$IMG" | cut -d' ' -f1) == "$sum_before" ]] \
  && ok "rápido: dados do genuíno intactos após restaurar" || bad "rápido alterou o genuíno"

make_fake wrap wrap 128 32
sum_before=$(sha256sum "$BACK_FILE" | cut -d' ' -f1)
r=$(fake_check "$TOP")
expect "rápido: wrap 32 MiB anunciado 128 → falso" "$r" \
  ".verdict == \"fake\" and (.real_bytes - 32*$MIB | fabs) <= $MIB and .wrap_module_bytes == 32*$MIB"
[[ $(sha256sum "$BACK_FILE" | cut -d' ' -f1) == "$sum_before" ]] \
  && ok "rápido: dados do falso (wrap) restaurados bit a bit" || bad "rápido não restaurou o wrap"
r=$(fake_check "$TOP" --full)
expect "completo: wrap → real exato 32 MiB" "$r" ".verdict == \"fake\" and .real_bytes == 32*$MIB and .wrap_module_bytes == 32*$MIB"

make_fake discard discard 128 40
r=$(fake_check "$TOP")
expect "rápido: descarte 40 MiB anunciado 128 → falso" "$r" \
  ".verdict == \"fake\" and (.real_bytes - 40*$MIB | fabs) <= $MIB"
r=$(fake_check "$TOP" --full)
expect "completo: descarte → real exato 40 MiB" "$r" ".verdict == \"fake\" and .real_bytes == 40*$MIB"

# M2b: reaproveitar o falso só na parte real, com a sugestão do relatório.
max=$(jq -r .suggested_max_bytes <<<"$r")
if [[ $max =~ ^[0-9]+$ ]] && (( max <= 40 * MIB )); then
  ok "sugere --max-size $(( max / MIB )) MiB (≤ 40 MiB reais)"
else
  bad "sugestão de --max-size: $max"
fi
if "$BIN" format "$TOP" --fs exfat --max-size "$max" --allow-non-usb --yes-i-know "$TOP" >/dev/null 2>&1; then
  dev=${TOP#/dev/}
  end=$(( ( $(cat "/sys/block/$dev/${dev}p1/start") + $(cat "/sys/block/$dev/${dev}p1/size") ) * 512 ))
  if (( end <= max )) && [[ $(blkid -p -s TYPE -o value "${TOP}p1") == exfat && $(blkid -p -s PTTYPE -o value "$TOP") == dos ]]; then
    ok "format --max-size: exFAT/MBR terminando em $(( end / MIB )) MiB"
  else
    bad "partição termina em $end (máx $max)"
  fi
  mkdir -p "$WORK/fake"
  head -c 30M /dev/urandom > "$WORK/dados.bin"
  mount "${TOP}p1" "$WORK/fake" && cp "$WORK/dados.bin" "$WORK/fake/" && sync && umount "$WORK/fake"
  blockdev --flushbufs "${TOP}p1" "$TOP"; echo 3 > /proc/sys/vm/drop_caches
  mount -o ro "${TOP}p1" "$WORK/fake"
  cmp -s "$WORK/dados.bin" "$WORK/fake/dados.bin" \
    && ok "30 MB gravados no falso reformatado sobrevivem a remontar" \
    || bad "dados corrompidos no falso reformatado"
  umount "$WORK/fake"
else
  bad "format --max-size falhou"
fi

r=$(fake_check "$LOOP" --full)
expect "completo: genuíno" "$r" '.verdict == "genuine" and .bad_bytes == 0'

b=$("$BIN" bench "$LOOP" --json --sample-mb 32 | jq -c .bench || echo null)
expect "bench leitura" "$b" '.seq_read_mbps > 0 and .rand_read_iops > 0 and .seq_write_mbps == null'
b=$("$BIN" bench "$LOOP" --write --allow-non-usb --yes-i-know "$LOOP" --json --sample-mb 32 | jq -c .bench || echo null)
expect "bench escrita" "$b" '.seq_write_mbps > 0 and .rand_write_iops > 0'

# Ctrl+C no meio do teste completo: sai com erro e mensagem de cancelamento.
truncate -s 2G "$WORK/grande.img"
BIG=$(losetup -f --show "$WORK/grande.img"); FAKE_BACKS+=("$BIG")
# --foreground: sem ele o timeout repete o sinal para o grupo inteiro (= 2 Ctrl+C).
out=$(timeout --foreground -s INT 2 "$BIN" fake-check "$BIG" --full --allow-non-usb --yes-i-know "$BIG" 2>&1) \
  && bad "Ctrl+C não interrompeu" \
  || { [[ $out == *cancelado* ]] && ok "Ctrl+C cancela o teste completo" || bad "Ctrl+C: $out"; }

# ---------------------------------------------------------------- M5
# Protocolo da interface gráfica: progresso JSON no stderr, cancel pelo stdin.
lines=$("$BIN" --no-history fake-check "$LOOP" --allow-non-usb --yes-i-know "$LOOP" --json --progress-json 2>&1 >/dev/null | grep -c '"phase":"probe"' || true)
(( lines > 0 )) && ok "--progress-json emite linhas de progresso ($lines)" || bad "--progress-json sem linhas"
out=$( { sleep 1; echo cancel; } | "$BIN" --no-history fake-check "$BIG" --full --allow-non-usb --yes-i-know "$BIG" --json --progress-json --cancel-stdin 2>&1 >/dev/null ) \
  && bad "cancel pelo stdin não interrompeu" \
  || { grep -q '"error":"cancelado' <<<"$out" && ok "cancel pelo stdin + erro em JSON" || bad "cancel stdin: $(tail -1 <<<"$out")"; }
f=$("$BIN" format "$LOOP" --fs exfat -l GUI --allow-non-usb --yes-i-know "$LOOP" --json | jq -c . || echo null)
expect "format --json" "$f" '.filesystem == "exfat" and .label == "GUI"'
steps=$("$BIN" format "$LOOP" --fs fat32 --allow-non-usb --yes-i-know "$LOOP" --json --progress-json 2>&1 >/dev/null | grep -c '"phase":"format"' || true)
(( steps >= 3 )) && ok "format emite etapas no --progress-json ($steps)" || bad "format sem etapas JSON"

# ---------------------------------------------------------------- M3
make_fake bad badblock 64 64
h=$("$BIN" health "$TOP" --scan --json | jq -c . || echo null)
expect "varredura acha exatamente o bloco ilegível em 10 MiB" "$h" \
  ".scan.unreadable_blocks == 1 and .scan.unreadable_offsets == [10*$MIB]"
expect "loop sem SMART explica o motivo" "$h" '.smart.verdict == "unknown" and (.smart.notes | length) > 0'

h=$("$BIN" health "$LOOP" --scan --json | jq -c . || echo null)
expect "varredura limpa no genuíno" "$h" '.scan.unreadable_blocks == 0 and .scan.scanned_bytes > 0'

# SMART real (somente leitura) nos discos físicos desta máquina.
for disk in $(lsblk -dn -o NAME,TYPE,TRAN | awk '$2 == "disk" && $3 != "" {print $1}'); do
  h=$("$BIN" --no-history health "$disk" --json | jq -c '{v: .smart.verdict, t: .smart.device_type, m: .smart.model}' || echo null)
  expect "SMART real em $disk: $h" "$h" '.v != null and .v != "unknown"'
done

hist=$("$BIN" history --json | jq -c . || echo null)
expect "histórico registra os dispositivos testados" "$hist" 'length >= 3'
key=$(jq -r --arg size "$(( 128 * MIB ))" '.[] | select(.advertised_bytes == ($size | tonumber)) | .key' <<<"$hist" | head -1)
ev=$("$BIN" history "$key" --json | jq -c '[.events[].kind] | unique' || echo null)
expect "eventos do falso: fake_check registrado" "$ev" 'index("fake_check") != null'
if "$BIN" report "$key" -o "$WORK/laudo.html" >/dev/null && grep -q "FALSIFICADO" "$WORK/laudo.html"; then
  ok "laudo HTML do falso gerado ($(wc -c < "$WORK/laudo.html") bytes)"
else
  bad "laudo não gerado ou sem veredito"
fi

echo
echo "$pass ok, $fail falhas"
[[ $fail -eq 0 ]]
