# System baseline — sm3llsl1k3s0ld3r

Captured 2026-10-08T07:27:27Z UTC · kernel 7.0.0-38-generic · 4 CPUs

## Memory and swap

```
               total        used        free      shared  buff/cache   available
Mem:           32038       18863         540        9043       22456       13174
Swap:          36863        6555       30308

NAME                TYPE SIZE USED PRIO
/swap.img           file   4G   0B   -1
/c0de/swap/swapfile file  32G 6.4G  100

Cached:         21339972 kB
SwapCached:       116024 kB
Dirty:              6220 kB
Mapped:          1489716 kB
Shmem:           9260160 kB
vm.swappiness=60 vm.page-cluster=0
```

## Pressure (PSI) and load

```
cpu: some avg10=1.25 avg60=1.09 avg300=1.03 total=1952094421
memory: some avg10=0.00 avg60=0.00 avg300=0.06 total=245595628
io: some avg10=83.67 avg60=79.85 avg300=68.97 total=3805792283
loadavg: 5.71 4.69 2.77 1/1195 3999723
```

## tmpfs (RAM-backed; counts against memory)

```
Filesystem      Size  Used Avail Use% Mounted on
tmpfs           3.2G  3.3M  3.2G   1% /run
tmpfs            16G  168K   16G   1% /dev/shm
tmpfs            16G   12G  4.5G  72% /tmp
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
NVIDIA GeForce RTX 3090, 20678 MiB, 24576 MiB, 0 %, 37
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
kr0ki-sysml-mcp	Up 22 hours	localhost/kr0ki-sysml-mcp:dev
kr0ki-dev-kroki	Up 22 hours	localhost/kr0ki-kroki-compat:dev
valkey	Up 20 hours (healthy)	docker.io/valkey/valkey:8-alpine
spire-agent	Up 18 hours	ghcr.io/spiffe/spire-agent:1.15.3
b00t-heretic	Up 11 hours	ghcr.io/ggml-org/llama.cpp:server-cuda
```

## Largest resident processes

```
  RSS COMMAND
5263696 llama-server
514684 kube-apiserver
423272 claude
230124 opencode
227224 systemd-journal
218088 codex
161636 java
150736 mlflow
```

_Thresholds for reading this are in docs/ops/OPS-PATTERNS.md._
