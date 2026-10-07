import unittest

import server


class PanelTest(unittest.TestCase):
    def test_a_render_panel_names_the_tool_call_that_made_it(self):
        panel = server.panel_from_tool_result("render_diagram", {"format": "d2", "source": "a -> b"}, "image/svg+xml", b"<svg/>", "call-7")
        self.assertEqual(panel["toolCallId"], "call-7")
        self.assertEqual(panel["source"]["text"], "a -> b")

    def test_the_tool_call_id_is_optional(self):
        panel = server.panel_from_tool_result("render_diagram", {"format": "d2", "source": "x"}, "image/svg+xml", b"<svg/>")
        self.assertIsNone(panel["toolCallId"])


if __name__ == "__main__":
    unittest.main()
