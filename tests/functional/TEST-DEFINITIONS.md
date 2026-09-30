# Functional Test Definitions

This document defines functional tests with expected outcomes for kr0ki.

## Test Execution

Tests are executed using Chrome DevTools MCP server connected to Chromium at `192.168.1.150:9222`.

---

## Core Rendering Tests

### CORE-001: kr0ki Server Health Check

**Objective:** Verify kr0ki server is running and healthy

**Steps:**
1. Navigate to `http://192.168.1.137:8787/health`
2. Wait for response

**Expected Outcome:**
- Response contains `"status":"ok"`
- Response contains `"version":"0.0.4"`
- Response contains `"service":"kr0ki"`

**Validation:**
```bash
curl -s http://192.168.1.137:8787/health | jq '.status'
# Expected: "ok"
```

---

### CORE-002: Agent Server Health Check

**Objective:** Verify agent server is running and healthy

**Steps:**
1. Navigate to `http://192.168.1.137:8789/health`
2. Wait for response

**Expected Outcome:**
- Response contains `"status":"ok"`
- Response contains `"service":"kr0ki-storyb00k-agent"`
- Response contains `"max_model_tool_rounds":15` (loop protection active)

**Validation:**
```bash
curl -s http://192.168.1.137:8789/health | jq '.status'
# Expected: "ok"
```

---

### CORE-003: Direct Diagram Render (D2)

**Objective:** Verify direct API render works for D2 format

**Steps:**
1. Navigate to `http://192.168.1.137:8787/playbook/`
2. Click "Code Editor" tab
3. Wait for editor to load
4. Enter D2 source: `x -> y -> z`
5. Click "Render" button
6. Wait for rendered image

**Expected Outcome:**
- Image element appears in preview area
- Image is not broken (has valid src)
- Status shows cache hit/miss information

**Validation:**
```bash
curl -X POST http://192.168.1.137:8787/render/d2?output=svg \
  -H "Content-Type: text/plain" \
  -d 'x -> y -> z' \
  -o /tmp/test.svg
# Expected: Valid SVG file created
```

---

### CORE-004: Auto-Render on Source Change

**Objective:** Verify auto-render triggers when source changes

**Steps:**
1. Navigate to Code Editor
2. Enter initial source: `a -> b`
3. Wait 1 second
4. Verify image appears
5. Modify source to: `a -> b\nc -> d`
6. Wait 1 second
7. Verify image updates

**Expected Outcome:**
- Image appears after initial source entry
- Image updates after source modification
- No manual "Render" click required

**Validation:**
- Screenshot comparison before/after source change
- Image src attribute changes

---

### CORE-005: Format Switching

**Objective:** Verify output format can be changed in Setup

**Steps:**
1. Navigate to Setup panel
2. Change "Default Output Format" to PNG
3. Save settings
4. Navigate to Code Editor
5. Render a diagram

**Expected Outcome:**
- Format dropdown shows PNG selected
- Rendered image is PNG format
- Setting persists after page reload

**Validation:**
```bash
# Check localStorage
# Expected: kr0ki:outputFormat = "png"
```

---

## Agent Interaction Tests

### AGENT-001: Direct Agent Request

**Objective:** Verify agent can generate diagram from text request

**Steps:**
1. Navigate to Agent tab
2. Enter request: "Create a simple flowchart with 3 boxes: Start, Process, End"
3. Click Send
4. Wait for response

**Expected Outcome:**
- Agent responds with diagram
- Diagram appears in Evidence panels
- Response includes explanation
- No errors in console

**Validation:**
- Screenshot of agent response
- Diagram visible in panels
- Console has no errors

---

### AGENT-002: Code Editor to Agent Handoff

**Objective:** Verify "Send to Agent" button works correctly

**Steps:**
1. Navigate to Code Editor
2. Enter D2 source: `x -> y -> z`
3. Click "Render"
4. Wait for image
5. Click "Send to Agent" button
6. Verify handoff preview appears
7. Enter request: "Review this diagram"
8. Click Send

**Expected Outcome:**
- Purple "Send to Agent" button is visible
- Handoff preview shows rendered image
- Image can be dismissed with × button
- Agent receives diagram context
- Agent provides review/suggestions

