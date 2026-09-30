# Vue-Flow Revision Graph: Current Implementation & Git-Based Redesign

## Current Implementation Explained

### Architecture Overview

The current revision system uses a **simple DAG (Directed Acyclic Graph)** to track diagram evolution:

```
revisionGraph.js  →  RevisionFlow.vue  →  StoryB00k.vue
(data structure)     (visualization)      (integration)
```

### Data Structure: `revisionGraph.js`

**Node Shape:**
```javascript
{
  id: 'rev-abc123',
  parentId: 'rev-xyz789' | null,  // null = root
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
  activeId: 'rev-abc123'  // current HEAD
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

## Problems with Current Implementation

### 1. **No Real Version Control**
- Just an in-memory DAG
- No actual commits, branches, or refs
- No persistence to disk
- No collaboration support

### 2. **Code Editor Handoff is Broken**
When code is sent from Code Editor:
```javascript
// Current flow:
1. Session starts → createRevisionGraph({ source: '', ... })
2. User sends code → becomes a PROMPT node (not root)
3. Root remains empty
```

**Expected:**
```javascript
// Desired flow:
1. Session starts → createRevisionGraph({ source: '', ... })
2. User sends code from editor → becomes ROOT commit
3. All subsequent changes branch from this root
```

### 3. **Visualization is Not Git-Like**
Current tree layout:
```
    root
     |
   prompt1
     |
   prompt2
```

Git-style graph (what users expect):
```
*  abc1234 (HEAD -> main) Add final step
|
*  def5678 Improve layout
|
*  ghi9012 Initial diagram
```

### 4. **No Branch Management**
- Forks are just nodes with different parentId
- No named branches (main, feature-x, etc.)
- No branch switching
- No merge support

### 5. **No Remote Sync**
- Everything in memory
- `persistChart()` sends to agent but no git push/pull
- No collaboration

---

## Git-Based Redesign

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Git Revision System                   │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────┐      ┌──────────────┐                │
│  │  Git Client  │◄────►│ Local .git   │                │
│  │  (isomorphic)│      │   repo       │                │
│  └──────────────┘      └──────────────┘                │
│         │                        │                       │
│         ▼                        ▼                       │
│  ┌──────────────┐      ┌──────────────┐                │
│  │ Revision     │      │ Branch       │                │
│  │ Graph        │      │ Manager      │                │
│  └──────────────┘      └──────────────┘                │
│         │                        │                       │
│         └────────┬───────────────┘                       │
│                  ▼                                        │
│         ┌──────────────┐                                │
│         │ Vue-Flow     │                                │
│         │ Visualizer   │                                │
│         └──────────────┘                                │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

### Core Concepts

#### 1. **Git Repository Integration**

Use `isomorphic-git` for browser-based git operations:

```javascript
import git from 'isomorphic-git'
import http from 'isomorphic-git/http/web'

// Initialize local repo
await git.init({ fs, dir: '/kr0ki-repo' })

// Commit diagram state
await git.commit({
  fs,
  dir: '/kr0ki-repo',
  message: 'Add initial diagram',
  author: { name: 'Agent', email: 'agent@kr0ki.local' }
})
```

#### 2. **Commit = Diagram State**

Each git commit stores:
- Diagram source code (in file)
- Metadata (format, route, prompt)
- Parent commit(s)

```javascript
// File structure in repo:
/kr0ki-repo/
  diagram.d2          # Current diagram source
  .kr0ki-meta.json    # { format, route, prompt, notes }
  .git/               # Git internals
```

#### 3. **Branches = Named Lines of Development**

```javascript
// Create branch
await git.branch({ fs, dir: '/kr0ki-repo', ref: 'feature-add-step' })

// Switch branch
await git.checkout({ fs, dir: '/kr0ki-revo', ref: 'feature-add-step' })

// List branches
const branches = await git.listBranches({ fs, dir: '/kr0ki-repo' })
```

#### 4. **HEAD = Current Active State**

```javascript
// Get current commit
const head = await git.resolveRef({ fs, dir: '/kr0ki-repo', ref: 'HEAD' })

