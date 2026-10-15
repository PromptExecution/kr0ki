"""The core template must yield edges. Offline: feeds a saved model output (a real Qwen3.8 dense extraction of the
Considering.Rust.pptx excerpt) through docling-graph's converter, so no model or GPU is needed.

Run with the venv that has docling-graph: `python -m unittest discover -s tests` (from containers/kr0ki-docling).
"""
import json
import pathlib
import sys
import unittest

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

from docling_graph.core.converters.graph_converter import GraphConverter  # noqa: E402
from pydantic import ConfigDict  # noqa: E402

from kr0ki_docling.templates.core import DocumentKnowledge  # noqa: E402

OUTPUT = json.loads((HERE / "fixtures" / "assembled_root.json").read_text())


def edge_labels(model_cls):
    graph, _ = GraphConverter().pydantic_list_to_graph([model_cls.model_validate(OUTPUT)])
    labels = {}
    for _, _, data in graph.edges(data=True):
        labels[data.get("label")] = labels.get(data.get("label"), 0) + 1
    return graph, labels


class CoreTemplateEdges(unittest.TestCase):
    def test_template_yields_every_edge_kind(self):
        graph, labels = edge_labels(DocumentKnowledge)
        for kind in ("HAS_CLAIM", "HAS_ENTITY", "HAS_CONCEPT", "MENTIONS", "ABOUT", "RELATED_TO"):
            self.assertGreater(labels.get(kind, 0), 0, f"no {kind} edges: {labels}")
        self.assertGreater(graph.number_of_edges(), graph.number_of_nodes(), "a connected graph has at least as many edges as nodes")

    def test_a_component_root_drops_all_edges(self):
        """Pins WHY the root must be an entity: with is_entity=False docling-graph returns no edges at all, from valid output."""

        class ComponentRoot(DocumentKnowledge):
            model_config = ConfigDict(is_entity=False, graph_id_fields=["title"])

        _, labels = edge_labels(ComponentRoot)
        self.assertEqual(labels, {}, "docling-graph changed: a component root now produces edges; revisit the template rule")


if __name__ == "__main__":
    unittest.main()
