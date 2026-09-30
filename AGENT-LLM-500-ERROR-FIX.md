# Agent Issues Fixed - 2026-09-27

## Summary

Fixed multiple issues with the Agent panel:
1. **LLM 500 Error**: "HTTP Error 500: Internal Server Error" when sending messages
2. **Repetitive Questions**: Agent kept asking the same questions over and over
3. **Interrupt 404 Errors**: "answer failed: HTTP 404" when answering questions
4. **Task Never Finishes**: Agent didn't know when to stop asking and start generating

---

## Root Causes

### 1. LLM 500 Error
The LLM server (llama.cpp) uses a Jinja chat template that **requires at least one message with role "user"**. When the AG-UI client sent requests with empty messages or no user message, the LLM server returned:
```
Error: Jinja Exception: No user query found in messages.
```

### 2. Repetitive Questions
The agent kept asking the same questions because:
- QA history injection logic was flawed (checking if answer text was in message content)
- No explicit tracking of already-asked questions
- System prompt didn't list what questions had been asked

### 3. Interrupt 404 Errors
The `/respond-to-interrupt` endpoint returned 404 when:
- Question wasn't in `_pending_questions` dict (already answered or expired)
- No draft session found for the thread
- Client tried to answer a question that was already processed

### 4. Task Never Finishes
The agent didn't have clear signals about:
- What questions had already been asked
- When to stop asking and start generating
- How to track conversation progress

---

## Solutions

### Fix 1: Ensure User Message Exists

Modified `server.py` to ensure there's always at least one user message:

```python
messages = [{"role": "system", "content": SYSTEM_PREAMBLE + qa_memory + "\n\n" + skills_text}]
messages += messages_from_payload(payload)
# Ensure there's at least one user message for the LLM's Jinja template
if not any(m.get("role") == "user" for m in messages):
    user_prompt = ""
    for msg in payload.get("messages") or []:
        if msg.get("role") == "user" and msg.get("content"):
            user_prompt = msg["content"]
            break
    if not user_prompt:
        user_prompt = "Hello"
    messages.append({"role": "user", "content": user_prompt})
```

### Fix 2: Proper QA History Injection

Changed from conditional injection to always injecting all QA pairs:

```python
# Track which questions have been answered to prevent repetition
asked_questions = set()
for qa in project.get("qa", []):
    question_text = qa.get("question", "")
    answer_text = qa.get("answer", "")
    asked_questions.add(question_text.lower().strip())
    # Always inject the QA pair so the model sees the full history
    messages.append({
        "role": "user",
        "content": f"(answer to your question \"{question_text}\"): {answer_text}",
    })
# Add explicit instruction about already-asked questions
if asked_questions:
    questions_list = "\n".join(f"- {q}" for q in asked_questions)
    messages.append({
        "role": "user",
        "content": f"IMPORTANT: You have already asked these questions and received answers. Do NOT ask them again:\n{questions_list}\n\nIf you need more information, ask NEW questions that haven't been asked yet.",
    })
```

### Fix 3: List Already-Asked Questions in System Prompt

Added to system prompt:
```python
qa_history = project.get("qa", [])
if qa_history:
    asked_list = "\n".join(f"  {i+1}. {qa['question']}" for i, qa in enumerate(qa_history))
    qa_memory += f"\n\nALREADY ASKED QUESTIONS (do NOT repeat these):\n{asked_list}"
```

### Fix 4: Prevent Asking Same Question Twice

Added check before asking questions:
```python
if name == "ask_user":
    # Check if this question has already been asked
    question_text = arguments.get("question", "")
    project = project_store.get_project(thread_id)
    already_asked = False
    if project:
        for qa in project.get("qa", []):
            if qa.get("question", "").lower().strip() == question_text.lower().strip():
                already_asked = True
                break
    if already_asked:
        # Question already asked, skip it and continue
        messages.append({"role": "tool", "tool_call_id": tool_call_id, "content": f"Question already asked: {question_text}"})
        run_log.event("plan.question_skipped", {"question": question_text, "reason": "already_asked"})
        continue
```

### Fix 5: Handle Interrupt 404 Gracefully

Check QA history for already-answered questions:
```python
if draft is None and not is_answer:
    # Check if this question was already answered (prevents 404 on retry)
    project = project_store.get_project(thread_id)
    interrupt_id = payload.get("interruptId")
    already_answered = False
    if project:
        for qa in project.get("qa", []):
            if qa.get("interruptId") == interrupt_id:
                already_answered = True
                break
    if already_answered:
        # Question was already answered, return success
        return self._json(200, {"status": "ok", "already_answered": True})
```

### Fix 6: Store Interrupt IDs in QA Entries

Track interrupt IDs for deduplication:
```python
qa_entry = {"question": question.get("question", ""), "answer": answer, "ts": time.time()}
if question.get("id"):
    qa_entry["interruptId"] = question["id"]
project_store.add_qa(thread_id, qa_entry["question"], qa_entry["answer"])
# Update the stored QA entry with interrupt ID
project = project_store.get_project(thread_id)
if project and project.get("qa"):
    for qa in project["qa"]:
        if qa.get("question") == qa_entry["question"] and qa.get("answer") == qa_entry["answer"]:
            qa["interruptId"] = question.get("id")
            break
    project_store.save_project(thread_id, project)
```

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

### Test 3: Question Answering Flow

**Before Fix:**
- Agent asks question
- User answers → HTTP 404
- Agent asks same question again
- Loop repeats indefinitely

**After Fix:**
- Agent asks question
- User answers → Success (200)
- Agent sees answer in QA history
- Agent asks NEW questions or generates diagram
- Task completes

---

## Files Modified

| File | Changes |
|------|---------|
| `containers/kr0ki-storyb00k-agent/server.py` | All 6 fixes applied |

---

## Git Commit

```
f028d45 fix: prevent agent from repeating questions and handle interrupt 404s
60301c9 fix: ensure user message exists for LLM Jinja template
```

---

## Verification Checklist

- [x] Agent server running on port 8789
- [x] LLM configured and accessible
- [x] Empty messages array handled correctly
- [x] Normal user messages work correctly
- [x] Tool calls work correctly
- [x] No 500 errors from LLM server
- [x] Agent doesn't repeat questions
- [x] Interrupt answers work without 404 errors
- [x] Agent responds appropriately
- [x] Task completes successfully

---

## Lessons Learned

### 1. LLM Chat Templates Have Requirements
**Issue:** Assumed any message array would work  
**Lesson:** LLM chat templates (especially Jinja templates) often have specific requirements like requiring a user message

### 2. Defensive Programming
**Issue:** Trusted the client to always send valid data  
**Lesson:** Always validate and fix up incoming data before passing to downstream services

### 3. Track State Explicitly
**Issue:** Relied on implicit assumptions about conversation state  
**Lesson:** Explicitly track what questions have been asked and answered

### 4. Handle Edge Cases Gracefully
**Issue:** 404 errors when answering already-answered questions  
**Lesson:** Check historical state before returning errors

### 5. Debug Logging is Essential
**Issue:** Hard to diagnose without seeing what was being sent  
**Lesson:** Add debug logging to see actual payloads being sent to external services

---

## Related Issues

- Agent panel showing "HTTP Error 500: Internal Server Error"
- Agent repeating the same questions indefinitely
- "answer failed: HTTP 404" when answering questions
- Agent task never completing
- LLM server logs showing "No user query found in messages"

---

**Author:** Agent-assisted development session  
**Date:** 2026-09-27  
**Status:** ✅ All issues fixed and verified