// Get commit details
const commit = await git.readCommit({ fs, dir: '/kr0ki-repo', oid: head })
```

### Data Model Redesign

#### Old: Simple DAG
```javascript
{
  nodes: [{ id, parentId, kind, source, ... }],
  activeId: '...'
}
```

#### New: Git-Based
```javascript
{
  repoPath: '/kr0ki-repo',
  currentBranch: 'main',
  head: 'abc123...',  // commit SHA
  
  // Derived from git log
  commits: [
    {
      sha: 'abc123...',
      message: 'Add final step',
      author: { name: 'Agent', email: '...' },
      timestamp: 1234567890,
      parents: ['def456...'],
      diagram: {
        source: 'x -> y -> z',
        format: 'd2',
        route: '/render/d2',
        prompt: 'User asked for...'
      }
    }
  ],
  
  branches: [
    { name: 'main', sha: 'abc123...' },
    { name: 'feature-x', sha: 'ghi789...' }
  ]
}
```

### Visualization Redesign

#### Git-Style Graph (like `jj log`)

```
┌────────────────────────────────────────────────────────┐
│  ● abc1234  (HEAD -> main)  Add final step            │
│  │  Agent • 2 minutes ago                             │
│  │                                                     │
│  ● def5678  Improve layout                            │
│  │  Agent • 5 minutes ago                             │
│  │                                                     │
│  │ ● ghi9012  (feature-x)  Try different approach    │
│  │ │  Agent • 10 minutes ago                          │
│  │ │                                                   │
│  ●─┘ jkl3456  Initial diagram                         │
│     User • 15 minutes ago                             │
└────────────────────────────────────────────────────────┘
```

#### Vue-Flow Implementation

```javascript
// Layout: vertical timeline with branches
const layout = computed(() => {
  const commits = gitLog.value
  const positions = new Map()
  
  let y = 0
  const branchColumns = new Map()  // branch -> column
  
  for (const commit of commits) {
    const branch = getBranchForCommit(commit)
    const column = branchColumns.get(branch) || 0
    
    positions.set(commit.sha, {
      x: column * 200,
      y: y * 100
    })
    
    y++
  }
  
  return positions
})

// Nodes = commits
const nodes = computed(() => 
  gitLog.value.map(commit => ({
    id: commit.sha,
    type: 'commit',
    position: layout.value.get(commit.sha),
    data: { commit }
  }))
)

// Edges = parent relationships
const edges = computed(() =>
  gitLog.value.flatMap(commit =>
    commit.parents.map(parent => ({
      id: `${parent}-${commit.sha}`,
      source: parent,
      target: commit.sha,
      animated: commit.sha === head.value
    }))
  )
)
```

### Code Editor Handoff Fix

#### Problem
Code sent from editor becomes a prompt node, not the root.

#### Solution
When code is sent from editor, create initial commit:

```javascript
async function handleEditorHandoff({ source, format, detectedType }) {
  // Check if repo exists
  const repoExists = await fs.exists('/kr0ki-repo/.git')
  
  if (!repoExists) {
    // Initialize repo
    await git.init({ fs, dir: '/kr0ki-repo' })
    
    // Write initial diagram
    await fs.writeFile('/kr0ki-repo/diagram.d2', source)
    await fs.writeFile('/kr0ki-repo/.kr0ki-meta.json', JSON.stringify({
      format,
      detectedType,
      prompt: 'Initial diagram from Code Editor'
    }))
    
    // Stage and commit
    await git.add({ fs, dir: '/kr0ki-revo', filepath: '.' })
    await git.commit({
      fs,
      dir: '/kr0ki-repo',
      message: 'Initial diagram from Code Editor',
      author: { name: 'User', email: 'user@kr0ki.local' }
    })
    
    // Update revision graph
    await refreshGitLog()
  } else {
    // Repo exists, create new commit on current branch
    await commitDiagram(source, format, 'Diagram from Code Editor')
  }
}
```

### Branch Management UI

```vue
<template>
  <div class="branch-manager">
    <select v-model="currentBranch" @change="switchBranch">
      <option v-for="branch in branches" :key="branch.name" :value="branch.name">
        {{ branch.name }} {{ branch.name === 'main' ? '(main)' : '' }}
      </option>
    </select>
    
    <button @click="createBranch">+ New Branch</button>
    <button @click="mergeBranch" v-if="currentBranch !== 'main'">Merge to main</button>
  </div>
