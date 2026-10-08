# Operational patterns — what is normal, what is not

Consolidated from the 2026-10-07 host stall and the 2026-10-08 unit clean-up. The matching snapshot is
[`SYSTEM-BASELINE-2026-10-08.md`](SYSTEM-BASELINE-2026-10-08.md); regenerate with
`just ops-baseline` (read-only, no sudo).

**Status of the numbers.** "Observed" values were measured on this host on 2026-10-08. "Proposed" thresholds are
judgement calls that have not been tuned against a longer history; treat them as starting points, not facts.

## 1. Reading the snapshot

| Signal | Observed 2026-10-08 | Proposed "investigate" line | Why |
|---|---|---|---|
| Memory PSI `some avg10` | 0.00 | > 5 for a minute | memory stalls precede the hard stalls seen on 2026-10-07 |
| IO PSI `some avg10` | **77–88 (abnormal)** | > 20 sustained | root is a spinning disk (~100 random IOPS); see §3 |
| CPU PSI `some avg10` | ~1 | > 25 | 4 CPUs; cargo builds are the usual cause |
| `MemAvailable` | 13 GB of 32 GB | < 3 GB | `scripts/guard-run.sh` trips on this |
| Swap used | 6.3 GB, all on the NVMe file | any use of the HDD `/swap.img` | HDD swap should be overflow only |
| tmpfs `/tmp` | **12 of 16 GB** | > 2 GB | `/tmp` is RAM; see §3 |
| `NRestarts >= 5` | 2 historical (pi-agent 11, qwen36 6) | any *currently* restarting unit | see §2 |
| GPU memory | 20.7 of 24.6 GB (Qwen3.8 + mmproj) | n/a | the GPU is owned by `heretic-neo-coder`; do not start the qwen36 unit |

Normal listeners on loopback: 8000 (socat → 8002), 8002 (Qwen3.8), 8010 (Kroki compat), 8790 (SysML MCP).
On all interfaces: 8787 (kr0ki), 8789 (agent), 22, 111, 631. The k0s control plane (6443, 2380, 10250) and
NATS (4222) and Redis/valkey (6379) are also up.

## 2. Failure patterns seen, and the fix for each

**P1 — The restart counter is not the health signal.** `b00t-historian` showed 2,323 restarts and
`container-spire-agent` showed a restart storm in the journal, yet both had been stable for 18 hours. Check
`systemctl --user status` ("Active … since") and `journalctl --since -1h` first. `systemctl --user reset-failed <unit>`
clears the counter once you have confirmed the unit is healthy.

**P2 — A missing `ExecStart` binary loops forever.** `buildkit.service` pointed at `~/nerdctl/extras/rootless/…`,
which does not exist, exited `203/EXEC` and restarted every 2 s: 36,000 restarts. systemd's default start-rate limit
did not stop it because `RestartSec=2` spaces the starts just outside the window. For any `Restart=always` unit use
`RestartSec=5` or more, plus `StartLimitIntervalSec=300` and `StartLimitBurst=5`.
*Fixed:* the unit now runs `/usr/local/bin/containerd-rootless-setuptool.sh` (backup kept beside the scratchpad).

**P3 — A unit that needs a service the host does not wire up.** `b00t-hive-serena` runs
`kubectl apply -f …/serena.yaml`, but `~/.kube/config` has no current context. A k0s controller *is* running on this
host (`k0scontroller.service`), so the cluster exists; it is the kubeconfig that is missing. The unit retried every
30 s, 2,646 times. *Disabled for now* (`systemctl --user disable --now`). To bring it back, create an admin
kubeconfig for k0s and give the unit its own `KUBECONFIG` instead of editing your main config (commands in the PR
description).

**P4 — Expired credentials in a pre-start step.** `b00t-historian` failed `ExecStartPre=sync-nats-secrets.sh` with
`AADSTS700024: Client assertion is not within its valid time range` until the assertion was refreshed. If it
loops again, read the first error line in the journal, not the restart count.

