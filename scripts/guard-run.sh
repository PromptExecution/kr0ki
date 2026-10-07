#!/usr/bin/env bash
# Run a command inside a capped systemd scope and watch the HOST while it runs; stop the job before it can starve the machine.
#
#   scripts/guard-run.sh [--mem 10G] [--cpu 300] [--min-avail-gib 4] [--log guard.csv] -- cargo test --workspace
#
# - the job is capped by cgroup (MemoryMax, no swap for the job, CPUQuota), so the kernel kills IT, not the host's other services;
# - a sampler writes one CSV row every 2 s: time, MemAvailable, swap used, memory PSI (some/full avg10), CPU PSI, load1, job RSS;
# - if MemAvailable drops under --min-avail-gib, or memory PSI "full" avg10 exceeds 10 %, the scope is stopped and the exit code is 99.
# Born from the 2026-10-07 stall: the host sat at ~2 GiB available for two hours with the HDD swap full, then stopped.
set -uo pipefail

MEM=10G; CPU=300; MIN_AVAIL_GIB=4; LOG="guard-$(date +%Y%m%d-%H%M%S).csv"
while [ $# -gt 0 ]; do
  case "$1" in
    --mem) MEM=$2; shift 2;; --cpu) CPU=$2; shift 2;; --min-avail-gib) MIN_AVAIL_GIB=$2; shift 2;;
    --log) LOG=$2; shift 2;; --) shift; break;; *) break;;
  esac
done
[ $# -gt 0 ] || { sed -n '2,12p' "$0"; exit 2; }

UNIT="kr0ki-guard-$$"
echo "time,mem_avail_mib,swap_used_mib,psi_mem_some10,psi_mem_full10,psi_cpu_some10,load1,job_mem_mib" > "$LOG"

sample() {
  local avail swapt swapf some full cpu load job
  avail=$(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo)
  swapt=$(awk '/SwapTotal/{t=$2}/SwapFree/{f=$2}END{print int((t-f)/1024)}' /proc/meminfo)
  some=$(awk '/^some/{split($2,a,"=");print a[2]}' /proc/pressure/memory)
  full=$(awk '/^full/{split($2,a,"=");print a[2]}' /proc/pressure/memory)
  cpu=$(awk '/^some/{split($2,a,"=");print a[2]}' /proc/pressure/cpu)
  load=$(cut -d' ' -f1 /proc/loadavg)
  job=$(systemctl --user show "$UNIT.scope" -p MemoryCurrent --value 2>/dev/null)
  case "$job" in ''|*[!0-9]*) job=0;; esac
  echo "$(date +%T),$avail,$swapt,$some,$full,$cpu,$load,$((job/1048576))" >> "$LOG"
  echo "$avail $full"
}

( # watchdog
  while sleep 2; do
    read -r avail full < <(sample)
    if [ "$avail" -lt $((MIN_AVAIL_GIB*1024)) ] || awk "BEGIN{exit !($full>10)}"; then
      echo "guard-run: host squeezed (MemAvailable=${avail}MiB, mem PSI full10=${full}%) -> stopping $UNIT" >&2
      touch "$LOG.tripped"; systemctl --user stop "$UNIT.scope" 2>/dev/null; exit 0
    fi
  done
) &
WD=$!
trap 'kill $WD 2>/dev/null' EXIT

systemd-run --user --scope --quiet --unit="$UNIT" -p MemoryMax="$MEM" -p MemorySwapMax=0 -p CPUQuota="${CPU}%" -- "$@"
RC=$?
kill $WD 2>/dev/null; wait $WD 2>/dev/null
if [ -e "$LOG.tripped" ]; then rm -f "$LOG.tripped"; echo "guard-run: tripped; log: $LOG" >&2; exit 99; fi
awk -F, 'NR>1{ if($2<min||min==""){min=$2}; if($3>sw){sw=$3}; if($8>jm){jm=$8}; if($5>pf){pf=$5} }
         END{printf "guard-run: min MemAvailable %d MiB | max swap used %d MiB | max job mem %d MiB | max mem PSI full10 %.2f%% | log %s\n", min, sw, jm, pf, "'"$LOG"'"}' "$LOG" >&2
exit $RC
