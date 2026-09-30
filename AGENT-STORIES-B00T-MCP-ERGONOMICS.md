# Agent Stories: Improving b00t MCP Ergonomics

**Date:** 2026-09-27  
**Context:** Friction encountered while integrating Chrome DevTools MCP server  
**Goal:** Document non-idiomatic friction and propose improvements via GitHub issues

---

## Executive Summary

While integrating the official Chrome DevTools MCP server into the kr0ki project, we encountered multiple friction points in the b00t MCP workflow. These friction points represent opportunities to improve the agent experience and make the MCP ecosystem more accessible.

This document presents 6 "Agent Stories" (similar to user stories) that describe the desired behavior and acceptance criteria for each improvement. Each story is backed by a GitHub issue in the kr0ki repository.

---

## Friction Points Identified

### 1. Registry Discovery (#59)

**Friction:** Search command syntax is unclear and doesn't accept natural language queries.

**Example:**
```bash
b00t mcp registry search chrome-devtools-mcp
# Error: unexpected argument 'chrome-devtools-mcp' found
```

**Impact:** Agents must know exact datum names or use manual grep workarounds.

**Agent Story:** "As an agent trying to install an MCP server, I want to search the registry using natural language, so that I can discover available servers without knowing exact naming conventions."

---

### 2. Datum Creation & Validation (#60)

**Friction:** No clear schema documentation or validation feedback.

**Example:**
```bash
b00t mcp install chrome-devtools-mcp dotmcpjson
# Error: Failed to parse MCP config TOML
```

**Impact:** Contributors must reverse-engineer datum format from existing files.

**Agent Story:** "As an agent creating a new MCP server datum, I want clear documentation and validation of the datum schema, so that I can create valid datums without trial-and-error."

---

### 3. Configuration Overrides (#61)

**Friction:** No way to customize datum defaults during installation.

**Example:**
```bash
# Datum defines base config
b00t mcp install chrome-devtools-mcp dotmcpjson

# But project needs specific browser URL
# Must manually edit .mcp.json (breaks datum workflow)
```

**Impact:** Forces manual .mcp.json editing or datum proliferation.

**Agent Story:** "As an agent installing an MCP server with project-specific configuration, I want to override datum defaults during installation, so that I can customize server behavior without manually editing .mcp.json."

---

### 4. Dynamic Loading via b00t-mcp Proxy (#62)

**Friction:** All MCP servers must be statically configured in .mcp.json.

**Example:**
```json
{
  "mcpServers": {
    "chrome-devtools-mcp": { ... },
    "kr0ki-mcp": { ... }
  }
}
```

**Impact:** Can't use servers on-demand without pre-configuration.

**Agent Story:** "As an agent needing to use an MCP server on-demand, I want to dynamically load MCP servers via the b00t-mcp proxy, so that I don't need to pre-configure every possible server in .mcp.json."

---

### 5. Documentation & Examples (#63)

**Friction:** No comprehensive documentation with worked examples.

**Example:**
- No schema reference
- No minimal/advanced examples
- No validation command
- Inconsistent patterns across datums

**Impact:** Steep learning curve for new contributors.

**Agent Story:** "As an agent learning to create MCP datums, I want comprehensive documentation with worked examples, so that I can create valid datums without reverse-engineering existing files."

---

### 6. Error Messages & Debugging (#64)

**Friction:** Error messages are cryptic and don't suggest fixes.

**Example:**
```bash
b00t mcp install chrome-devtools-mcp dotmcpjson
# Error: Failed to parse MCP config TOML
```

**Impact:** Agents spend time debugging instead of solving problems.

**Agent Story:** "As an agent using b00t MCP commands, I want clear, actionable error messages, so that I can diagnose and fix issues without external debugging."

---

## Proposed Solutions

### Short-Term (Quick Wins)

