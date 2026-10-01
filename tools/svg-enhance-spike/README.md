# svg-enhance-spike

Throwaway proof of the identifier-driven SVG enhancement layer described in
`docs/DESIGN-NOTE-skills-identity-and-svg-enhancement.md`. The *contract* (stamp `data-kr0ki-*`, then CSS-selected brand
rules, idempotent, brand as swappable JSON) is what to keep; this Python is not the implementation.

- `sysml_fixture_server.py` read-only fake OMG SysML v2 server (a small vehicle model, UUID ids)
- `enhance.py` index + brand rules; `brand.example.json` placeholder brand (not canonical)
- `test_enhance.py` `python3 -m unittest test_enhance`
