import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import project_store


class ProjectStoreTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="kr0ki-project-test-")
        patcher = patch.object(project_store, "CHARTS_DIR", Path(self.tmp) / "charts")
        patcher.start()
        self.addCleanup(patcher.stop)
        project_store._memory.clear()

    def test_goal_and_title_from_first_prompt(self):
        project_store.add_prompt("t1", "user", "Render the cluster topology\nwith annotations")
        p = project_store.get_project("t1")
        self.assertEqual(p["goal"], "Render the cluster topology\nwith annotations")
        self.assertEqual(p["title"], "Render the cluster topology")

    def test_title_not_overwritten_by_later_prompts(self):
        project_store.add_prompt("t2", "user", "First goal")
        project_store.add_prompt("t2", "user", "Second goal")
        p = project_store.get_project("t2")
        self.assertEqual(p["goal"], "First goal")
        self.assertEqual(p["prompts"][1]["role"], "user")

    def test_full_prompt_log(self):
        project_store.add_prompt("t3", "user", "hello")
        project_store.add_prompt("t3", "assistant", "hi there", run="run-1")
        p = project_store.get_project("t3")
        self.assertEqual([e["role"] for e in p["prompts"]], ["user", "assistant"])
        self.assertEqual(p["prompts"][1]["run"], "run-1")

    def test_thinking_capture(self):
        project_store.add_thinking("t4", "the user probably wants d2", run="run-9")
        p = project_store.get_project("t4")
        self.assertEqual(p["thinking"][0]["text"], "the user probably wants d2")

    def test_qa_and_lock(self):
        project_store.add_qa("t5", "Which diagram type?", "d2")
        project_store.set_requirements("t5", "A d2 diagram of the cluster.")
        p = project_store.get_project("t5")
        self.assertEqual(p["qa"][0]["answer"], "d2")
        self.assertTrue(p["locked"])
        self.assertEqual(p["requirements"], "A d2 diagram of the cluster.")

    def test_rename(self):
        project_store.add_prompt("t6", "user", "goal")
        project_store.rename("t6", "Cluster map")
        self.assertEqual(project_store.get_project("t6")["title"], "Cluster map")

    def test_persistence_round_trip(self):
        project_store.add_prompt("t7", "user", "persist me")
        project_store._memory.clear()
        p = project_store.get_project("t7", create=False)
        self.assertIsNotNone(p)
        self.assertEqual(p["prompts"][0]["text"], "persist me")

    def test_get_missing_no_create(self):
        self.assertIsNone(project_store.get_project("nope", create=False))

    def test_list_projects(self):
        project_store.add_prompt("list-a", "user", "alpha")
        project_store.add_prompt("list-b", "user", "beta")
        names = {p["threadId"] for p in project_store.list_projects()}
        self.assertIn("list-a", names)
        self.assertIn("list-b", names)

    def test_traversal_rejected(self):
        with self.assertRaises(ValueError):
            project_store.add_prompt("../evil", "user", "x")


if __name__ == "__main__":
    unittest.main()