1. **Improve Error Messages (#64)**
   - Add line numbers and field names to parse errors
   - Suggest similar names for "not found" errors
   - Link to documentation in error messages

2. **Add Validation Command (#60)**
   - `b00t mcp datum validate <file>`
   - Clear error messages with supported fields
   - Examples of valid datums

3. **Fix Search Syntax (#59)**
   - Accept positional arguments: `b00t mcp registry search <query>`
   - Support fuzzy matching
   - Show usage examples in errors

### Medium-Term (Structural Improvements)

4. **Configuration Overrides (#61)**
   - `--arg-append` and `--arg-override` flags
   - Environment variable substitution in datums
   - Configuration profiles

5. **Documentation Overhaul (#63)**
   - Schema reference document
   - Minimal and advanced examples
   - `b00t mcp datum examples` command
   - Link documentation from help text

### Long-Term (Architectural Enhancements)

6. **Dynamic Loading via b00t-mcp Proxy (#62)**
   - `b00t mcp use <server>` command
   - On-demand server loading
   - Session-scoped and persistent modes
   - Configuration inheritance

---

## Architecture Vision

### Current State

```
Agent → .mcp.json (static) → MCP Server
```

All servers must be pre-configured. No flexibility for on-demand usage.

### Future State

```
Agent → b00t-mcp proxy (dynamic) → MCP Server
              ↓
         Datum Registry
              ↓
      Configuration Overrides
```

The proxy acts as a dynamic loader, fetching datums from the registry and starting servers on-demand with appropriate configuration.

### Benefits

1. **Flexibility:** Use any server without pre-configuration
2. **Simplicity:** Smaller .mcp.json files
3. **Experimentation:** Try new servers without commitment
4. **Consistency:** Same datum-driven workflow for static and dynamic

---

## Implementation Priorities

### Phase 1: Foundation (Issues #59, #60, #64)

**Goal:** Fix immediate friction points

- [ ] Improve search command syntax
- [ ] Add datum validation command
- [ ] Enhance error messages

**Effort:** 2-3 days  
**Impact:** High (unblocks current workflow)

---

### Phase 2: Configuration (Issues #61, #63)

**Goal:** Enable flexible configuration

- [ ] Add override flags to install command
- [ ] Create comprehensive documentation
- [ ] Add examples command

**Effort:** 3-5 days  
**Impact:** Medium (improves contributor experience)

---

### Phase 3: Dynamic Loading (Issue #62)

**Goal:** Enable on-demand server usage

- [ ] Design b00t-mcp proxy architecture
- [ ] Implement `b00t mcp use` command
- [ ] Add session management
- [ ] Document dynamic loading workflow

**Effort:** 1-2 weeks  
**Impact:** High (enables new use cases)

---

## Success Metrics

### Quantitative

- **Time to install:** Reduce from 15+ minutes (with errors) to < 2 minutes
- **Datum creation success rate:** Increase from ~50% (trial-and-error) to > 90%
- **Error resolution time:** Reduce from 10+ minutes to < 1 minute

### Qualitative

- **Agent confidence:** Agents can install servers without external help
- **Contributor onboarding:** New contributors can create datums quickly
- **Ecosystem growth:** More MCP servers added to registry

---

## Related Work

### Similar Patterns

1. **npm/npx:** Dynamic package execution without global install
2. **Docker:** Image registry with dynamic pulling
3. **Homebrew:** Formula registry with validation

### b00t Ecosystem

- **b00t-mcp proxy:** Already exists for static servers
- **Datum system:** Already exists for configuration
- **Registry:** Already exists for discovery

The proposed enhancements build on existing infrastructure rather than replacing it.

### Implementation Location

**⚠️ Important:** All implementations must be done in the **b00t repository**, not kr0ki.

- **Repository:** `elasticdotventures/_b00t_` (b00t CLI source)
- **Workflow:** Implement in b00t → Submit PR to b00t → Reference kr0ki issue
- **Who:** kr0ki team can implement, b00t maintainers review/merge
- **Why:** b00t is shared infrastructure; fixes benefit the entire ecosystem

Each GitHub issue (#59-64) includes a comment clarifying this requirement.

---

## Conclusion

The friction points identified in this document represent opportunities to significantly improve the b00t MCP experience. By addressing these issues through the proposed agent stories, we can:

1. **Reduce friction** for agents and contributors
2. **Accelerate ecosystem growth** with better documentation
3. **Enable new use cases** with dynamic loading
4. **Improve overall ergonomics** of the b00t MCP workflow

Each agent story is backed by a GitHub issue with clear acceptance criteria, making it easy to track progress and measure success.

---

## GitHub Issues

- #59: Agent Story: MCP Registry Search Should Accept Natural Language Queries
- #60: Agent Story: MCP Datum Creation Should Have Clear Schema and Validation
- #61: Agent Story: MCP Installation Should Support Per-Project Configuration Overrides
- #62: Agent Story: b00t-mcp Proxy Should Support Dynamic MCP Server Loading
- #63: Agent Story: MCP Datum Documentation Should Include Worked Examples
- #64: Agent Story: MCP Commands Should Provide Actionable Error Messages

---

## Next Steps

1. **Review agent stories** with b00t maintainers
2. **Prioritize implementation** based on impact and effort
3. **Assign owners** for each phase
4. **Track progress** via GitHub issues
5. **Measure success** using defined metrics

---

**Author:** Agent-assisted development session  
**Reviewers:** b00t maintainers, kr0ki contributors  
**Status:** Proposed, pending review
