#!/usr/bin/env bash
# The execution sandbox (assurance part `KrOKiAssurance::ExecutionSandbox`, requirement KR-A07):
# run a command that can write ONLY to its declared writable mounts.
#
#   run-sandboxed.sh --baseline DIR [--writable HOSTDIR:/CONTAINER/PATH]... -- COMMAND [ARG...]
#
# What the command sees:
#   /baseline   the requirement baseline directory, READ-ONLY
#   /scratch    an empty tmpfs, writable, gone when the command exits (the only default writable mount)
#   /usr and the system directories, read-only; everything else does not exist
#   no network, no host environment (PATH, HOME=/scratch, nothing else), its own PID namespace
#
# The root filesystem is read-only, so a write anywhere but a declared mount fails with EROFS.
# This is the deployment component's enforcement; kr0ki displays its relationships and its
# evidence (tests/assurance_a07.rs), it does not enforce it. The same policy in a pod is
# `readOnlyRootFilesystem: true`, the baseline as a `readOnly: true` volumeMount, an
# `emptyDir` for /scratch, and a deny-all NetworkPolicy.
set -euo pipefail

baseline=""
writable=()
while [ $# -gt 0 ]; do
  case "$1" in
    --baseline) baseline="${2:?--baseline needs a directory}"; shift 2 ;;
    --writable) writable+=("${2:?--writable needs HOSTDIR:/PATH}"); shift 2 ;;
    --) shift; break ;;
    *) echo "run-sandboxed: unknown argument: $1" >&2; exit 2 ;;
  esac
done
[ -n "$baseline" ] || { echo "run-sandboxed: --baseline is required" >&2; exit 2; }
[ -d "$baseline" ] || { echo "run-sandboxed: baseline directory not found: $baseline" >&2; exit 2; }
[ $# -gt 0 ] || { echo "run-sandboxed: no command given (put it after --)" >&2; exit 2; }
command -v bwrap >/dev/null || { echo "run-sandboxed: bubblewrap (bwrap) is not installed" >&2; exit 127; }

args=(--unshare-all --die-with-parent --new-session --clearenv
      --setenv PATH /usr/bin:/bin --setenv HOME /scratch
      --proc /proc --dev /dev)

# System directories, read-only. Usually /bin, /lib, /lib64, /sbin are symlinks into /usr.
args+=(--ro-bind /usr /usr)
for d in bin lib lib64 sbin; do
  if [ -L "/$d" ]; then args+=(--symlink "$(readlink "/$d")" "/$d")
  elif [ -d "/$d" ]; then args+=(--ro-bind "/$d" "/$d"); fi
done

args+=(--ro-bind "$(cd "$baseline" && pwd -P)" /baseline --tmpfs /scratch)

for w in "${writable[@]+"${writable[@]}"}"; do
  host="${w%%:*}"; dest="${w#*:}"
  case "$dest" in
    /baseline*|/usr*|/proc*|/dev*|"") echo "run-sandboxed: refusing to make $dest writable" >&2; exit 2 ;;
  esac
  [ -d "$host" ] || { echo "run-sandboxed: writable host directory not found: $host" >&2; exit 2; }
  args+=(--bind "$(cd "$host" && pwd -P)" "$dest")
done

# Last: make the root and /dev (a separate tmpfs) read-only. Mounts made above (/scratch,
# --writable) stay writable; device nodes such as /dev/null stay usable.
args+=(--remount-ro / --remount-ro /dev --chdir /scratch)
exec bwrap "${args[@]}" -- "$@"
