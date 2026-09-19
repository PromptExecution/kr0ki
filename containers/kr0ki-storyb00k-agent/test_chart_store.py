import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import chart_store


class ChartStoreTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="kr0ki-charts-test-")
        self.charts = Path(self.tmp) / "charts"
        patcher = patch.object(chart_store, "CHARTS_DIR", self.charts)
        patcher.start()
        self.addCleanup(patcher.stop)
        # Deterministic jj availability per test
        self.jj_patch = patch.object(chart_store, "jj_available", return_value=False)
        self.jj_patch.start()
        self.addCleanup(self.jj_patch.stop)

    def test_save_and_load_round_trip(self):
        graph = {"version": 1, "activeId": "n1", "nodes": [{"id": "n1", "source": "a -> b"}]}
        result = chart_store.save_chart("t1", graph, "a -> b", "d2")
        self.assertIsNone(result.get("error"))
        loaded = chart_store.load_chart("t1")
        self.assertEqual(loaded["nodes"][0]["source"], "a -> b")
        self.assertTrue((self.charts / "t1" / "diagram.d2").is_file())

    def test_extension_mapping(self):
        chart_store.save_chart("t-mermaid", {"version": 1, "nodes": []}, "graph TD", "mermaid")
        self.assertTrue((self.charts / "t-mermaid" / "diagram.mmd").is_file())
        chart_store.save_chart("t-k8s", {"version": 1, "nodes": []}, "apiVersion: v1", "k8s-topology")
        self.assertTrue((self.charts / "t-k8s" / "diagram.yaml").is_file())

    def test_thread_traversal_rejected(self):
        with self.assertRaises(ValueError):
            chart_store.save_chart("../escape", {"version": 1, "nodes": []}, "x", "d2")
        with self.assertRaises(ValueError):
            chart_store.save_chart("a/b", {"version": 1, "nodes": []}, "x", "d2")

    def test_oversized_source_rejected(self):
        with patch.object(chart_store, "MAX_SOURCE_CHARS", 10):
            with self.assertRaises(ValueError):
                chart_store.save_chart("t-big", {"version": 1, "nodes": []}, "x" * 100, "d2")

    def test_list_charts(self):
        chart_store.save_chart("alpha", {"version": 1, "activeId": "a", "nodes": [{"id": "a"}]}, "s", "d2")
        chart_store.save_chart("beta", {"version": 1, "activeId": "b", "nodes": [{"id": "1"}, {"id": "2"}]}, "s", "d2")
        names = {c["thread"] for c in chart_store.list_charts()}
        self.assertIn("alpha", names)
        self.assertIn("beta", names)
        beta = next(c for c in chart_store.list_charts() if c["thread"] == "beta")
        self.assertEqual(beta["revisions"], 2)

    def test_history_empty_without_jj(self):
        chart_store.save_chart("t-hist", {"version": 1, "nodes": []}, "s", "d2")
        self.assertEqual(chart_store.history("t-hist"), [])

    def test_delete_chart(self):
        chart_store.save_chart("t-del", {"version": 1, "nodes": []}, "s", "d2")
        self.assertTrue(chart_store.delete_chart("t-del"))
        self.assertIsNone(chart_store.load_chart("t-del"))


class ChartStoreWithJjTest(unittest.TestCase):
    """Real jj integration (skipped when jj isn't installed)."""

    def setUp(self):
        import shutil
        if shutil.which("jj") is None:
            self.skipTest("jj not installed")
        self.tmp = tempfile.mkdtemp(prefix="kr0ki-charts-jj-")
        self.charts = Path(self.tmp) / "charts"
        patcher = patch.object(chart_store, "CHARTS_DIR", self.charts)
        patcher.start()
        self.addCleanup(patcher.stop)

    def test_save_snapshots_with_jj(self):
        graph = {"version": 1, "activeId": "n1", "nodes": [{"id": "n1"}]}
        result = chart_store.save_chart("jj-thread", graph, "a -> b", "d2", description="first")
        self.assertTrue(result["jj"], f"expected jj snapshot: {result}")
        # Second save: new snapshot with a different description.
        result2 = chart_store.save_chart("jj-thread", graph, "a -> b -> c", "d2", description="second")
        self.assertTrue(result2["jj"])
        entries = chart_store.history("jj-thread")
        self.assertTrue(any("second" in e for e in entries), f"history: {entries}")

    def test_restore_time_travels(self):
        chart_store.save_chart("jj-restore", {"version": 1, "nodes": []}, "v1", "d2", description="v1")
        chart_store.save_chart("jj-restore", {"version": 1, "nodes": []}, "v2 changed", "d2", description="v2")
        log = chart_store.history("jj-restore")  # newest first
        # Pick the real v1 commit by description — jj's virtual root
        # (000000000000, empty description) must never be restored from, as it
        # predates the workspace files.
        v1_line = next((line for line in log if line.endswith("v1")), None)
        self.assertTrue(v1_line, f"v1 snapshot missing from history: {log}")
        commit_id = v1_line.split()[0]
        result = chart_store.restore("jj-restore", commit_id)
        self.assertTrue(result["ok"], f"restore: {result}")
        restored = (self.charts / "jj-restore" / "diagram.d2").read_text()
        self.assertEqual(restored, "v1")


if __name__ == "__main__":
    unittest.main()
