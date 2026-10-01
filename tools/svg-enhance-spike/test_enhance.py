import json, unittest, xml.etree.ElementTree as ET
import enhance as E

SVG = ('<svg xmlns="http://www.w3.org/2000/svg"><g class="%s"><rect x="10" y="20" width="50" height="30"/><text>%s</text></g>'
       '<g class="other"><rect x="0" y="0" width="1" height="1"/></g></svg>')
ID = "00000000-0000-4000-8000-000000000002"
KNOWN = {ID: {"type": "PartUsage", "name": "engine"}}
BRAND = json.load(open("brand.example.json"))
svg = SVG % (E.b64_class(ID), ID)

class T(unittest.TestCase):
    def test_b64_class_matches_d2s_unpadded_encoding(self):
        self.assertEqual(E.b64_class(ID), "MDAwMDAwMDAtMDAwMC00MDAwLTgwMDAtMDAwMDAwMDAwMDAy")

    def test_index_stamps_only_known_identifiers(self):
        root = ET.fromstring(svg)
        found = E.index(root, KNOWN)
        self.assertEqual(list(found), [ID])
        self.assertEqual(found[ID].get("data-kr0ki-type"), "PartUsage")
        self.assertIsNone(next(g for g in root.iter(E.Q("g")) if g.get("class") == "other").get("data-kr0ki-id"))

    def test_brand_adds_icon_class_and_restores_the_display_name_once(self):
        out, rep = E.enhance(svg, KNOWN, BRAND)
        root = ET.fromstring(out)
        g = next(g for g in root.iter(E.Q("g")) if g.get("data-kr0ki-id") == ID)
        self.assertIn("k-part", g.get("class"))
        self.assertEqual(g.find(E.Q("text")).text, "engine")
        self.assertEqual(len(g.findall(E.Q("use"))), 1)
        self.assertEqual(len(list(root.iter(E.Q("symbol")))), 3)
        self.assertEqual(rep["indexed"], 1)

    def test_idempotent(self):
        once, _ = E.enhance(svg, KNOWN, BRAND)
        twice, rep = E.enhance(once, KNOWN, BRAND)
        self.assertEqual(once, twice)
        self.assertEqual(rep["rewrites"], 0)

    def test_unindexed_ids_are_reported_not_silently_dropped(self):
        _, rep = E.enhance(svg, {**KNOWN, "missing": {"type": "PartUsage", "name": "x"}}, BRAND)
        self.assertEqual(rep["unindexed"], ["missing"])

    def test_unsupported_selectors_fail_loudly(self):
        with self.assertRaises(ValueError):
            E.apply_brand(ET.fromstring(svg), {"rules": [{"select": "g > rect"}]})

if __name__ == "__main__":
    unittest.main()