**Validation:**
- Screenshot of handoff preview
- Agent response mentions diagram elements
- No errors in console

---

### AGENT-003: Agent Error Handling

**Objective:** Verify agent handles invalid requests gracefully

**Steps:**
1. Navigate to Agent tab
2. Enter invalid request: "Create a diagram with format=svg"
3. Click Send
4. Wait for response

**Expected Outcome:**
- Agent responds with error message
- Error message explains SVG is output format
- Agent suggests valid input formats
- Loop protection prevents infinite retry

**Validation:**
- Response contains error explanation
- No infinite loop (response completes)
- Console shows format validation error

---

### AGENT-004: Agent Loop Protection

**Objective:** Verify agent stops after consecutive failures

**Steps:**
1. Navigate to Agent tab
2. Enter request that will fail: "Render this as SVG format"
3. Click Send
4. Wait for response

**Expected Outcome:**
- Agent attempts render
- Render fails (SVG is output format)
- After 3 consecutive failures, agent stops
- Agent provides error message
- No infinite loop

**Validation:**
- Response completes (doesn't hang)
- Error message mentions consecutive failures
- Console shows loop protection triggered

---

## Handoff Tests

### HANDOFF-001: Valid D2 Handoff

**Objective:** Verify D2 diagram handoff sets context correctly

**Steps:**
1. POST to `/projects/set-diagram-context` with D2 source, format=d2, detectedType=d2
2. Check response status is "ok"
3. Verify format is echoed in response

**Expected Outcome:**
- HTTP 200 with `{"status":"ok","format":"d2"}`
- Project stores diagramContext

---

### HANDOFF-002: Valid PlantUML Handoff

**Objective:** Verify PlantUML diagram handoff works

**Steps:**
1. POST with PlantUML source (`@startuml...`), format=plantuml
2. Check response

**Expected Outcome:**
- HTTP 200, context stored

---

### HANDOFF-003: Missing Source Validation

**Objective:** Verify server rejects handoff without source

**Steps:**
1. POST with format=d2 but no source field
2. Check response

**Expected Outcome:**
- HTTP 400 with `{"error":"missing_source"}`

---

### HANDOFF-004: Invalid Format Validation

**Objective:** Verify server rejects handoff with empty/invalid format

**Steps:**
1. POST with source but format=""
2. Check response

**Expected Outcome:**
- HTTP 400 with `{"error":"missing_format"}`

---

### HANDOFF-005: Metadata Preservation

**Objective:** Verify all handoff metadata is stored correctly

**Steps:**
1. POST with full metadata (source, format, detectedType, output, title)
2. GET the project and verify diagramContext fields

**Expected Outcome:**
- All fields preserved in project.diagramContext
- handoffHistory array contains the entry

---

### HANDOFF-006: Subsequent Prompts After Handoff

**Objective:** Verify agent can continue conversation after handoff

**Steps:**
1. Set diagram context via handoff
2. Send a follow-up prompt via /run
3. Verify agent responds

**Expected Outcome:**
- Agent responds with RUN_FINISHED
- Agent has access to diagram context

---

## UI/UX Tests

### UI-001: Navigation Between Views

**Objective:** Verify all view tabs work correctly

**Steps:**
1. Navigate to `http://192.168.1.137:8787/playbook/`
2. Click each tab: Gallery, Code Editor, Agent, Setup
3. Verify each view loads

**Expected Outcome:**
- All tabs are clickable
- Each view loads without errors
- Active tab is highlighted
- Content changes appropriately

**Validation:**
- Screenshots of each view
- No console errors
- Active tab has correct styling

---

### UI-002: Version Display

**Objective:** Verify version is displayed in sidebar

**Steps:**
1. Navigate to playbook
2. Look at sidebar

**Expected Outcome:**
- Version tag shows "v0.0.4"
- Version is under "kr0ki playb00k" text
- Version is styled correctly

**Validation:**
```bash
# Check built files
grep -o "0.0.4" playbook/dist/assets/*.js
# Expected: Found
```

---

### UI-003: Catalog Loading

**Objective:** Verify example catalog loads correctly

**Steps:**
1. Navigate to playbook
2. Wait for catalog to load

**Expected Outcome:**
- Catalog shows 37 fixtures
- Each fixture has format and title
- Catalog is clickable
- No 404 errors

**Validation:**
```bash
curl -s http://192.168.1.137:8787/api/examples | jq 'length'
# Expected: 37
```

---

### UI-004: Setup Panel Configuration

**Objective:** Verify setup panel can save settings

**Steps:**
1. Navigate to Setup panel
2. Enter renderer URL: `http://192.168.1.137:8787`
3. Click "Save Settings"
4. Verify save confirmation

**Expected Outcome:**
- Settings are saved to localStorage
- "✓ Saved" button appears briefly
- Settings persist after page reload

**Validation:**
```javascript
// Check localStorage
localStorage.getItem('kr0ki:rendererUrl')
// Expected: "http://192.168.1.137:8787"
```

---

### UI-005: Auto Render Checkbox

**Objective:** Verify Auto Render checkbox works

**Steps:**
1. Navigate to Code Editor
2. Verify Auto Render checkbox is present
3. Uncheck Auto Render
4. Modify source
5. Verify no auto-render
6. Check Auto Render
7. Modify source
8. Verify auto-render triggers

**Expected Outcome:**
- Checkbox is visible and functional
- When unchecked, no auto-render
- When checked, auto-render triggers after 500ms

**Validation:**
- Checkbox state changes
- Render behavior matches checkbox state

---

## Integration Tests

### INT-001: End-to-End Flow

**Objective:** Verify complete user flow from editor to agent

**Steps:**
1. Navigate to Code Editor
2. Create diagram: `start -> process -> end`
3. Render diagram
4. Click "Send to Agent"
5. Enter request: "Improve this diagram"
6. Wait for agent response
7. Verify agent provides suggestions

**Expected Outcome:**
- Diagram renders successfully
- Handoff to agent works
- Agent receives diagram context
- Agent provides meaningful suggestions
- No errors throughout flow

**Validation:**
- Screenshots at each step
- Agent response is relevant
- No console errors

---

### INT-002: Cross-Component State

**Objective:** Verify state persists across components

**Steps:**
1. Set renderer URL in Setup
2. Navigate to Code Editor
3. Verify renderer URL is used
4. Set output format in Setup
5. Navigate to Code Editor
6. Verify format is used

**Expected Outcome:**
- Settings from Setup are used in Code Editor
- State persists across navigation
- No manual configuration needed in Code Editor

**Validation:**
- localStorage contains settings
- Code Editor uses those settings

---

## Performance Tests

### PERF-001: Render Response Time

**Objective:** Verify render completes in reasonable time

**Steps:**
1. Navigate to Code Editor
2. Enter simple D2 source
3. Click Render
4. Measure time to image appearance

**Expected Outcome:**
- Render completes in < 5 seconds
- No timeout errors
- Image appears smoothly

**Validation:**
- Time measurement < 5000ms
- No timeout in console

---

### PERF-002: Agent Response Time

**Objective:** Verify agent responds in reasonable time

**Steps:**
1. Navigate to Agent tab
2. Enter simple request
3. Click Send
4. Measure time to first response

**Expected Outcome:**
- First response appears in < 10 seconds
- Complete response in < 30 seconds
- No timeout errors

**Validation:**
- Time measurement < 30000ms
- No timeout in console

---

## Test Execution Checklist

Before declaring work complete, run through this checklist:

- [ ] All CORE tests pass
- [ ] All AGENT tests pass
- [ ] All UI tests pass
- [ ] All INT tests pass
- [ ] All PERF tests pass
- [ ] No console errors in any test
- [ ] Screenshots captured for visual validation
- [ ] All expected outcomes verified

---

## Test Results Template

```
Test Suite: [Name]
Date: [YYYY-MM-DD]
Tester: [Agent/Manual]

Results:
- Passed: X
- Failed: Y
- Skipped: Z

Failed Tests:
1. [Test Name]: [Error Message]

Screenshots:
- [Screenshot path 1]
- [Screenshot path 2]

Notes:
[Any additional observations]
```
