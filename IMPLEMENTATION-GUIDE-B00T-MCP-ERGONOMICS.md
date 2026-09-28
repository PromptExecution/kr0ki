# Implementation Guide: b00t MCP Ergonomics Improvements

**Date:** 2026-09-27  
**Status:** Ready for implementation  
**Target Repository:** `elasticdotventures/_b00t_` (b00t CLI)

---

## Overview

This guide provides instructions for the kr0ki team to implement the 6 agent stories (issues #59-64) in the b00t repository. While the issues were discovered during kr0ki development, the actual implementations must be done in b00t since it's shared infrastructure.

---

## Quick Reference

| Issue | Title | Priority | Effort | Phase |
|-------|-------|----------|--------|-------|
| #59 | Registry Search Natural Language | High | 1 day | 1 |
| #60 | Datum Schema & Validation | High | 1-2 days | 1 |
| #61 | Configuration Overrides | Medium | 2-3 days | 2 |
| #62 | Dynamic Loading via Proxy | High | 1-2 weeks | 3 |
| #63 | Documentation & Examples | Medium | 2-3 days | 2 |
| #64 | Actionable Error Messages | High | 1 day | 1 |

**Total Effort:** ~3-4 weeks (can be parallelized)

---

## Workflow

### 1. Setup

```bash
# Clone b00t repository
git clone git@github.com:elasticdotventures/_b00t_.git
cd _b00t_

# Create feature branch
git checkout -b feature/mcp-ergonomics-59

# Reference the kr0ki issue in commits
git commit -m "feat: improve registry search syntax (#59)

Implements natural language search for MCP registry.

References: PromptExecution/kr0ki#59"
```

### 2. Implementation

Each issue should be implemented as a separate PR to b00t:

- **PR #1:** Fix registry search syntax (#59)
- **PR #2:** Add datum validation (#60)
- **PR #3:** Improve error messages (#64)
- **PR #4:** Add configuration overrides (#61)
- **PR #5:** Create documentation (#63)
- **PR #6:** Implement dynamic loading (#62)

### 3. Testing

```bash
# Run b00t tests
cargo test

# Test MCP commands
b00t mcp registry search chrome
b00t mcp datum validate test.mcp.toml
b00t mcp install test-mcp dotmcpjson
```

### 4. Submission

```bash
# Push feature branch
git push origin feature/mcp-ergonomics-59

# Create PR to b00t
gh pr create --title "feat: improve registry search syntax" \
  --body "Implements natural language search for MCP registry.

## Changes
- Accept positional arguments in \`b00t mcp registry search\`
- Support fuzzy matching
- Show usage examples in errors

## Testing
- [ ] \`b00t mcp registry search chrome\` works
- [ ] Fuzzy matching returns relevant results
- [ ] Error messages include examples

## References
- Implements: PromptExecution/kr0ki#59
- Discovered during: Chrome DevTools MCP integration in kr0ki"
```

---

## Implementation Details

### Issue #59: Registry Search Natural Language

**File:** `src/mcp/registry.rs` (or similar)

**Current:**
```rust
// Requires flags, doesn't accept positional args
b00t mcp registry search --tag browser
```

**Desired:**
```rust
// Accept positional args, support fuzzy matching
b00t mcp registry search chrome-devtools
b00t mcp registry search browser automation
```

**Implementation:**
1. Update CLI parser to accept positional argument
2. Implement fuzzy matching algorithm
3. Return ranked results with relevance scores
4. Update error messages with usage examples

**Tests:**
```rust
#[test]
fn test_search_positional_arg() {
    let result = search("chrome");
    assert!(result.contains("chrome-devtools-mcp"));
}

#[test]
fn test_search_fuzzy_matching() {
    let result = search("browser");
    assert!(result.contains("chrome-devtools-mcp"));
    assert!(result.contains("playwright"));
}
```

---

### Issue #60: Datum Schema & Validation

**File:** `src/mcp/datum.rs` (or similar)

**Current:**
```rust
// No validation, cryptic errors
b00t mcp install my-server dotmcpjson
// Error: Failed to parse MCP config TOML
```

**Desired:**
```rust
// Clear validation with line numbers
b00t mcp datum validate my-server.mcp.toml
// ✗ Invalid: unknown field 'b00t.env' at line 15
// Supported fields: [b00t], [[b00t.mcp.stdio]], [[b00t.gate]]
```

**Implementation:**
1. Add `validate` subcommand to `b00t mcp datum`
2. Implement schema validation with line numbers
3. List supported fields in error messages
4. Add `b00t mcp datum schema` command

**Tests:**
```rust
#[test]
fn test_validate_valid_datum() {
    let result = validate("valid.mcp.toml");
    assert!(result.is_ok());
}

#[test]
fn test_validate_invalid_field() {
    let result = validate("invalid.mcp.toml");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("unknown field"));
}
```

---

### Issue #61: Configuration Overrides

**File:** `src/mcp/install.rs` (or similar)

**Current:**
```rust
// No way to override args during installation
b00t mcp install chrome-devtools-mcp dotmcpjson
```

**Desired:**
```rust
// Override args during installation
b00t mcp install chrome-devtools-mcp dotmcpjson \
  --arg-append "--browser-url=http://192.168.1.150:9222"
```

**Implementation:**
1. Add `--arg-append` flag to install command
2. Add `--arg-override` flag to replace specific args
3. Support environment variable substitution in datums
4. Update .mcp.json generation to include overrides

**Tests:**
```rust
#[test]
fn test_arg_append() {
    let result = install("chrome-devtools-mcp", Some(vec!["--browser-url=..."]));
    assert!(result.args.contains("--browser-url=..."));
}
```

---

### Issue #62: Dynamic Loading via Proxy

**File:** `src/mcp/proxy.rs` (new file)

**Current:**
```rust
// All servers must be in .mcp.json
{
  "mcpServers": {
    "chrome-devtools-mcp": { ... }
  }
}
```

**Desired:**
```rust
// Load servers on-demand
b00t mcp use chrome-devtools-mcp --browser-url=http://...
```

**Implementation:**
1. Create `b00t mcp use` command
2. Implement dynamic server loading from datum
3. Add session management (auto-cleanup)
4. Support configuration overrides
5. Update b00t-mcp proxy to handle dynamic loading

**Architecture:**
```
Agent → b00t mcp use → b00t-mcp proxy → MCP Server
                         ↓
                    Datum Registry
```

**Tests:**
```rust
#[test]
fn test_dynamic_load() {
    let result = use_server("chrome-devtools-mcp", None);
    assert!(result.is_ok());
    assert!(result.server.is_running());
}
```

---

### Issue #63: Documentation & Examples

**File:** `docs/mcp-datums.md` (new file)

**Content:**
1. Schema reference (required/optional fields)
2. Minimal working example
3. Advanced example (multiple transports)
4. Validation examples
5. Common patterns

**Implementation:**
1. Create comprehensive documentation
2. Add `b00t mcp datum examples` command
3. Link documentation from help text
4. Include examples in error messages

**Tests:**
```rust
#[test]
fn test_examples_command() {
    let result = datum_examples();
    assert!(result.contains("Minimal Example"));
    assert!(result.contains("Advanced Example"));
}
```

---

### Issue #64: Actionable Error Messages

**File:** `src/error.rs` (or similar)

**Current:**
```rust
// Cryptic errors
Error: Failed to parse MCP config TOML
```

**Desired:**
```rust
// Clear, actionable errors
✗ Failed to parse MCP config TOML: ~/.dotfiles/_b00t_/chrome-devtools-mcp.mcp.toml

Error at line 15: unknown field 'b00t.env'

Supported fields:
  - [b00t] (required)
  - [[b00t.mcp.stdio]] (required)
  - [[b00t.gate]] (optional)

See: b00t mcp datum schema
```

**Implementation:**
1. Add line numbers to parse errors
2. List supported fields in errors
3. Suggest similar names for "not found" errors
4. Link to documentation
5. Add usage examples to command errors

**Tests:**
```rust
#[test]
fn test_error_includes_line_number() {
    let result = parse_invalid_datum();
    assert!(result.err().contains("line 15"));
}

#[test]
fn test_error_suggests_similar() {
    let result = install("chrom-devtools-mcp");
    assert!(result.err().contains("Did you mean: chrome-devtools-mcp?"));
}
```

---

## Testing Strategy

### Unit Tests

Each feature should have comprehensive unit tests:

```bash
cargo test --lib mcp::registry
cargo test --lib mcp::datum
cargo test --lib mcp::install
cargo test --lib mcp::proxy
```

### Integration Tests

Test end-to-end workflows:

```bash
# Test search
b00t mcp registry search chrome | grep chrome-devtools-mcp

# Test validation
b00t mcp datum validate test.mcp.toml

# Test installation with overrides
b00t mcp install test-mcp dotmcpjson --arg-append "--test"

# Test dynamic loading
b00t mcp use test-mcp --test
```

### Manual Testing

Test with real MCP servers:

```bash
# Install chrome-devtools-mcp with custom browser URL
b00t mcp install chrome-devtools-mcp dotmcpjson \
  --arg-append "--browser-url=http://192.168.1.150:9222"

# Verify .mcp.json
cat .mcp.json | jq '.mcpServers["chrome-devtools-mcp"]'

# Test dynamic loading
b00t mcp use chrome-devtools-mcp --browser-url=http://192.168.1.150:9222
```

---

## Coordination

### With b00t Maintainers

Before starting work:

1. **Open an issue in b00t** referencing the kr0ki issue
2. **Discuss approach** with b00t maintainers
3. **Get approval** for the implementation plan
4. **Coordinate timing** to avoid conflicts

### With kr0ki Team

During implementation:

1. **Share progress** in kr0ki issue comments
2. **Request reviews** from kr0ki team members
3. **Test in kr0ki** after b00t PR is merged
4. **Update kr0ki issues** when complete

---

## Success Criteria

### Phase 1 (Issues #59, #60, #64)

- [ ] `b00t mcp registry search chrome` works without flags
- [ ] `b00t mcp datum validate` command exists and works
- [ ] Error messages include line numbers and suggestions
- [ ] All tests pass
- [ ] PRs merged to b00t

### Phase 2 (Issues #61, #63)

- [ ] `--arg-append` and `--arg-override` flags work
- [ ] Documentation is comprehensive
- [ ] Examples command works
- [ ] All tests pass
- [ ] PRs merged to b00t

### Phase 3 (Issue #62)

- [ ] `b00t mcp use` command works
- [ ] Dynamic loading loads servers on-demand
- [ ] Session management works (auto-cleanup)
- [ ] All tests pass
- [ ] PR merged to b00t

---

## Timeline

**Week 1:** Phase 1 (Issues #59, #60, #64)
- Day 1-2: Implement #59 (search)
- Day 3-4: Implement #60 (validation)
- Day 5: Implement #64 (errors)

**Week 2:** Phase 2 (Issues #61, #63)
- Day 1-3: Implement #61 (overrides)
- Day 4-5: Implement #63 (docs)

**Week 3-4:** Phase 3 (Issue #62)
- Week 3: Design and implement dynamic loading
- Week 4: Testing and refinement

**Total:** 3-4 weeks (can be parallelized with multiple developers)

---

## Resources

### Documentation

- [Agent Stories Summary](./AGENT-STORIES-B00T-MCP-ERGONOMICS.md)
- [GitHub Issues #59-64](https://github.com/PromptExecution/kr0ki/issues)
- [b00t Repository](https://github.com/elasticdotventures/_b00t_)

### Code References

- Current MCP implementation: `src/mcp/` in b00t
- Datum parsing: `src/mcp/datum.rs`
- Registry: `src/mcp/registry.rs`
- Installation: `src/mcp/install.rs`

### Communication

- b00t maintainers: Coordinate before starting work
- kr0ki team: Share progress and request reviews
- GitHub issues: Track progress and updates

---

## Conclusion

This guide provides a clear path for the kr0ki team to implement the 6 agent stories in the b00t repository. By following this workflow, we can:

1. **Improve b00t ergonomics** for the entire ecosystem
2. **Maintain single source of truth** in b00t
3. **Enable better agent experience** across all projects
4. **Build stronger collaboration** between kr0ki and b00t teams

Each issue has clear acceptance criteria, implementation details, and testing strategy. The phased approach allows for incremental delivery and feedback.

**Next Steps:**
1. Review this guide with the team
2. Assign owners for each phase
3. Coordinate with b00t maintainers
4. Begin implementation (Phase 1 first)

---

**Author:** Agent-assisted development session  
**Reviewers:** kr0ki team, b00t maintainers  
**Status:** Ready for implementation
