# SysMD sidecar

[tukcps/SysMD](https://github.com/tukcps/SysMD) is an interval constraint solver for SysML v2 / KerML (AADD-based; Kotlin, JVM, Spring Boot).
kr0ki runs it as an **optional black-box sidecar**: it sends model text over REST and reads back solved variables. No SysMD code is linked, and
nothing in kr0ki depends on it being there (`/sysmd/solve` answers 503 `sysmd_not_configured` without `KR0KI_SYSMD_URL`).

```bash
just sysmd-image      # clone PromptExecution/SysMD@kr0ki/sidecar, gradle bootJar (JDK 25, ~7 min, run ALONE), podman build
just sysmd-up         # 127.0.0.1:8081, read-only root, 1 GiB, cap-drop ALL
just test-live-sysmd  # the solver client against the running sidecar
KR0KI_SYSMD_URL=http://127.0.0.1:8081 just run
curl -X POST 'localhost:8787/sysmd/solve?language=sysml' --data-binary \
  'attribute l: ISQ::LengthValue = 0.1 .. 0.3 [m]; attribute w: ISQ::LengthValue = l * 2.0;'
```

## What comes back
`SolveReport { verdict, issues, iterations, variables[] }`. Each variable has SysMD's own `raw` string, a parsed `range`, and (when the range is
bounded and the unit maps) a `ufo_types::quantity::Quantity`.

* `verdict`: `consistent`; `inconsistent` (a `WARN_INCONSISTENCY`: a constraint or dependency cannot hold, so the values are **not** a solution);
  `error` (an `ERROR*` issue: the model did not parse or analyse). A model SysMD rejects is a report, not an HTTP error.
* `range.kind`: `bounded` | `unbounded` | `empty` | `not_a_number` | `other` (`true`, `Unknown`, dates). An unbounded or empty range is never a number.
* The standard library SysMD loads into every session, and its `a/2/1` bookkeeping variables, are not returned.

## Why the numbers are widened (soundness)
SysMD prints values for people (`Representer`, 5 decimals of the mantissa), so the string loses information. Found in `Representer.kt` and
confirmed live on 4.3.0 (2026-10-05): `2.000001` prints `2`; `0..1e-7` prints `0` (a range whose bounds are within 1e-5, or both under 5e-6, prints
only its lower bound); `1..1e9` prints `0..1e9`. `sysmd_client::parse_value` therefore **widens every printed bound outward** so the interval
contains the true range. Cost: an exact `6` comes back as roughly `[5.99997, 6.00003]`. That is the honest precision of what SysMD reports.
Values are always in SI base units (`5 mm` -> `0.005 m`, `20 degC` -> `293.15 K`); units are mapped by dimension (`kg m / s^2`, `1 / s`, `m^2`) and an
unmappable unit is reported (`unit_problem`), not guessed.

## Our patches to SysMD (fork `PromptExecution/SysMD`, branch `kr0ki/sidecar`)
| Branch | Fix |
|---|---|
| `fix/toolchain-resolver` | `jvmToolchain(25)` could not be provisioned on a JRE-only machine |
| `fix/currency-distinct-units` | all currencies were canonicalised to EUR, so `5 USD + 5 EUR` summed |
| `fix/rest-session-libraries` | `POST /session` made a session with **no standard library**, so every model failed with `NullPointerException` (the Notebook UI passes the libraries; REST did not). Has a regression test that fails without the fix |
| `feat/rest-delete-session` | no way to release a REST session (documented in the controller, never implemented); each holds the whole library in memory |

Upstream is on the CPS chair's GitLab (GitHub is a mirror), so patches go by emailed diff; the formatted patches are kept with the fork branches.

## Limits
* SysMD writes each project as files under `/app/SysMD` (a tmpfs here, so nothing persists across restarts).
* `POST /session` / `PUT /session/model` take no auth: keep it on loopback (the unit and `just sysmd-up` bind `127.0.0.1`).
* kr0ki solves at most 2 models at once and deletes each project and session; the container memory limit and `Restart=on-failure` are the backstop.
* SysMD's own `Variable` values are only the *solved range*; kr0ki does not feed solutions back into `ufo-types` models yet.
