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

    def test_a_large_svg_reaches_the_panel_whole(self):
        # D2 writes its connections after its shapes; cutting the SVG at the model-output limit drew the boxes and lost the lines.
        svg = ("<svg xmlns='http://www.w3.org/2000/svg'>" + "<rect/>" * 6000 + "<path class='connection'/></svg>").encode()
        self.assertGreater(len(svg), server.MAX_OUTPUT_CHARS)
        panel = server.panel_from_tool_result("render_diagram", {"format": "d2", "source": "x"}, "image/svg+xml", svg)
        self.assertEqual(panel["content"], svg.decode())

    def test_non_svg_text_output_is_still_truncated(self):
        text = ("x" * (server.MAX_OUTPUT_CHARS + 10)).encode()
        panel = server.panel_from_tool_result("render_diagram", {"format": "d2", "source": "x"}, "text/plain", text)
        self.assertIn("[truncated 10 chars]", panel["content"])


if __name__ == "__main__":
    unittest.main()
