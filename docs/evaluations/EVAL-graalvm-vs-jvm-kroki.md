# EVAL — GraalVM CE vs Temurin for the Kroki render backend

**Date:** 2026-10-07 · **Branch:** `feat/graalvm-kroki-eval` · **Verdict:** keep Temurin. GraalVM CE is slower and larger on this workload.
Raw data: [`evidence/graalvm-vs-jvm-kroki-21.json`](evidence/graalvm-vs-jvm-kroki-21.json), [`evidence/graalvm-vs-jvm-kroki-25.json`](evidence/graalvm-vs-jvm-kroki-25.json).
Harness: `scripts/bench_kroki_jvm.py` · image variants: `containers/kroki-compat/Containerfile.jdk-swap`, `Containerfile.jdk-dir`.

## Question

Would swapping the JVM under the Kroki backend (`kr0ki-kroki-compat`) for GraalVM reduce memory or improve throughput? The
motivation was memory and resource behaviour after the 2026-10-07 host stall, and a possible Java/Rust/WASM interop story.

## Method

- **Only the JDK changes.** The production image is rebuilt with `/opt/java/openjdk` replaced. Kroki's entrypoint, `PATH` and
  `plantuml-java` all resolve `java` from there, so the Kroki server and the per-request PlantUML JVM move together.
- Production container limits: `--memory=2g --memory-swap=2g --cpus=1`, `KROKI_SAFE_MODE=secure`. Fresh container per run.
- Workload per run: time to `/health`, idle memory, one cold request per engine, then 90 sequential renders and 90 renders on
  4 threads. The mix is D2 / Graphviz / PlantUML in equal shares. Memory is `podman stats` plus the cgroup's `memory.peak`.
- 3 rounds, variants interleaved so drift hits both equally. The table shows medians. Run under `scripts/guard-run.sh`: the host
  never dropped below 25 GiB available and memory pressure stayed 0.00 %.

## Results

| JDK | ready s | idle MiB | after load MiB | cgroup peak MiB | 4-thread rps | p50 ms | p95 ms | cold D2 / Graphviz / PlantUML ms |
|---|---|---|---|---|---|---|---|---|
| Temurin 21.0.9 (production) | 3.5 | 92 | 140 | **214** | **1.9** | **1452** | **5272** | 832 / 100 / 1580 |
| GraalVM CE 21.0.2 | 3.7 | 128 | 136 | 272 | 1.6 | 1790 | 6588 | 429 / 65 / 1902 |
| Temurin 25.0.4.1 | 3.2 | 86 | 96 | **150** | **2.0** | **1343** | **4892** | 382 / 60 / 1333 |
| GraalVM CE 25.0.4.1.1 | 3.8 | 107 | 101 | 196 | 1.6 | 1743 | 6286 | 352 / 89 / 1709 |

Both pairs point the same way: Temurin has about 19–25 % more throughput, about 20–22 % lower p95 latency and 46–58 MiB lower
peak memory. GraalVM's only advantage is a faster cold first request on D2 at JDK 21 (429 vs 832 ms) that does not carry over
to Graphviz or PlantUML. At JDK 25 the cold D2 numbers are within noise (352 vs 382 ms).

## Why, and what this does not show

- The mix is dominated by PlantUML, which starts a new JVM per request on 1 CPU. That favours the runtime with the cheaper JVM
  start and warm-up; HotSpot C2 with CDS wins that here. A workload of long-lived, hot Java code could differ.
- The 21 pair is not patch-matched (GraalVM CE 21.0.2 is seven months older than Temurin 21.0.9). The 25 pair is (25.0.4.1 vs
  25.0.4.1.1). The conclusion does not change between them.
- GraalVM **Community** only, as a drop-in JDK. **Not tested:** Oracle GraalVM, Native Image (Kroki and PlantUML lean on
  reflection and `ServiceLoader`, which makes a native build a project of its own), and GraalVM's polyglot/Truffle features.
  The Java/Rust/WASM interop motivation is therefore **not evaluated** here: swapping the JDK under an unmodified Kroki
  cannot show it. If a concrete embedding use appears (for example running WASM inside the JVM), evaluate that as its own
  candidate.
- Single machine, 4 cores, one workload shape, 3 rounds. Round 0 of Temurin 25 is a visible outlier (cold page cache); medians absorb it.

## A constraint worth keeping: x86-64-v3

The official `ghcr.io/graalvm/jdk-community:25` image is built on Oracle Linux 10 and fails on this node with
`Fatal glibc error: CPU does not support x86-64-v3`. The node deliberately exposes only x86-64-v2 (the same reason PlantUML runs
from its JAR). The GraalVM 25 **JDK tarball** itself runs fine on Kroki's Ubuntu base, so it was used instead (verified by
sha256, `Containerfile.jdk-dir`). Any future vendor image on this node needs checking against v2.

## Related measurements from the same investigation (host stall, 2026-10-07)

- **Docling memory** (CPU-only PyTorch, capped cgroup, `scripts/guard-run.sh`-style limits): PPTX (67 slides) 0.9 s / **440 MiB**
  peak; 43-page PDF, 4 threads **730 s / 3.0 GiB**; same PDF with 2 threads, batch 1, 3 GB cap and 200 % CPU quota 1059 s / 3.0 GiB.
  Restricting threads trades time for nothing: memory stays at about 3 GiB. The levers that work are a cgroup cap, one document
  at a time, and running the converter on another host or GPU.
- **Host:** `sar` shows available memory falling from 11 GB to about 1.5 GB between 06:30 and 07:02, the 4 GB HDD swap full all
  morning, load average 59–68, and the journal stalling about 20 minutes. Root, the journal and the swap all sit on a spinning
  disk; `/c0de` is NVMe. `scripts/setup-nvme-swap.sh` (dry run by default) moves swap to the NVMe with higher priority and keeps the
  HDD swap as overflow.
- **Rust tests under the guard:** `cargo test --workspace` 819 passed, 0 failed, 22 ignored; host minimum 22.4 GiB available.

## Reproduce

```bash
podman build --memory=2g -t localhost/kr0ki-kroki-compat:temurin25 --build-arg JDK_IMAGE=docker.io/library/eclipse-temurin:25-jre \
  -f containers/kroki-compat/Containerfile.jdk-swap .
podman build --memory=2g -t localhost/kr0ki-kroki-compat:graalvm25 --build-context jdkdir=/path/to/unpacked/graalvm-jdk \
  -f containers/kroki-compat/Containerfile.jdk-dir .
scripts/guard-run.sh --mem 4G --min-avail-gib 6 -- scripts/bench_kroki_jvm.py \
  temurin25=localhost/kr0ki-kroki-compat:temurin25 graalvm25=localhost/kr0ki-kroki-compat:graalvm25 --rounds 3
```
