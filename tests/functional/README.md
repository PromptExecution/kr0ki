# Functional Test Suite

This directory contains functional tests that validate kr0ki through actual user interactions using Chrome DevTools MCP.

## Test Philosophy

Tests are **user-flow driven** - they simulate real user interactions and validate outcomes, not just API responses.

## Test Categories

### 1. Core Rendering Tests
- Direct render via API
- Code Editor render
- Format switching
- Auto-render behavior

### 2. Agent Interaction Tests
- Direct agent requests
- Code Editor → Agent handoff
- Diagram review flow
- Error handling

### 3. UI/UX Tests
- Navigation between views
- Setup panel configuration
- Version display
- Catalog loading

### 4. Integration Tests
- End-to-end flows
- Cross-component interactions
- State persistence

## Running Tests

```bash
# Run all tests
just test-functional

# Run specific test suite
just test-functional core
just test-functional agent
just test-functional ui

# Run with Chrome DevTools
just test-functional --with-chrome
```

## Test Structure

Each test file follows this pattern:
```javascript
{
  name: "Test Name",
  description: "What this test validates",
  steps: [
    { action: "navigate", url: "..." },
    { action: "click", selector: "..." },
    { action: "type", selector: "...", text: "..." },
    { action: "wait", condition: "..." },
    { action: "assert", selector: "...", expected: "..." }
  ],
  expectedOutcome: "What should be true after all steps"
}
```

## Test Validation

Tests validate:
- **Visual state**: Elements present, text correct
- **Functional state**: API calls made, data persisted
- **User experience**: No errors, smooth transitions
- **Performance**: Response times acceptable

## Adding New Tests

1. Create test file in appropriate category
2. Define user flow steps
3. Specify expected outcomes
4. Run test and verify it passes
5. Document any known issues
