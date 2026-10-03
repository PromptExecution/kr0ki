import shutil
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import server


class MbseSkillTest(unittest.TestCase):
    """Drafted by Qwen3.8 via pi (docs/PATTERN-pi-subagents.md); imports, naming and the mutation check are ours."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.patcher = patch.object(server, "MBSE_SKILLS_DIR", self.tmp)
        self.patcher.start()

    def tearDown(self):
        self.patcher.stop()
        shutil.rmtree(self.tmp)

    def test_confidence_included(self):
        skill = self.tmp / "reqwriting.md"
        skill.write_text("---\nname: reqwriting\nconfidence: recalled (2)\n---\nAsk clarifying questions.")
        result = server.mbse_skill("reqwriting")
        self.assertTrue(result.startswith("[confidence: recalled (2)]"))
        self.assertNotIn("---", result)
        self.assertNotIn("name:", result)

    def test_no_frontmatter(self):
        skill = self.tmp / "plain.md"
        skill.write_text("  Just plain advice.  ")
        result = server.mbse_skill("plain")
        self.assertEqual(result, "Just plain advice.")
        self.assertFalse(result.startswith("[confidence:"))

    def test_unknown_returns_none(self):
        self.assertIsNone(server.mbse_skill("does-not-exist"))

    def test_path_traversal_rejected(self):
        # a real file just outside the skills directory: without name validation "../outside" would read it
        outside = self.tmp.parent / "kr0ki-outside-skill.md"
        outside.write_text("secret")
        self.addCleanup(outside.unlink)
        (self.tmp / "sub").mkdir()
        (self.tmp / "sub" / "inner.md").write_text("nested")
        for name in ["../kr0ki-outside-skill", "sub/inner", "..", ""]:
            self.assertIsNone(server.mbse_skill(name), name)

    def test_max_chars_cap(self):
        skill = self.tmp / "long.md"
        skill.write_text("x" * (server.MAX_SKILL_CHARS + 100))
        result = server.mbse_skill("long")
        self.assertEqual(len(result), server.MAX_SKILL_CHARS)


if __name__ == "__main__":
    unittest.main()