**P5 — `Conflicts=` is applied even when the unit's own condition fails.** Test-starting the Qwen3.6 unit stopped
Qwen3.8 mid-load. Never `start` a conflicting unit "to see what happens". The boot-order cause was fixed at its
source, `~/.b00t/_b00t_/opencode.agent.toml`.

**P6 — `/tmp` is a 16 GB tmpfs, so scratch space is RAM.** 12 GB of it was a cargo `target/` directory from an
earlier Claude session. That shows up as `Shmem` (9.4 GB) in `/proc/meminfo`, shrinks the page cache, and is
swapped out when memory is tight. Put build trees under `~/.cache` or `/c0de`, never the scratchpad. Anything in
`/tmp` is also lost on reboot. The earlier scratchpad held four git *worktrees* (`b00t-pr`, `infra-pr`,
`infra-spire`, `infra-vultr`); their commits and branches live in `~/.b00t` and `~/promptexecution/infrastructure`, so
only uncommitted edits (none were present) would be lost, not the commits. Check `git status` and the `.git` file
of anything in a scratchpad before assuming either way.

**P7 — Spinning-disk root, so random reads dominate IO pressure.** With swap now on NVMe, the remaining IO PSI is
file-backed page faults against `/` (observed: `dm-0` 100 % util at ~190 random 4 KB reads/s, NVMe idle). Heavy
readers were the Kubernetes API server and its datastore, plus long-lived CLI processes. Keeping hot data
(`/var/lib/k0s`, container storage, build caches) on `/c0de` is the structural fix; it has not been done.

**P8 — The kernel log needs the `adm` group.** `/var/log/kern.log*` is `syslog:adm 0640` and `dmesg` is restricted, so
an agent running as the user cannot check for OOM kills. Either add the user to `adm` or use
`sudo grep -iE "out of memory|oom-kill|hung task|blocked for more|NVRM: Xid" /var/log/kern.log.1 /var/log/kern.log`
(use `NVRM: Xid`, not bare `Xid`: the bare form also matches the r8169 NIC line "XID 480" and overlayfs layer names).

## 3. Kernel-log findings (read 2026-10-08, `kern.log.1` + `kern.log`)

| When (UTC) | Event | Reading |
|---|---|---|
| 2026-09-29 15:42–15:46 | `containerd` tasks blocked 122–245 s, then "future hung task reports are suppressed" | disk stall; the kernel stopped reporting further ones |
| 2026-10-02 14:37–14:43 | **global OOM**: kernel killed k0s pods first (envoy-gateway, coredns, metrics-server, mlflow, kube-rbac-proxy, local-path-prov, envoy), then **`llama-server` (8.3 GB resident) in the Qwen container** | the only real out-of-memory event in the log; Kubernetes pods carry `oom_score_adj` ~1000 so they die before the model |
| 2026-10-05 09:11 and 14:28 | `containerd-shim` blocked >122 s | same disk-stall signature as 29 Sep |
| 2026-10-07 | **no OOM-kill, no hung-task, no GPU error** | see below |
| any | GPU `NVRM: Xid` | **none**; the two "XID" hits were a NIC and an overlayfs layer name |

**What this means for the 7 Oct stall.** The kernel logged nothing on 7 Oct. That fits a stall in which the machine stopped
responding without the kernel noticing an out-of-memory condition (heavy swap and IO wait), or one that ended in a hard reset
that gave the kernel no chance to write. It does *not* confirm either; it only rules out an OOM kill and a reported hung task as
the visible cause. The pattern is recurring, though: three separate disk-stall or OOM episodes in nine days, all involving
containerd/k0s on the spinning root disk, which supports moving `/var/lib/k0s` and container storage to `/c0de` (P7).

## 4. Standing rules

1. Before changing a unit, read its journal and confirm the failure is the unit's own (P1–P4).
2. Never run a `Conflicts=` unit to test it (P5).
3. Build trees and clones live on disk, not `/tmp` (P6).
4. Wrap anything memory-hungry in `scripts/guard-run.sh` (capped scope + PSI watchdog).
5. Take a baseline before and after a change: `just ops-baseline > …`.
