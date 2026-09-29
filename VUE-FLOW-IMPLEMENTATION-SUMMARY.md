# Vue-Flow Revision Graph: Implementation Summary

## Current Implementation (Before Fix)

### Architecture Overview

The revision graph system tracks the evolution of diagrams through a **Directed Acyclic Graph (DAG)** structure, visualized using vue-flow.

```
┌─────────────────────────────────────────────────────────────┐
│                    Revision Graph System                     │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  revisionGraph.js  ──►  RevisionFlow.vue  ──►  StoryB00k   │
│  (Data Structure)      (Visualization)       (Integration)  │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Data Structure: `revisionGraph.js`

**Node Shape:**
```javascript
{
  id: 'rev-abc123',
  parentId: 'rev-xyz789' | null,  // null = root node
  kind: 'root' | 'prompt' | 'edit' | 'fork',
  label: 'Short description',
  prompt: 'User prompt that created this',
  source: 'x -> y -> z',  // diagram source code
  format: 'd2',
  route: '/render/d2' | null,
  notes: '',
  createdAt: '2026-09-29T...'
}
```

**Graph Shape:**
```javascript
{
  nodes: [node1, node2, ...],
  activeId: 'rev-abc123'  // current HEAD (active node)
}
```

**Operations:**
- `createRevisionGraph()` - Creates root node with empty source
- `addPromptNode()` - Adds agent-generated diagram as child of active
- `addEditNode()` - Adds manual edit as child of active
- `checkoutNode()` - Time travel: changes activeId to existing node
- `forkFrom()` - Creates branch from any node
- `serialize()` / `deserialize()` - JSON persistence

### Visualization: `RevisionFlow.vue`

Uses `@vue-flow/core` to render the DAG as a tree:

**Layout Algorithm:**
```javascript
// Simple BFS from roots, one column per depth
function place(node, depth) {
  const slot = depthCount.get(depth) || 0
  depthCount.set(depth, slot + 1)
  positioned.set(node.id, { x: slot * 240, y: depth * 130 })
  for (const child of children.get(node.id) || []) {
    place(child, depth + 1)
  }
}
```

**Visual Elements:**
- Nodes colored by kind (root=blue, prompt=green, edit=purple, fork=orange)
- Active node highlighted with blue outline
- Edges connect parent → child
- Click node to checkout (time travel)
- Fork button (⑂) to branch from any node

### Integration: `StoryB00k.vue`

**Initialization:**
```javascript
const revisionGraph = reactive(
  createRevisionGraph({ source: '', format: 'd2', label: 'Session start' })
)
```

**Auto-tracking:**
```javascript
// Watch for new renders from agent
watch(panels, (list) => {
  const latest = [...list].reverse().find((p) => p.kind === 'render')
  if (latest) {
    addPromptNode(revisionGraph, {
      prompt: latest.toolName,
      source: latest.source.text,
      format: latest.source.format
    })
  }
})
```

**User Actions:**
- Click node → `checkoutRevision()` → time travel
- Click fork → `forkRevision()` → create branch
- Save → `persistChart()` → send to agent server

---

## Problem: Code Editor Handoff

### Issue
When code is sent from Code Editor to Agent mode, it was being added as a **prompt node** instead of becoming the **root node**.

**Before Fix:**
```
Session Start
  ↓
Root (empty source)  ← Problem: root is empty
  ↓
Prompt node (code from editor)  ← Should be root
  ↓
Prompt node (agent response)
```

**Expected:**
```
Session Start
  ↓
Root (code from editor)  ← Fixed: root has the code
  ↓
