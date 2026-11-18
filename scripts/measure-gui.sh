#!/usr/bin/env bash
# Mede PSS (MB) e CPU (%) do usbkeeper-gui parado por N segundos.
# Conta: o processo principal (incluindo filhos já encerrados, como o lsblk
# da consulta periódica, em cutime/cstime) e os processos WebKit (persistentes).
# Filhos de vida curta não entram na lista: sumiriam entre as leituras.
# Uso: abra a interface, deixe parada e rode: scripts/measure-gui.sh [segundos]
set -euo pipefail
secs=${1:-60}
main=$(pgrep -x usbkeeper-gui | head -1) || { echo "abra o usbkeeper-gui primeiro"; exit 1; }
webkit=$(pgrep -P "$main" '^WebKit' || true)
stat_field_sum() { awk -v a="$2" -v b="$3" '{ print $a + $b }' "/proc/$1/stat"; }
ticks() {
  local t
  t=$(( $(stat_field_sum "$main" 14 15) + $(stat_field_sum "$main" 16 17) ))
  for p in $webkit; do t=$(( t + $(stat_field_sum "$p" 14 15) )); done
  echo "$t"
}
pss() {
  local t=0
  for p in "$main" $webkit; do t=$(( t + $(awk '/^Pss:/{print $2}' "/proc/$p/smaps_rollup") )); done
  echo $(( t / 1024 ))
}
hz=$(getconf CLK_TCK)
t0=$(ticks); sleep "$secs"; t1=$(ticks)
cpu=$(LC_ALL=C awk -v d=$(( t1 - t0 )) -v hz="$hz" -v s="$secs" 'BEGIN { printf "%.2f", d * 100 / hz / s }')
echo "PSS: $(pss) MB   CPU: ${cpu}%   (orçamento: < 250 MB, < 1%)"
