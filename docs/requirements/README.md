# Requirements Documentation

This directory contains the requirements documentation for kr0ki and related systems.

## Writing Good Requirements

Good requirements are:

1. **Unambiguous** - Clear and specific, leaving no room for interpretation
2. **Testable** - Can be verified through objective criteria
3. **Traceable** - Can be linked to design, implementation, and testing
4. **Necessary** - Essential for the system's purpose
5. **Feasible** - Can be implemented within constraints
6. **Prioritized** - Ranked by importance (MoSCoW: Must/Should/Could/Won't)

## Requirement Format

Each requirement should follow this structure:

```markdown
### REQ-XXX: [Title]

**Priority:** Must | Should | Could | Won't

**Description:**
[Clear, concise description of what the system shall do]

**Acceptance Criteria:**
- [ ] [Specific, testable criterion 1]
- [ ] [Specific, testable criterion 2]

**Rationale:**
[Why this requirement exists]

**Dependencies:**
- [Any related requirements or external dependencies]
```

## Requirement Categories

- **Functional Requirements (FR)** - What the system does
- **Non-Functional Requirements (NFR)** - How well the system does it
- **Interface Requirements (IR)** - How the system interacts with others
- **Constraint Requirements (CR)** - Limitations and restrictions

## Valigate Process

Requirements go through a **valigate** (validate + gate + date) process:

1. **Draft** - Initial requirement written
2. **Review** - Stakeholder review and feedback
3. **Validate** - Check against system goals and constraints
4. **Gate** - Approval for implementation
5. **Date** - Timestamp and version tracking

All requirements must pass through the pre-commit hook validation before being committed.