Prompt node (agent response)
```

---

## Fix Implemented

### Changes Made

#### 1. `revisionGraph.js` - Enhanced Root Node Creation

**Before:**
```javascript
export function createRevisionGraph({ source = '', format = 'd2', route = null, label = 'Initial diagram' } = {}) {
  const root = {
    id: makeId('rev'),
    parentId: null,
    kind: NODE_KINDS.ROOT,
    label,
    prompt: null,
    source,
    format,
    route,
    notes: '',
    createdAt: new Date().toISOString(),
  }
  return { nodes: [root], activeId: root.id }
}
```

**After:**
```javascript
export function createRevisionGraph({ 
  source = '', 
  format = 'd2', 
  route = null, 
  label = 'Initial diagram',
  detectedType = null,  // NEW: diagram type detection
  output = null,        // NEW: output format (svg/png)
  title = null          // NEW: diagram title
} = {}) {
  const root = {
    id: makeId('rev'),
    parentId: null,
    kind: NODE_KINDS.ROOT,
    label,
    prompt: null,
    source,
    format,
    route,
    detectedType,  // NEW
    output,        // NEW
    title,         // NEW
    notes: '',
    createdAt: new Date().toISOString(),
  }
  return { nodes: [root], activeId: root.id }
}
```

#### 2. `StoryB00k.vue` - Initialize with Editor Handoff

**Before:**
```javascript
const revisionGraph = reactive(
  createRevisionGraph({ source: '', format: 'd2', label: 'Session start' })
)
```

**After:**
```javascript
// Initialize with editor handoff source if available
const initialSource = props.editorHandoff?.source || ''
const initialFormat = props.editorHandoff?.format || 'd2'
const initialLabel = props.editorHandoff ? 'Initial diagram from Code Editor' : 'Session start'

const revisionGraph = reactive(createRevisionGraph({ 
  source: initialSource, 
  format: initialFormat, 
  label: initialLabel,
  detectedType: props.editorHandoff?.detectedType,
  output: props.editorHandoff?.output,
  title: props.editorHandoff?.title
}))

// If editor handoff provided, mark the root node with metadata
if (props.editorHandoff) {
  const rootNode = revisionGraph.nodes[0]
  if (rootNode) {
    rootNode.detectedType = props.editorHandoff.detectedType
    rootNode.output = props.editorHandoff.output
    rootNode.title = props.editorHandoff.title
    console.info('[storyb00k] initialized revision graph with Code Editor handoff:', {
      source: initialSource.substring(0, 50) + '...',
      format: initialFormat,
      detectedType: props.editorHandoff.detectedType
    })
  }
}
```

#### 3. `RevisionFlow.vue` - Visual Enhancements

**Enhanced Node Display:**
```vue
<template #node-revision="{ data }">
  <div
    class="revflow__node"
    :class="{ 
      'revflow__node--active': data.isActive,
      'revflow__node--root': data.node.kind === 'root' && data.node.source
    }"
    :style="{ borderColor: data.kindColor }"
  >
    <span class="revflow__node-kind" :style="{ background: data.kindColor }">
      {{ data.node.kind }}
    </span>
    <p class="revflow__node-label" :title="data.node.prompt || data.node.label">
      {{ data.node.kind === 'root' && data.node.source ? '📝 ' : '' }}
      {{ data.node.label }}
    </p>
    <p v-if="data.node.detectedType" class="revflow__node-meta" 
       :title="`Detected: ${data.node.detectedType}`">
      {{ data.node.detectedType }}
    </p>
    <div class="revflow__node-actions">
      <button v-if="!data.isActive" class="revflow__btn" 
              title="Time travel to this state" 
              @click.stop="$emit('checkout', data.node.id)">⏱</button>
      <button class="revflow__btn" 
              title="Fork a new branch from here" 
              @click.stop="onForkClick(data.node.id)">⑂</button>
    </div>
  </div>
</template>
```

**Enhanced Styling:**
```css
.revflow__node--root { 
  border-width: 3px; 
  border-style: double;  /* Visual distinction for root with source */
}

