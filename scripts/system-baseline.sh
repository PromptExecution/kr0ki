#!/usr/bin/env bash
# system-baseline.sh — read-only "what is normal on this host" snapshot, as markdown on stdout.
#
# No sudo, no writes, no network calls beyond loopback. Safe to run any time (it does not touch
# services). Redirect it to keep a dated snapshot:
#     scripts/system-baseline.sh > docs/ops/SYSTEM-BASELINE-$(date -u +%F).md
#
# Reads: /proc (meminfo, pressure, loadavg), systemd --user, podman, ss, df, nvidia-smi (optional).
# Each section degrades to "n/a" when a tool is missing. Environment values are never printed.
set -uo pipefail

have() { command -v "$1" >/dev/null 2>&1; }
section() { printf '\n## %s\n\n' "$1"; }
fence() { printf '```\n'; cat; printf '```\n'; }

printf '# System baseline — %s\n\n' "$(hostname)"
printf 'Captured %s UTC · kernel %s · %s CPUs\n' "$(date -u +%FT%TZ)" "$(uname -r)" "$(nproc)"

section "Memory and swap"
{ free -m; echo; swapon --show 2>/dev/null || echo "swapon: n/a"; echo;
  grep -E '^(Shmem|Cached|Dirty|Mapped|SwapCached):' /proc/meminfo;
  echo "vm.swappiness=$(cat /proc/sys/vm/swappiness) vm.page-cluster=$(cat /proc/sys/vm/page-cluster)"; } | fence

section "Pressure (PSI) and load"
{ for r in cpu memory io; do echo "$r: $(head -1 /proc/pressure/$r 2>/dev/null || echo n/a)"; done;
  echo "loadavg: $(cat /proc/loadavg)"; } | fence

section "tmpfs (RAM-backed; counts against memory)"
df -h -t tmpfs 2>/dev/null | awk 'NR==1 || $3!="0"' | fence

section "Disks"
df -h / /c0de 2>/dev/null | fence

section "GPU"
if have nvidia-smi; then
  nvidia-smi --query-gpu=name,memory.used,memory.total,utilization.gpu,temperature.gpu --format=csv,noheader | fence
else echo "n/a"; fi

section "Listening sockets"
# Specific host addresses are masked: tracked files must not carry machine addresses (AGENTS.md).
if have ss; then
  ss -ltnH | awk '{print $4}' | sed -E 's/^(127\.|\[::1?\]|0\.0\.0\.0|\*|\[::\]|127\.0\.0\.5)/\1/; t; s/^[0-9.]+:/<host-ip>:/' | sort -u | fence
else echo "n/a"; fi

section "systemd --user: running services"
systemctl --user list-units --type=service --state=running --no-legend --plain 2>/dev/null | awk '{print $1}' | fence

section "systemd --user: problem units (failed / restarting)"
{ systemctl --user list-units --state=failed,activating,auto-restart --no-legend --plain 2>/dev/null | awk '{print $1, $3, $4}';
  echo "--- NRestarts >= 5 (running or not):";
  for u in $(systemctl --user list-units --type=service --all --no-legend --plain 2>/dev/null | awk '{print $1}'); do
    n=$(systemctl --user show "$u" -p NRestarts --value 2>/dev/null); [ "${n:-0}" -ge 5 ] 2>/dev/null && echo "$u NRestarts=$n"
  done; } | fence

section "systemd (system): failed"
systemctl list-units --state=failed --no-legend --plain 2>/dev/null | awk '{print $1}' | fence

section "Containers"
if have podman; then podman ps --format '{{.Names}}\t{{.Status}}\t{{.Image}}' | cut -c1-140 | fence; else echo "n/a"; fi

section "Kernel log: OOM kills, hung tasks, GPU Xid"
# Needs the adm group (or root). Without it this section says so rather than reporting a false "clean".
pat='out of memory|oom-kill|hung task|blocked for more|Xid'
if [ -r /var/log/kern.log ]; then
  hits=$(grep -ihE "$pat" /var/log/kern.log /var/log/kern.log.1 2>/dev/null | tail -20)
  if [ -n "$hits" ]; then printf '%s\n' "$hits" | cut -c1-200 | fence; else echo "none in kern.log / kern.log.1"; fi
else
  echo "UNREADABLE: /var/log/kern.log is syslog:adm 0640 and this user is not in adm."
  echo
  echo 'Run: `sudo grep -iE "'"$pat"'" /var/log/kern.log.1 /var/log/kern.log`'
  echo 'Or: `sudo usermod -aG adm "$USER"` and log in again so this section can run unattended.'
fi

section "Largest resident processes"
ps -eo rss,comm --sort=-rss | head -9 | fence

printf '\n_Thresholds for reading this are in docs/ops/OPS-PATTERNS.md._\n'
