# System baseline — sm3llsl1k3s0ld3r

Captured 2026-10-08T09:38:22Z UTC · kernel 7.0.0-38-generic · 4 CPUs

## Memory and swap

```
               total        used        free      shared  buff/cache   available
Mem:           32038       10373        2180         373       20635       21664
Swap:          36863        3789       33074

NAME                TYPE SIZE USED PRIO
/swap.img           file   4G   0B   -1
/c0de/swap/swapfile file  32G 3.7G  100

Cached:         12816616 kB
SwapCached:        84672 kB
Dirty:              1332 kB
Mapped:          1462276 kB
Shmem:            382064 kB
vm.swappiness=60 vm.page-cluster=0
```

## Pressure (PSI) and load

```
cpu: some avg10=1.64 avg60=1.38 avg300=0.87 total=2049483252
memory: some avg10=0.00 avg60=0.00 avg300=0.00 total=246242973
io: some avg10=2.02 avg60=5.42 avg300=1.99 total=7832778547
loadavg: 0.68 0.52 0.75 2/1178 188267
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
NVIDIA GeForce RTX 3090, 20678 MiB, 24576 MiB, 0 %, 35
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
d35d3961e3bd9927ff06fe22628efeb4126e4f849894e83fa21aa7de6a5c1949-3f2513007f457aad.service
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
kr0ki-sysml-mcp	Up 25 hours	localhost/kr0ki-sysml-mcp:dev
kr0ki-dev-kroki	Up 25 hours	localhost/kr0ki-kroki-compat:dev
valkey	Up 22 hours (healthy)	docker.io/valkey/valkey:8-alpine
spire-agent	Up 20 hours	ghcr.io/spiffe/spire-agent:1.15.3
b00t-heretic	Up 13 hours	ghcr.io/ggml-org/llama.cpp:server-cuda
```

## Kernel log: OOM kills, hung tasks, GPU Xid

UNREADABLE: /var/log/kern.log is syslog:adm 0640 and this user is not in adm.

Run: `sudo grep -iE "out of memory|oom-kill|hung task|blocked for more|NVRM: Xid" /var/log/kern.log.1 /var/log/kern.log`
Or: `sudo usermod -aG adm "$USER"` and log in again so this section can run unattended.

## Largest resident processes

```
  RSS COMMAND
5238164 llama-server
490640 kube-apiserver
377228 claude
276680 opencode
236844 systemd-journal
220836 codex
161636 java
150736 mlflow
```

_Thresholds for reading this are in docs/ops/OPS-PATTERNS.md._
