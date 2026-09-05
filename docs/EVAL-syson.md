# EVAL — Eclipse SysON

**Evaluated:** 2026-09-05 · **For:** kr0ki — (a) reference oracle for SysML v2 notation,
(b) optional upstream authoring front-end, (c) competitor check. `PLAN-KR0KI-002`.

---

## What it is

**Eclipse SysON** — **EPL-2.0**, Java 21 / Spring Boot backend + React / TypeScript
frontend, built on **Sirius Web** (Obeo's web modeling platform). Persistence is
**PostgreSQL 15**. Developed by **Obeo + CEA List**, very active — daily commits,
**CalVer** releases, latest around **v2026.7.4**.

It is a browser-based SysML v2 modeling tool: a project explorer, a properties view, and
graphical diagram editors.

## Diagram surface

Four standard `ViewDefinition`s plus a table:

- **General View**
- **Interconnection View**
- **Action Flow View**
- **State Transition View**
- **Requirements Table**

Diagrams are **React Flow** canvases with **client-side `elkjs` (ELK) layout**. Image
export is client-side via **`html-to-image`**. There is **no headless render API** — to
get an image out of SysON without a browser you must drive a real browser with
Playwright/Puppeteer.

## OMG REST API coverage

SysON exposes a **partial** OMG Systems Modeling API at **`/api/rest/`**. It is
**read-mostly**:

- one **commit** and one **branch** per project (no real version graph);
- element **navigation** works (projects, elements, relationships, roots);
- element **mutation** through the REST API is limited — authoring is expected to happen
  in the web UI or via the textual path.
- **No `Link` header pagination** on the element collection (kr0ki's client handles this
  with its full-page/`page-size` fallback heuristic — see `kr0ki-sysmlv2-client` docs).

### Import / export

- **No SysML v2 Standard JSON** (the OMG interchange JSON) import or export.
- Textual **`.sysml`** import/export via Sensmetry's **SySIDE** (`syside-cli.js` run as a
  subprocess). This covers a **growing subset** of the language, not all of it.

## Verdict for kr0ki

### (a) Reference oracle — **strong**

SysON is the **best open-source implementation of standard SysML v2 notation** (CEA List
backing gives it real spec-compliance discipline). Use it to check that kr0ki's rendered
views are *structurally* right — same elements, same containment, same connections.

**Caveat:** visual parity is hard. SysON lays out with client-side ELK; kr0ki will lay
out differently. **Compare structure, not pixels.**

### (b) Optional upstream authoring front-end — **plausible, subset-limited**

A modeler could author in SysON and kr0ki could ingest either via:

- the **textual `.sysml`** path (SySIDE subset limits apply), or
- the **partial REST read API** (single commit/branch, but that is enough for a
  render-only consumer).

Not a committed dependency — just a viable source if one is wanted.

### (c) Competitor check — **not a competitor**

SysON renders **interactively, in a browser, with client-side layout and client-side
image export**. kr0ki's role is **headless, cached, referenceable artifacts** served
from a CDN. That is precisely the gap SysON does *not* fill. No overlap.

## Reusable pieces

- **`ViewUsage.exposedElement`** is valuable: it tells kr0ki *which elements belong in
  which view* directly from the model, so kr0ki's per-view projection (FR4) does not
  have to reinvent scoping heuristics.
- Java libraries `syson-sysml-metamodel`, `syson-sysml-import`, `syson-sysml-export`,
  `syson-sysml-validation` exist and are reasonably factored — but they are **JVM**, so
  not directly reusable from kr0ki's Rust.
- **EPL-2.0 is file-level copyleft**: modifications to SysON's own source files must be
  released under EPL. Consuming SysON as a running service, or reading its REST API, does
  not trigger this — only editing its files does.
