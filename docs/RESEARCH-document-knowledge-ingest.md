# RESEARCH — Document knowledge ingest, versioned fact graph, change report

**Status:** requirements + tool survey, 2026-10-07. No code. Search-snippet level: every tool below
needs a hands-on eval (see §5) before any is adopted.

## 1. Goal

Upload documents (pptx, docx, pdf, images) into a project with **no formal specification
structure**. For each version:

1. extract structure and content, including visual content (charts, diagrams, slide layout);
2. distill an ad hoc **graph of facts and knowledge**, every node/edge traced to a source passage;
3. optionally derive requirements (a thin layer, not the primary output);
4. on a new version, redo the process, then **compare and report**: summarize and trace what changed.

## 2. Where SysML v2 fits (and doesn't)

SysML v2 is the right home for *derived requirements*, their trace links and their revision
history (existing: `RequirementUsage` write path, `change_service`, `diff_managed`).
It is the wrong home for a free-form fact graph. The fact graph belongs in RDF (existing:
`rdf_store.rs`, oxigraph), with links to SysML elements where a fact supports a requirement.
The pilot server is strictly typed and drops unknown fields, so raw documents and extraction
provenance live in the evidence store; the model holds only a digest + reference.

## 3. Established patterns (no need to invent an ontology)

| Layer | Pattern | Use |
|---|---|---|
| Document structure | Docling `DoclingDocument` (or Unstructured elements) | Layout, reading order, tables, slide order, figure regions |
| Provenance | **W3C PROV-O** (`prov:wasDerivedFrom`, `wasGeneratedBy`, `Entity/Activity/Agent`) | Fact → source span → document version → extraction run (model, prompt, tool versions) |
| Fact schema | Small core vocabulary: `Entity`, `Claim`/`Statement`, `Evidence` (span + page/slide + bbox), `Concept` (SKOS-style) | Schema-guided extraction; keep the core small and let the domain layer stay open |
| Versioning | PROV snapshot chain / named graph per document version; change as first-class entity (PROV-O extensions such as PROV-STAR exist for triple-level change tracking) | Diff = set comparison across version graphs |
| Extraction | Ontology/schema-guided LLM extraction (OMD-GraphRAG, neo4j-graphrag, KGGen-style) | Schema in the prompt measurably improves triple quality |

## 4. Candidate tools (from search; unverified)

- **Docling (IBM)** — self-hostable; PDF/DOCX/PPTX/XLSX/HTML/images under one API; layout and
  reading order. Leading candidate for Box-1-style structure extraction. Python.
- **MinerU2.5**, **SmolDocling** — VLM-based parsing, for scanned or visually dense pages.
- **Unstructured** — broadest format coverage; element-typed output.
- **LlamaParse** — strong on visually complex layouts but hosted (conflicts with the
  private/air-gapped rule); exclude unless a self-hosted mode exists.
- **ColPali / ColQwen2.5 / ColQwen3** — page-image retrieval for VQA over slides and charts
  without a brittle OCR pipeline; pairs with a local VLM for answering.
- **Local VLM:** Qwen3.8 NEO-CODER with mmproj is already reachable on :8002 (vision-enabled
  2026-10-01). Whether it is good enough for slide/chart QA is **unknown** and is eval item E3.
- Graph extraction: neo4j-graphrag, LightRAG, KGGen — the pattern matters more than the library;
  none is needed if a thin schema-guided extractor over our own LLM endpoint suffices.

## 5. Requirements

**Functional**
- R1 Accept pptx, docx, pdf, png/jpg; store the original in the evidence store by SHA-256.
- R2 Extract structure (sections, slides, tables, figures with page/slide + bbox).
- R3 Visual content: answer questions about figures/charts/slides (VQA) and record the answer
  as a claim with the figure region as evidence.
- R4 Derive an ad hoc fact graph in RDF; every Claim has ≥1 Evidence (source span or bbox).
- R5 Optional: derive candidate requirements; commit to SysML v2 only through `change_service`
  (propose → validate → commit with expected revision); never auto-commit LLM output.
- R6 Re-ingest a new version of the same document; match facts across versions as
  *unchanged / modified / added / removed* with a recorded confidence.
- R7 Change report: human-readable summary plus a traceable list (fact, old evidence, new
  evidence, affected requirements/links).
- R8 Reproducibility: record model, prompt, tool versions and parameters per extraction run (PROV).

**Non-functional**
- N1 Self-hosted and air-gapped; no document content to public services.
- N2 Honour the existing auth/grant model (`model.commit`, audit log); new capability for ingest.
- N3 LLM output is schema-validated before it is stored; invalid output is rejected, not repaired silently.
- N4 Bounded resources (file size, page count, timeout), as for the behavior extractor.
- N5 Extraction runs outside the HTTP process (sidecar, like SysMD/SysML-MCP).

**Open questions**
- Q1 Identity across versions without numbering: embedding + structural-anchor matching, or LLM
  adjudication of candidate pairs? (Riskiest piece; spike first.)
- Q2 Core fact vocabulary: how small can it be before the graph is unusable for the change report?
- Q3 Is the local VLM adequate for slide/chart VQA, or does it need a dedicated model?
- Q4 Where does the fact graph persist (per-request oxigraph today; this needs a durable store)?

## 6. Evaluation plan (research tasks)

- E1 Run Docling, Unstructured and MinerU on 5–10 real pptx/docx files; score structure fidelity,
  figure/table capture and runtime. Pick one.
- E2 Take two real versions of one document; hand-label expected facts and changes (small gold set).
- E3 Local VLM on slide/chart VQA over the gold set; compare with ColQwen retrieval + VLM.
- E4 Schema-guided extraction vs. unguided, scored against the gold set.
- E5 Version matching strategies (Q1) scored on the gold set's changed/unchanged facts.

## 7. Suggested decomposition

(a) ingest + structure extraction, (b) fact/claim extraction with provenance, (c) cross-version
matching, (d) change report, (e) optional requirement derivation into SysML v2. Build (c)'s spike
alongside (a), since it decides whether (d) is trustworthy.

## Sources

- Docling / parser comparison: https://www.firecrawl.dev/blog/best-pdf-parsers , https://arxiv.org/pdf/2503.11576 , https://arxiv.org/pdf/2604.08538
- Ontology-guided KG extraction: https://arxiv.org/pdf/2603.25152 , https://arxiv.org/pdf/2510.20345
- KG provenance/versioning: https://arxiv.org/pdf/2210.02534 , https://arxiv.org/pdf/2305.08477
- Visual document retrieval: https://decodingml.substack.com/p/the-king-of-multi-modal-rag-colpali
