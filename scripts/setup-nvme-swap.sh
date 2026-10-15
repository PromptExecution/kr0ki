#!/usr/bin/env bash
# Put swap on the /c0de NVMe and demote the spinning-disk swap to overflow. Needs root. Dry run unless --apply.
#
#   sudo scripts/setup-nvme-swap.sh            # show what would change
#   sudo scripts/setup-nvme-swap.sh --apply    # do it (idempotent)
#
# Why: / and /swap.img are on an HDD (LVM on an 8 TB HGST); /c0de is a Samsung NVMe. When memory ran out on 2026-10-07 the
# swap, the journal and every service shared that one disk and the host stalled. Swap areas of EQUAL priority are filled
# round-robin, so splitting 50/50 would still send half the page traffic to the HDD. Give the NVMe file a HIGHER priority:
# the kernel fills it first and only touches the HDD swap after the NVMe file is full.
set -euo pipefail

SIZE_GIB="${SWAP_SIZE_GIB:-32}"
FILE="${SWAP_FILE:-/c0de/swap/swapfile}"
APPLY=0; [ "${1:-}" = "--apply" ] && APPLY=1

run() { echo "+ $*"; [ "$APPLY" = 1 ] && "$@" || true; }

[ "$(id -u)" = 0 ] || { echo "run as root (sudo)"; exit 1; }
mountpoint -q /c0de || { echo "/c0de is not mounted"; exit 1; }
avail_gib=$(df -BG --output=avail /c0de | tail -1 | tr -dc 0-9)
[ "$avail_gib" -gt $((SIZE_GIB + 20)) ] || { echo "only ${avail_gib}G free on /c0de; need ${SIZE_GIB}G + headroom"; exit 1; }

if swapon --show=NAME --noheadings | grep -qx "$FILE"; then
  echo "$FILE is already active; nothing to create"
else
  run mkdir -p "$(dirname "$FILE")"
  # fallocate on ext4 is fine for swap (no holes); dd would also work but is slower.
  run fallocate -l "${SIZE_GIB}G" "$FILE"
  run chmod 600 "$FILE"
  run mkswap "$FILE"
  run swapon --priority 100 "$FILE"
fi

if ! grep -q "^$FILE " /etc/fstab; then
  echo "+ append to /etc/fstab: $FILE none swap sw,pri=100 0 0"
  [ "$APPLY" = 1 ] && echo "$FILE none swap sw,pri=100 0 0" >> /etc/fstab
fi
# Demote the HDD swap to overflow-only.
if grep -qE '^/swap.img[[:space:]]+none[[:space:]]+swap[[:space:]]+sw[[:space:]]+0[[:space:]]+0' /etc/fstab; then
  echo "+ /etc/fstab: /swap.img -> sw,pri=-2"
  [ "$APPLY" = 1 ] && sed -i -E 's|^(/swap.img[[:space:]]+none[[:space:]]+swap[[:space:]]+)sw([[:space:]]+0[[:space:]]+0)|\1sw,pri=-2\2|' /etc/fstab
fi
run swapoff /swap.img
run swapon --priority -2 /swap.img

# SSD-friendly swap: no readahead of neighbouring swap pages (they are not neighbours on flash), keep default swappiness.
echo "+ sysctl vm.page-cluster=0 (persist in /etc/sysctl.d/90-kr0ki-swap.conf)"
if [ "$APPLY" = 1 ]; then
  echo "vm.page-cluster = 0" > /etc/sysctl.d/90-kr0ki-swap.conf
  sysctl -q -p /etc/sysctl.d/90-kr0ki-swap.conf
fi
echo; swapon --show
