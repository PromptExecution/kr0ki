# kr0ki-docling — SPIKE (not wired into kr0ki-server)

Throwaway-grade groundwork for the Docling sidecar (option C in issue #97): document → structure (Docling) → knowledge graph
(docling-graph, local Qwen3.8) → PROV-O. **What exists:** the extraction template and its tests. **What does not:** the HTTP
wrapper, the Containerfile, the docling-mcp entrypoint, the PROV-O mapping, kr0ki-server integration.

## Findings (2026-10-07, all measured; details in `docs/evaluations/EVAL-graalvm-vs-jvm-kroki.md` §Related measurements)

- Docling reads PPTX, including speaker notes (own `notes` layer); 67 slides in 0.9 s, 440 MiB peak. The 43-page PDF takes 730 s and
  3.0 GiB on CPU-only torch; fewer threads cost time, not memory. docling-mcp needs `DOCLING_MCP_CONVERSION_MODE=local` and only
  returns a document key from its `conversion` tools (no content-returning tool), so the product path should be a stateless HTTP wrapper.
- **docling-graph's "0 edges" cause:** `GraphConverter._create_edges_pass` returns an empty list at once when the template's
  ROOT class has `is_entity=False`. The model output was fine. Same output: 36 nodes / 0 edges vs 37 nodes / 67 edges once the root is an
  entity. Live (Qwen3.8, dense contract): 28 nodes / 53 edges. Pinned by `tests/test_template_edges.py`.
- LiteLLM needs the model as `openai/<name>` for an OpenAI-compatible server. Install CPU-only torch to keep the venv small
  (1.6 GB) and keep Docling off the GPU that the 27B model fills.

## Test

```bash
# venv with docling + docling-graph (CPU torch): see the install line in the eval doc
python -m unittest discover -s tests
```