.revflow__node-meta { 
  margin: .15rem 0; 
  font-size: .65rem; 
  color: #64748b; 
  overflow: hidden; 
  text-overflow: ellipsis; 
  white-space: nowrap; 
}
```

---

## How It Works Now

### Flow 1: Direct Agent Request (No Editor Handoff)

```
1. User opens Agent tab
2. revisionGraph initialized with empty root
3. User types: "Create a flowchart"
4. Agent generates diagram
5. addPromptNode() creates child of root
6. Revision graph: root (empty) → prompt (diagram)
```

### Flow 2: Code Editor → Agent Handoff (Fixed)

```
1. User edits diagram in Code Editor
2. User clicks "Send to Agent"
3. editorHandoff prop contains: { source, format, detectedType, ... }
4. revisionGraph initialized with source as ROOT
5. Root node has: source, format, detectedType, title
6. User types: "Review this diagram"
7. Agent responds
8. addPromptNode() creates child of root
9. Revision graph: root (with code) → prompt (agent response)
```

### Visual Representation

**Before Fix:**
```
┌─────────────────────┐
│ root                │
│ (empty)             │
└──────────┬──────────┘
           │
┌──────────▼──────────┐
│ prompt              │
│ x -> y -> z         │  ← Code from editor (wrong place)
└──────────┬──────────┘
           │
┌──────────▼──────────┐
│ prompt              │
│ Agent response      │
└─────────────────────┘
```

**After Fix:**
```
┌─────────────────────┐
│ 📝 root             │
│ x -> y -> z         │  ← Code from editor (correct!)
│ detected: d2        │
│ (double border)     │
└──────────┬──────────┘
           │
┌──────────▼──────────┐
│ prompt              │
│ Agent response      │
└─────────────────────┘
```

---

## Testing

### Test Case 1: Editor Handoff Creates Root

**Steps:**
1. Open Code Editor
2. Enter D2 source: `x -> y -> z`
3. Click "Send to Agent"
4. Verify root node has source code
5. Verify root node shows detected type

**Expected:**
- Root node label: "Initial diagram from Code Editor"
- Root node source: "x -> y -> z"
- Root node detectedType: "d2"
- Root node has double border (visual distinction)

### Test Case 2: Subsequent Agent Responses

**Steps:**
1. After handoff, type: "Review this diagram"
2. Agent responds
3. Verify new node is child of root

**Expected:**
- New prompt node created
- Parent is root (with editor code)
- Revision graph shows proper lineage

### Test Case 3: Time Travel

**Steps:**
1. Create multiple revisions
2. Click on root node (with editor code)
3. Verify diagram reverts to editor code

**Expected:**
- Active node changes to root
- Diagram displays editor code
- Visual highlight on root node

---

## Future Enhancements (Git Integration)

### Planned: Git-Based Revision System

The current DAG is a good foundation, but we plan to integrate with actual git:

**Benefits:**
- Real commits with SHAs
- Persistent storage
- Branch/merge support
- Remote sync
- Collaboration

**Architecture:**
```
┌─────────────────────────────────────────┐
│         Git Revision System              │
├─────────────────────────────────────────┤
│                                          │
│  isomorphic-git  ◄──►  Local .git repo  │
│        │                                  │
│        ▼                                  │
│  Revision Graph (git-based)              │
│        │                                  │
│        ▼                                  │
│  Vue-Flow Visualization                  │
│                                          │
└─────────────────────────────────────────┘
```

**Key Changes:**
1. Each commit = diagram state
2. Branches = named lines of development
3. HEAD = current active state
4. Checkout = time travel
5. Fork = create branch

See `VUE-FLOW-REDESIGN.md` for detailed design.

---

## Summary

### What Was Fixed

✅ Code Editor handoff now creates root node with source code  
✅ Root node displays detected type and metadata  
✅ Visual distinction for root nodes with source (double border)  
✅ Proper lineage: editor code → agent responses  

### What Works Now

✅ Direct agent requests (empty root → prompts)  
✅ Editor handoff (code as root → prompts)  
✅ Time travel to any revision  
✅ Branching from any node  
✅ Visual git-like history  

### What's Next

🔄 Git integration (isomorphic-git)  
🔄 Real commits and branches  
🔄 Remote sync and collaboration  
🔄 Agent-assisted branching  

---

**Status:** ✅ Fix implemented and deployed  
**Version:** 0.0.4  
**Date:** 2026-09-29
