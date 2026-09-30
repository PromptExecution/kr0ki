# Requirement Template

## REQ-XXX: [Requirement Title]

**ID:** REQ-XXX  
**Priority:** Must | Should | Could | Won't  
**Category:** Functional | Non-Functional | Interface | Constraint  
**Status:** Draft | Review | Validated | Gated | Implemented  
**Created:** YYYY-MM-DD  
**Updated:** YYYY-MM-DD  

---

### Description

[Clear, concise description of what the system shall do. Use "shall" for mandatory requirements, "should" for desirable ones.]

**Example:**
The system shall accept diagram source code in D2 format and render it as an SVG image within 2 seconds for diagrams under 100 nodes.

---

### Acceptance Criteria

- [ ] [Specific, measurable criterion that can be tested]
- [ ] [Another testable criterion]
- [ ] [Performance/quality criterion with specific metrics]

**Example:**
- [ ] System accepts valid D2 source code via POST request
- [ ] System returns SVG image with correct Content-Type header
- [ ] Render time is under 2 seconds for diagrams with < 100 nodes
- [ ] System returns appropriate error for invalid D2 syntax

---

### Rationale

[Why this requirement exists. Link to business goals, user needs, or system constraints.]

**Example:**
Users need fast feedback when iterating on diagram designs. The 2-second target ensures interactive responsiveness while remaining achievable with current rendering infrastructure.

---

### Dependencies

- [Related requirement IDs]
- [External systems or services]
- [Technical constraints]

**Example:**
- Depends on: REQ-001 (Diagram source validation)
- Requires: Kroki backend service availability
- Constrained by: NFR-003 (Memory usage < 512MB per render)

---

### Notes

[Any additional context, implementation hints, or open questions.]

---

## Validation Checklist

Before gating this requirement:

- [ ] Requirement is unambiguous and testable
- [ ] Acceptance criteria are specific and measurable
- [ ] Rationale aligns with system goals
- [ ] Dependencies are identified and resolved
- [ ] No conflicts with existing requirements
- [ ] Stakeholder review completed

---

## History

| Date | Version | Change | Author |
|------|---------|--------|--------|
| YYYY-MM-DD | 1.0 | Initial draft | [Name] |
