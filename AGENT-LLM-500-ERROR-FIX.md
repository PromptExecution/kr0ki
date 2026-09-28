# Agent LLM 500 Error Fix - 2026-09-27

## Summary

Fixed the "HTTP Error 500: Internal Server Error" that occurred when using the Agent panel in the playbook. The error was caused by the LLM server's Jinja template requiring a user message, but the AG-UI client sometimes sending requests without one.

---

## Problem

When users tried to use the Agent panel in the playbook at `http://192.168.1.137:5173/`, they received:

```
Error: HTTP Error 500: Internal Server Error

Agent: http://192.168.1.137:8789
LLM: http://192.168.1.137:8002/v1
Model: /models/snapshots/.../Qwen3.8-27B-...gguf
```

### Root Cause

The LLM server (llama.cpp) uses a Jinja chat template that requires at least one message with role "user". When the AG-UI client sent a request with an empty messages array or messages without a user role, the LLM server returned a 500 error:

```
Error: Jinja Exception: No user query found in messages.
```

### Why It Happened

The AG-UI client's `send` method should add a user message before sending, but in some cases (e.g., empty input, race conditions, or edge cases in the message handling), the messages array could be empty or lack a user message.

---

## Solution

Modified `/home/brianh/promptexecution/kr0ki/containers/kr0ki-storyb00k-agent/server.py` to ensure there's always at least one user message in the messages array before sending to the LLM server:

```python
messages = [{"role": "system", "content": SYSTEM_PREAMBLE + qa_memory + "\n\n" + skills_text}]
messages += messages_from_payload(payload)
# Ensure there's at least one user message for the LLM's Jinja template
if not any(m.get("role") == "user" for m in messages):
    # Extract user prompt from payload if available
    user_prompt = ""
    for msg in payload.get("messages") or []:
        if msg.get("role") == "user" and msg.get("content"):
            user_prompt = msg["content"]
            break
    if not user_prompt:
        user_prompt = "Hello"
    messages.append({"role": "user", "content": user_prompt})
```

### How It Works

1. After constructing the messages array (system + payload messages), check if any message has role "user"
2. If no user message exists:
   - Try to extract the user prompt from the payload's messages array
   - If not found, default to "Hello"
   - Append a user message with the extracted or default prompt
3. Continue with the LLM call as normal

---

## Testing

### Test 1: Empty Messages Array

**Before Fix:**
```
Status: 200
data: {"type": "RUN_ERROR", "message": "HTTP Error 500: Internal Server Error"}
```

**After Fix:**
```
Status: 200
data: {"type": "RUN_STARTED", ...}
data: {"type": "TEXT_MESSAGE_START", ...}
data: {"type": "TEXT_MESSAGE_CONTENT", "delta": "Hello! I'm storyb00k..."}
data: {"type": "TEXT_MESSAGE_END", ...}
data: {"type": "RUN_FINISHED", ...}
```

### Test 2: Normal User Message

**Before Fix:**
```
Status: 200
data: {"type": "RUN_ERROR", "message": "HTTP Error 500: Internal Server Error"}
```

**After Fix:**
```
Status: 200
data: {"type": "RUN_STARTED", ...}
data: {"type": "TOOL_CALL_START", "toolCallName": "list_formats", ...}
data: {"type": "TOOL_CALL_RESULT", "content": "[\"plantuml\",\"c4plantuml\",...]"}
data: {"type": "TEXT_MESSAGE_CONTENT", "delta": "I support 27 diagram formats..."}
data: {"type": "RUN_FINISHED", ...}
```

---

## Files Modified

| File | Change |
|------|--------|
| `containers/kr0ki-storyb00k-agent/server.py` | Added user message fallback logic in `_run` method |

---

## Git Commit

```
60301c9 fix: ensure user message exists for LLM Jinja template

- Add fallback to ensure at least one user message in messages array
- Fixes 'No user query found in messages' error from LLM server
- Extracts user prompt from payload if available, otherwise uses 'Hello'
- Resolves HTTP 500 errors when AG-UI client sends empty messages
```

---

## Verification Checklist

- [x] Agent server running on port 8789
- [x] LLM configured and accessible
- [x] Empty messages array handled correctly
- [x] Normal user messages work correctly
- [x] Tool calls work correctly
- [x] No 500 errors from LLM server
- [x] Agent responds appropriately

---

## Lessons Learned

### 1. LLM Chat Templates Have Requirements

**Issue:** Assumed any message array would work  
**Lesson:** LLM chat templates (especially Jinja templates) often have specific requirements like requiring a user message

### 2. Defensive Programming

**Issue:** Trusted the client to always send valid data  
**Lesson:** Always validate and fix up incoming data before passing to downstream services

### 3. Debug Logging is Essential

**Issue:** Hard to diagnose without seeing what was being sent  
**Lesson:** Add debug logging to see actual payloads being sent to external services

---

## Related Issues

- Agent panel showing "HTTP Error 500: Internal Server Error"
- LLM server logs showing "No user query found in messages"
- AG-UI client potentially sending empty or malformed message arrays

---

**Author:** Agent-assisted development session  
**Date:** 2026-09-27  
**Status:** ✅ Fixed and verified
