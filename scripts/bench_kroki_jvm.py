#!/usr/bin/env python3
"""NOT USED IN PRODUCTION: evaluation harness (docs/evaluations/EVAL-graalvm-vs-jvm-kroki.md); kept to reproduce those numbers.

Compare JVM builds of the Kroki render backend under the production container limits (--memory=2g --cpus=1).

Usage: scripts/bench_kroki_jvm.py label=image [label=image ...] [--rounds 3] [--requests 90] [--out result.json]

Per round and per variant (variants are interleaved so drift hits all equally): start a fresh container, time until /health
answers, sample memory, run a fixed render mix (sequential, then 4 concurrent), read the container's cgroup memory.peak.
Nothing here touches the running kr0ki-dev-kroki (:8010); containers use throwaway ports and are always removed.
"""
import argparse, json, re, statistics, subprocess, sys, threading, time, urllib.request

SRC = {
    "d2": ("d2/svg", "direction: right\n" + "\n".join(f"n{i}: Node {i}" for i in range(12)) + "\n" + "\n".join(f"n{i} -> n{i+1}: link" for i in range(11)) + "\n"),
    "graphviz": ("graphviz/svg", "digraph{rankdir=LR;" + ";".join(f"a{i}->a{i+1}" for i in range(25)) + "}"),
    "plantuml": ("plantuml/svg", "@startuml\n" + "\n".join(f"A{i%4} -> A{(i+1)%4}: message {i}" for i in range(14)) + "\n@enduml\n"),
}
MIX = ["d2", "graphviz", "plantuml"]


def sh(*a, check=True):
    return subprocess.run(a, capture_output=True, text=True, check=check).stdout.strip()


def post(port, kind):
    path, body = SRC[kind]
    t = time.perf_counter()
    req = urllib.request.Request(f"http://127.0.0.1:{port}/{path}", data=body.encode(), headers={"Content-Type": "text/plain"})
    with urllib.request.urlopen(req, timeout=60) as r:
        data = r.read()
    assert b"<svg" in data, f"{kind}: no svg"
    return time.perf_counter() - t


def mem_mib(name):
    """Current usage from `podman stats` ("205.2MB / 2.147GB"), converted to MiB."""
    first = sh("podman", "stats", "--no-stream", "--format", "{{.MemUsage}}", name).split("/")[0].strip()
    m = re.match(r"([\d.]+)\s*([kKMG]i?B)", first)
    num, unit = float(m.group(1)), m.group(2)
    factor = {"kB": 1e3, "KiB": 1024, "MB": 1e6, "MiB": 1048576, "GB": 1e9, "GiB": 1073741824}[unit]
    return num * factor / 1048576


def cg_peak_mib(name):
    for p in ("/sys/fs/cgroup/memory.peak",):
        out = sh("podman", "exec", name, "cat", p, check=False)
        if out.isdigit():
            return int(out) / 1048576
    return float("nan")


def pct(xs, q):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(round(q * (len(xs) - 1))))]


def one(label, image, port, nreq):
    name = f"kr0ki-bench-{label}"
    sh("podman", "rm", "-f", name, check=False)
    t0 = time.perf_counter()
    sh("podman", "run", "-d", "--rm", "--name", name, "--memory=2g", "--memory-swap=2g", "--cpus=1", "-p", f"127.0.0.1:{port}:8000", "-e", "KROKI_SAFE_MODE=secure", image)
    try:
        ready = None
        while time.perf_counter() - t0 < 120:
            try:
                urllib.request.urlopen(f"http://127.0.0.1:{port}/health", timeout=2).read()
                ready = time.perf_counter() - t0
                break
            except Exception:
                time.sleep(0.1)
        if ready is None:
            raise RuntimeError("not ready in 120 s")
        idle = mem_mib(name)
        first = {k: post(port, k) for k in MIX}          # cold first request per engine
        seq = {k: [] for k in MIX}
        for i in range(nreq):
            k = MIX[i % 3]
            seq[k].append(post(port, k))
        conc, lock = [], threading.Lock()
        def worker(n):
            for i in range(n):
                k = MIX[i % 3]
                d = post(port, k)
                with lock:
                    conc.append(d)
        t1 = time.perf_counter()
        ts = [threading.Thread(target=worker, args=(nreq // 4,)) for _ in range(4)]
        [t.start() for t in ts]; [t.join() for t in ts]
        wall = time.perf_counter() - t1
        after = mem_mib(name)
        return {
            "variant": label, "ready_s": ready, "idle_mib": idle, "after_load_mib": after, "cgroup_peak_mib": cg_peak_mib(name),
            "first_ms": {k: v * 1000 for k, v in first.items()},
            "seq_p50_ms": {k: pct(v, .5) * 1000 for k, v in seq.items()}, "seq_p95_ms": {k: pct(v, .95) * 1000 for k, v in seq.items()},
            "conc_throughput_rps": len(conc) / wall, "conc_p50_ms": pct(conc, .5) * 1000, "conc_p95_ms": pct(conc, .95) * 1000,
        }
    finally:
        sh("podman", "rm", "-f", name, check=False)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("variants", nargs="+")
    ap.add_argument("--rounds", type=int, default=3)
    ap.add_argument("--requests", type=int, default=90)
    ap.add_argument("--out", default="bench-result.json")
    a = ap.parse_args()
    vs = [v.split("=", 1) for v in a.variants]
    runs = []
    for r in range(a.rounds):
        for i, (label, image) in enumerate(vs):
            res = one(label, image, 18100 + i, a.requests)
            res["round"] = r
            runs.append(res)
            print(json.dumps({k: (round(v, 1) if isinstance(v, float) else v) for k, v in res.items() if k in ("variant", "round", "ready_s", "idle_mib", "after_load_mib", "cgroup_peak_mib", "conc_throughput_rps", "conc_p95_ms")}), flush=True)
    json.dump(runs, open(a.out, "w"), indent=1)
    print("\nvariant | ready s | idle MiB | after-load MiB | cgroup peak MiB | conc rps | conc p50 ms | conc p95 ms | cold d2/gv/puml ms")
    for label, _ in vs:
        rs = [x for x in runs if x["variant"] == label]
        m = lambda k: statistics.median(x[k] for x in rs)
        cold = [statistics.median(x["first_ms"][k] for x in rs) for k in MIX]
        print(f"{label} | {m('ready_s'):.1f} | {m('idle_mib'):.0f} | {m('after_load_mib'):.0f} | {m('cgroup_peak_mib'):.0f} | {m('conc_throughput_rps'):.1f} | {m('conc_p50_ms'):.0f} | {m('conc_p95_ms'):.0f} | " + "/".join(f"{c:.0f}" for c in cold))


if __name__ == "__main__":
    main()