</template>

<script setup>
async function createBranch() {
  const name = prompt('Branch name:')
  await git.branch({ fs, dir: '/kr0ki-repo', ref: name })
  await git.checkout({ fs, dir: '/kr0ki-repo', ref: name })
  await refreshGitLog()
}

async function mergeBranch() {
  // Merge current branch into main
  await git.checkout({ fs, dir: '/kr0ki-repo', ref: 'main' })
  await git.merge({
    fs,
    dir: '/kr0ki-repo',
    ours: 'main',
    theirs: currentBranch.value
  })
  await refreshGitLog()
}
</script>
```

### Remote Sync (Future)

```javascript
// Add remote
await git.addRemote({
  fs,
  dir: '/kr0ki-repo',
  remote: 'origin',
  url: 'https://github.com/user/kr0ki-diagrams.git'
})

// Push
await git.push({
  fs,
  http,
  dir: '/kr0ki-repo',
  remote: 'origin',
  ref: 'main'
})

// Pull
await git.pull({
  fs,
  http,
  dir: '/kr0ki-repo',
  remote: 'origin',
  ref: 'main',
  author: { name: 'User', email: 'user@kr0ki.local' }
})
```

### Agent-Assisted Branching

```javascript
// Agent suggests creating a branch
async function agentSuggestBranch(suggestion) {
  const { reason, baseCommit, branchName } = suggestion
  
  // Create branch from base commit
  await git.checkout({ fs, dir: '/kr0ki-repo', ref: baseCommit })
  await git.branch({ fs, dir: '/kr0ki-repo', ref: branchName })
  await git.checkout({ fs, dir: '/kr0ki-repo', ref: branchName })
  
  // Notify user
  showNotification(`Agent created branch '${branchName}': ${reason}`)
}
```

---

## Implementation Plan

### Phase 1: Git Integration (Week 1)
- [ ] Add `isomorphic-git` dependency
- [ ] Create `gitClient.js` wrapper
- [ ] Initialize local repo on first diagram
- [ ] Commit diagram state on each change
- [ ] Fix Code Editor handoff to create initial commit

### Phase 2: Git-Based Revision Graph (Week 2)
- [ ] Replace `revisionGraph.js` with git-based model
- [ ] Read git log for commit history
- [ ] Support branches (list, create, switch)
- [ ] Update `StoryB00k.vue` to use git operations

### Phase 3: Git-Style Visualization (Week 3)
- [ ] Redesign `RevisionFlow.vue` layout
- [ ] Vertical timeline with branch columns
- [ ] Commit nodes with SHA, message, author
- [ ] Branch indicators and HEAD marker

### Phase 4: Branch Management UI (Week 4)
- [ ] Branch selector dropdown
- [ ] Create branch dialog
- [ ] Merge branch workflow
- [ ] Conflict resolution UI

### Phase 5: Remote Sync (Week 5-6)
- [ ] Add remote repository support
- [ ] Push/pull operations
- [ ] Conflict detection
- [ ] Collaboration features

---

## Benefits of Git-Based Approach

1. **Real Version Control**
   - Actual commits with SHAs
   - Persistent storage
   - Standard git operations

2. **Proper Branching**
   - Named branches
   - Easy switching
   - Merge support

3. **Familiar Mental Model**
   - Users understand git
   - Standard terminology
   - Existing tools work

4. **Collaboration Ready**
   - Push/pull to remote
   - Multiple users
   - Conflict resolution

5. **Agent Integration**
   - Agent can create branches
   - Agent can suggest merges
   - Agent can resolve conflicts

---

## Next Steps

1. **Immediate:** Fix Code Editor handoff to create initial commit
2. **Short-term:** Implement git-based revision graph
3. **Medium-term:** Git-style visualization
4. **Long-term:** Remote sync and collaboration

---

**Author:** Agent-assisted design session  
**Date:** 2026-09-29  
**Status:** Design complete, implementation pending
