# Agent Loop Protection Fix - 2026-09-29

## Problem

The agent got stuck in an infinite loop trying to render SVG source code as if it were a diagram format (like D2 or PlantUML). The logs showed:

```
Round 50-61+: Agent repeatedly called render_diagram with format="svg" and SVG markup as source
Kroki returned: HTTP 400 Bad Request (SVG is an output format, not input)
Agent kept retrying the exact same failing call for 60+ rounds
```

## Root Causes

1. **No format validation**: Agent didn't understand that SVG/PNG are output formats, not input formats
2. **MAX_MODEL_TOOL_ROUNDS too high**: Set to 64, allowing too many retry attempts
3. **No consecutive failure detection**: Agent kept retrying the same failed call without learning
4. **Poor error messages**: Didn't guide the model toward valid formats

## Solution

### 1. Reduced Tool Round Limit
```python
MAX_MODEL_TOOL_ROUNDS = 15  # was 64
```

### 2. Added Consecutive Failure Tracking
```python
MAX_CONSECUTIVE_FAILURES = 3
consecutive_failures = 0

# Track failures in tool execution loop
if tool_ok:
    consecutive_failures = 0  # Reset on success
else:
    consecutive_failures += 1
    if consecutive_failures >= MAX_CONSECUTIVE_FAILURES:
        stream.text_message(f"[stopped after {consecutive_failures} consecutive tool failures...]")
        break
```

### 3. Added Format Validation
```python
VALID_INPUT_FORMATS = frozenset([
    "d2", "plantuml", "c4plantuml", "mermaid", "graphviz", "dot",
    "structurizr", "nomnoml", "erd", "bpmn", "bytefield", "pikchr",
    "wavedrom", "k8s", "k8s-topology", "kubediagram", ...
])

if name == "render_diagram":
    fmt = (arguments.get("format") or "").lower().strip()
    if fmt in ("svg", "png"):
        error_msg = f"ERROR: '{fmt}' is an OUTPUT format, not an input format. Valid input formats are: {', '.join(sorted(VALID_INPUT_FORMATS)[:10])}... Do NOT retry with '{fmt}' as the format."
        # Return error immediately without calling Kroki
```

### 4. Enhanced Error Messages
```python
if consecutive_failures >= 2:
    tool_content += f"\n\n[WARNING: {consecutive_failures} consecutive failures. Consider a different approach or format.]"
```

## Changes Made

### File: `containers/kr0ki-storyb00k-agent/server.py`

1. **Constants** (lines 36-50):
   - `MAX_MODEL_TOOL_ROUNDS = 15` (was 64)
   - `MAX_CONSECUTIVE_FAILURES = 3` (new)
   - `VALID_INPUT_FORMATS` frozenset (new)

2. **Loop Protection** (lines 808-815):
   - Initialize `consecutive_failures` counter
   - Track failures across tool calls
   - Break loop when threshold exceeded

3. **Format Validation** (lines 970-985):
   - Validate render_diagram format before calling Kroki
   - Return clear error message for invalid formats
   - Increment failure counter

4. **Enhanced Error Messages** (lines 1010-1015):
   - Add warning after 2 consecutive failures
   - Guide model toward different approach

5. **Loop Break** (lines 1035-1040):
   - Break out of for loop when threshold exceeded
   - Break out of while loop when threshold exceeded

## Testing

Both servers restarted successfully:
- kr0ki server: `http://192.168.1.137:8787` (v0.0.4)
- Agent server: `http://192.168.1.137:8789` (ok)

## Expected Behavior

Now when the agent encounters an invalid format:
1. First attempt: Clear error message explaining SVG/PNG are output formats
2. Second attempt: Warning about consecutive failures
3. Third attempt: Loop breaks with message to reconsider approach
4. Agent stops retrying and presents findings to user

## Prevention

This fix prevents:
- Infinite retry loops on invalid formats
- Wasted LLM tokens on repeated failures
- User frustration from stuck agent
- Resource exhaustion from endless tool calls

## Related Issues

- Agent got into endless loop trying to render SVG as input format
- MAX_MODEL_TOOL_ROUNDS=64 was too permissive
- No format validation allowed invalid calls to Kroki
- No consecutive failure detection allowed infinite retries
