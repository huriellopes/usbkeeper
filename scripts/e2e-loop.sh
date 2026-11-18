#!/usr/bin/env bash
# Teste ponta a ponta do `format`/`eject` em um loop device (arquivo de imagem).
# Nunca toca em disco real. Precisa de root:
#   cargo build && sudo scripts/e2e-loop.sh
set -euo pipefail
[[ $EUID -eq 0 ]] || { echo "rode com sudo: sudo $0"; exit 1; }

BIN=${BIN:-$(dirname "$0")/../target/debug/usbkeeper}
WORK=$(mktemp -d)
IMG=$WORK/pen.img
truncate -s 600M "$IMG"
LOOP=$(losetup -fP --show "$IMG")
HOLDER=""
cleanup() {
  [[ -n $HOLDER ]] && kill "$HOLDER" 2>/dev/null || true
  umount "$WORK/mnt" 2>/dev/null || true
  losetup -d "$LOOP" 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT
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

echo
echo "$pass ok, $fail falhas"
[[ $fail -eq 0 ]]
