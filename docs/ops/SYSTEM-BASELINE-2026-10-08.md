# System baseline — sm3llsl1k3s0ld3r

Captured 2026-10-08T08:59:36Z UTC · kernel 7.0.0-38-generic · 4 CPUs

## Memory and swap

```
               total        used        free      shared  buff/cache   available
Mem:           32038       10160        2668         373       20361       21877
Swap:          36863        3858       33005

NAME                TYPE SIZE USED PRIO
/swap.img           file   4G   0B   -1
/c0de/swap/swapfile file  32G 3.8G  100

Cached:         12547860 kB
SwapCached:       122100 kB
Dirty:              1716 kB
Mapped:          1482656 kB
Shmem:            382056 kB
vm.swappiness=60 vm.page-cluster=0
```

## Pressure (PSI) and load

```
cpu: some avg10=0.80 avg60=0.98 avg300=1.00 total=2021747944
memory: some avg10=0.00 avg60=0.00 avg300=0.00 total=246204516
io: some avg10=10.80 avg60=57.07 avg300=74.69 total=7819758245
loadavg: 3.52 4.21 4.12 1/1173 75054
```

## tmpfs (RAM-backed; counts against memory)

```
Filesystem      Size  Used Avail Use% Mounted on
tmpfs           3.2G  3.3M  3.2G   1% /run
tmpfs            16G  168K   16G   1% /dev/shm
tmpfs            16G   39M   16G   1% /tmp
tmpfs           3.2G  416K  3.2G   1% /run/user/1000
```

## Disks

```
Filesystem                         Size  Used Avail Use% Mounted on
/dev/mapper/ubuntu--vg-ubuntu--lv  7.2T  4.6T  2.4T  67% /
/dev/nvme0n1p5                     468G  277G  168G  63% /c0de
```

## GPU

```
NVIDIA GeForce RTX 3090, 20678 MiB, 24576 MiB, 0 %, 36
```

## Listening sockets

```
<host-ip>:179
<host-ip>:34973
<host-ip>:50051
<host-ip>:53
0.0.0.0:111
0.0.0.0:22
0.0.0.0:631
0.0.0.0:8787
0.0.0.0:8789
*:10249
*:10250
*:10256
[::]:111
127.0.0.1:10248
127.0.0.1:10257
127.0.0.1:10259
127.0.0.1:20241
127.0.0.1:3000
127.0.0.1:33767
127.0.0.1:39043
127.0.0.1:50051
127.0.0.1:6011
127.0.0.1:6012
127.0.0.1:6013
127.0.0.1:8000
127.0.0.1:8790
127.0.0.53%lo:53
127.0.0.54:53
[::1]:6011
[::1]:6012
[::1]:6013
*:20244
[::]:22
*:2380
*:4222
[::]:631
*:6379
*:6443
*:8002
*:8010
*:8080
*:8222
[fd7a:115c:a1e0::3c01:cedc]:55066
```

## systemd --user: running services

```
b00t-historian.service
b00t-hive-hive-watchdog.service
b00t-hive-inference-heretic-neo-coder.service
b00t-hive-opencode-agent.service
buildkit.service
container-spire-agent.service
containerd.service
dbus.service
hive-b00t-relay.service
kr0ki-agent.service
kr0ki-kroki.service
kr0ki-llm-socat.service
kr0ki-server.service
kr0ki-sysml-mcp.service
valkey.service
xdg-document-portal.service
xdg-permission-store.service
```

## systemd --user: problem units (failed / restarting)

```
--- NRestarts >= 5 (running or not):
b00t-hive-inference-qwen36-35b-a3b-llamacpp.service NRestarts=6
b00t-hive-pi-agent.service NRestarts=11
```

## systemd (system): failed

```
logrotate.service
```

## Containers

```
kr0ki-sysml-mcp	Up 24 hours	localhost/kr0ki-sysml-mcp:dev
kr0ki-dev-kroki	Up 24 hours	localhost/kr0ki-kroki-compat:dev
valkey	Up 21 hours (healthy)	docker.io/valkey/valkey:8-alpine
spire-agent	Up 20 hours	ghcr.io/spiffe/spire-agent:1.15.3
b00t-heretic	Up 12 hours	ghcr.io/ggml-org/llama.cpp:server-cuda
```

## Kernel log: OOM kills, hung tasks, GPU Xid

UNREADABLE: /var/log/kern.log is syslog:adm 0640 and this user is not in adm.

Run: `sudo grep -iE "out of memory|oom-kill|hung task|blocked for more|Xid" /var/log/kern.log.1 /var/log/kern.log`
Or: `sudo usermod -aG adm "$USER"` and log in again so this section can run unattended.

## Largest resident processes

```
  RSS COMMAND
5238164 llama-server
490388 kube-apiserver
393200 claude
260212 opencode
234952 systemd-journal
218292 codex
171984 codebase-memory
161636 java
```

_Thresholds for reading this are in docs/ops/OPS-PATTERNS.md._
