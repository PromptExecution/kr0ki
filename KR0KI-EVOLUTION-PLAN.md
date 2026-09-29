# Kr0ki Evolution: Architecture Plan

## Executive Summary

Evolve kr0ki from a diagram rendering service into a comprehensive system status dashboard with intelligent SVG manipulation, semantic visualization, and agent-assisted development.

## Current State

- **kr0ki**: Diagram rendering service (Rust + Axum)
- **playbook**: Vue.js example browser
- **agent**: StoryB00k agent (Python)
- **Missing**: System status dashboard, SVG post-processing, semantic visualization

## Target State

1. **Main Dashboard** (Quasar/Vue.js)
   - Real-time system status
   - Service health monitoring
   - Plugin registry
   - Semantic visualization

2. **SVG Post-Processing** (Rust/WASM)
   - Intelligent SVG transformation
   - Interactive elements
   - Animation hooks
   - Custom graphics replacement

3. **Semantic Layer** (Rust + WASM)
   - Embedding visualization
   - Cosine distance mapping
   - SysML-v2 integration
   - Knowledge graph representation

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Kr0ki Main Dashboard                      │
│                    (Quasar/Vue.js + WASM)                    │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ System       │  │ Plugin       │  │ Semantic     │      │
│  │ Status       │  │ Registry     │  │ Visualizer   │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                    Kr0ki Core Services                       │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ kr0ki-server │  │ kr0ki-core   │  │ kr0ki-svg    │      │
│  │ (HTTP API)   │  │ (Rendering)  │  │ (WASM)       │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                    b00t Ecosystem                            │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ b00t         │  │ b00t         │  │ b00t         │      │
│  │ Registry     │  │ Plugins      │  │ Agent        │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└─────────────────────────────────────────────────────────────┘
```

## Component Design

### 1. Main Dashboard (Quasar/Vue.js)

**Technology Stack:**
- Quasar Framework (Vue 3)
- Vite (build tool)
- Pinia (state management)
- Vue Router
- WebSocket (real-time updates)

**Key Components:**

#### SystemStatus.vue
```vue
<template>
  <q-card>
    <q-card-section>
      <div class="text-h6">System Status</div>
    </q-card-section>
    <q-list>
      <q-item v-for="service in services" :key="service.name">
        <q-item-section>
          <q-item-label>{{ service.name }}</q-item-label>
          <q-item-label caption>{{ service.status }}</q-item-label>
        </q-item-section>
        <q-item-section side>
          <q-badge :color="statusColor(service.status)" />
        </q-item-section>
      </q-item>
    </q-list>
  </q-card>
</template>
```

**Features:**
- Real-time service health
- Configuration display
- Error reporting
- Performance metrics

#### PluginRegistry.vue
```vue
<template>
  <q-card>
    <q-card-section>
      <div class="text-h6">Plugin Registry</div>
    </q-card-section>
    <q-list>
      <q-item v-for="plugin in plugins" :key="plugin.id">
        <q-item-section>
          <q-item-label>{{ plugin.name }}</q-item-label>
          <q-item-label caption>{{ plugin.description }}</q-item-label>
        </q-item-section>
        <q-item-section side>
          <q-btn icon="settings" @click="configure(plugin)" />
        </q-item-section>
      </q-item>
    </q-list>
  </q-card>
</template>
```

**Features:**
- List registered plugins
- Enable/disable plugins
- Configure plugin settings
- View plugin documentation

#### SemanticVisualizer.vue
```vue
<template>
  <div ref="container" class="semantic-viz"></div>
</template>

<script setup>
import { onMounted, ref } from 'vue'
import * as d3 from 'd3'
import { init_wasm, visualize_embeddings } from 'kr0ki-semantic-wasm'

const container = ref(null)

onMounted(async () => {
  await init_wasm()
  visualize_embeddings(container.value)
})
</script>
```

**Features:**
- t-SNE/UMAP visualization
- Interactive node exploration
- Cosine distance display
- Topic clustering

### 2. SVG Post-Processing (Rust/WASM)

**Technology Stack:**
- Rust (core logic)
- wasm-pack (WASM compilation)
- usvg (SVG parsing)
- svgtypes (type-safe SVG)
- tiny-skia (rendering)
- wasm-bindgen (JS interop)

**Crate Structure:**

```
crates/
├── kr0ki-svg/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs          # WASM exports
│   │   ├── parser.rs       # SVG parsing
│   │   ├── transformer.rs  # SVG transformation
│   │   ├── animator.rs     # Animation hooks
│   │   └── interactor.rs   # Event handling
│   └── tests/
└── kr0ki-semantic/
    ├── Cargo.toml
    ├── src/
    │   ├── lib.rs          # WASM exports
    │   ├── embedding.rs    # Embedding operations
    │   ├── visualize.rs    # Visualization
    │   └── distance.rs     # Cosine distance
    └── tests/
```

**Key Functions:**

```rust
// kr0ki-svg/src/lib.rs
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn parse_svg(svg: &str) -> Result<JsValue, JsValue> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default())?;
    Ok(serde_wasm_bindgen::to_value(&tree)?)
}

#[wasm_bindgen]
pub fn transform_svg(tree: JsValue, transforms: JsValue) -> Result<String, JsValue> {
    // Apply transformations
    // Replace basic shapes with custom graphics
    // Add interactive elements
    Ok(transformed_svg)
}

#[wasm_bindgen]
pub fn add_animation_hooks(svg: &str, hooks: JsValue) -> Result<String, JsValue> {
    // Add animation triggers
    // Insert event listeners
    Ok(enhanced_svg)
}
```

**Features:**
- Parse SVG with usvg
- Transform SVG elements
- Add interactive hooks
- Generate animations
- Export enhanced SVG

### 3. Semantic Layer (Rust/WASM)

**Technology Stack:**
- Rust (core logic)
- wasm-pack (WASM compilation)
- ndarray (numerical computing)
- linfa (machine learning)
- wasm-bindgen (JS interop)

**Key Functions:**

```rust
// kr0ki-semantic/src/lib.rs
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn compute_embeddings(texts: JsValue) -> Result<JsValue, JsValue> {
    // Compute embeddings for text inputs
    // Return embedding vectors
    Ok(embeddings)
}

#[wasm_bindgen]
pub fn visualize_embeddings(embeddings: JsValue, container: &str) -> Result<(), JsValue> {
    // Apply t-SNE/UMAP
    // Render visualization
    Ok(())
}

#[wasm_bindgen]
pub fn compute_cosine_distance(a: JsValue, b: JsValue) -> f64 {
    // Compute cosine distance between vectors
    distance
}
```

**Features:**
- Compute embeddings
- Visualize with t-SNE/UMAP
- Compute distances
- Cluster topics
- Interactive exploration

## Implementation Phases

### Phase 1: Foundation (Week 1-2)

**Goals:**
- Set up Quasar project
- Create basic dashboard structure
- Implement system status display
- Connect to existing kr0ki services

**Deliverables:**
- Quasar project scaffold
- SystemStatus component
- WebSocket connection
- Basic styling

**Tasks:**
1. Create Quasar project
2. Set up Vite configuration
3. Create SystemStatus component
4. Implement WebSocket client
5. Connect to kr0ki health endpoints
6. Add basic styling

### Phase 2: SVG Post-Processing (Week 3-4)

**Goals:**
- Create kr0ki-svg crate
- Implement SVG parsing
- Add transformation capabilities
- Compile to WASM

**Deliverables:**
- kr0ki-svg crate
- WASM module
- Basic transformations
- Test suite

**Tasks:**
1. Create kr0ki-svg crate
2. Add usvg dependency
3. Implement SVG parsing
4. Add transformation logic
5. Compile to WASM
6. Write tests
7. Integrate with playbook

### Phase 3: Plugin Registry (Week 5-6)

**Goals:**
- Design plugin system
- Implement registry
- Create plugin interface
- Register existing plugins

**Deliverables:**
- Plugin registry API
- Plugin interface
- Plugin manager UI
- Registered plugins

**Tasks:**
1. Design plugin schema
2. Implement registry API
3. Create plugin interface
4. Build plugin manager UI
5. Register kr0ki plugins
6. Add plugin documentation

### Phase 4: Semantic Visualization (Week 7-8)

**Goals:**
- Create kr0ki-semantic crate
- Implement embedding computation
- Add visualization
- Integrate with dashboard

**Deliverables:**
- kr0ki-semantic crate
- WASM module
- Visualization component
- Integration tests

**Tasks:**
1. Create kr0ki-semantic crate
2. Add ndarray/linfa dependencies
3. Implement embedding computation
4. Add t-SNE/UMAP
5. Compile to WASM
6. Create visualization component
7. Integrate with dashboard
8. Write tests

### Phase 5: Integration & Testing (Week 9-10)

**Goals:**
- Integrate all components
- Add agent-assisted testing
- Performance optimization
- Documentation

**Deliverables:**
- Integrated dashboard
- Test suite
- Performance report
- Documentation

**Tasks:**
1. Integrate all components
2. Add NEO-CODER agent tests
3. Optimize performance
4. Write documentation
5. Create user guide
6. Deploy to staging

### Phase 6: Challenge & Review (Week 11-12)

**Goals:**
- Review architecture
- Identify gaps
- Plan improvements
- Prepare for production

**Deliverables:**
- Architecture review
- Gap analysis
- Improvement plan
- Production readiness report

**Tasks:**
1. Review architecture
2. Identify gaps
3. Plan improvements
4. Prepare production deployment
5. Create monitoring
6. Document lessons learned

## b00t Integration

### Plugin Registration

**Register kr0ki plugins:**

```bash
# Register kr0ki-svg plugin
b00t plugin register kr0ki-svg \
  --name "Kr0ki SVG Post-Processor" \
  --description "Intelligent SVG transformation and animation" \
  --version "0.1.0" \
  --type wasm \
  --entry "kr0ki_svg.wasm"

# Register kr0ki-semantic plugin
b00t plugin register kr0ki-semantic \
  --name "Kr0ki Semantic Visualizer" \
  --description "Embedding visualization and analysis" \
  --version "0.1.0" \
  --type wasm \
  --entry "kr0ki_semantic.wasm"
```

### b00t Stack Review

**Blessed Tools:**
- **Rendering**: kr0ki (existing)
- **Agent**: storyb00k (existing)
- **SVG Processing**: usvg, svgtypes, tiny-skia (Rust)
- **Visualization**: D3.js, t-SNE, UMAP
- **Animation**: GSAP, anime.js
- **Dashboard**: Quasar/Vue.js

**External Libraries to Use:**
1. **SVG Processing**:
   - usvg (Rust) - SVG parsing
   - svgtypes (Rust) - Type-safe SVG
   - tiny-skia (Rust) - Rendering
   - wasm-bindgen (Rust) - JS interop

2. **Visualization**:
   - D3.js (JavaScript) - General visualization
   - d3-force (JavaScript) - Force-directed graphs
   - umap-js (JavaScript) - UMAP implementation
   - tsne-js (JavaScript) - t-SNE implementation

3. **Animation**:
   - GSAP (JavaScript) - Professional animations
   - anime.js (JavaScript) - Lightweight animations

4. **Dashboard**:
   - Quasar (Vue.js) - UI framework
   - Pinia (Vue.js) - State management
   - Vue Router (Vue.js) - Routing

## Risk Assessment

### High Risk
1. **WASM Performance**: SVG processing in WASM may be slower than expected
   - **Mitigation**: Benchmark early, optimize critical paths
   
2. **Semantic Visualization Complexity**: t-SNE/UMAP can be computationally expensive
   - **Mitigation**: Use Web Workers, limit data size

3. **Plugin System Design**: Poor design could limit extensibility
   - **Mitigation**: Review with b00t team, iterate on design

### Medium Risk
1. **Quasar Learning Curve**: Team may not be familiar with Quasar
   - **Mitigation**: Allocate time for learning, use examples

2. **Integration Complexity**: Multiple WASM modules may conflict
   - **Mitigation**: Test integration early, isolate modules

3. **Agent Testing**: NEO-CODER may not understand complex requirements
   - **Mitigation**: Provide clear specifications, review tests

### Low Risk
1. **Documentation**: May be incomplete
   - **Mitigation**: Document as you go, review at end

2. **Performance**: Dashboard may be slow
   - **Mitigation**: Optimize queries, use caching

## Success Criteria

### Phase 1 (Foundation)
- [ ] Quasar project builds successfully
- [ ] SystemStatus displays real-time data
- [ ] WebSocket connection stable
- [ ] Basic styling complete

### Phase 2 (SVG Post-Processing)
- [ ] kr0ki-svg crate compiles
- [ ] WASM module loads in browser
- [ ] Basic transformations work
- [ ] Tests pass

### Phase 3 (Plugin Registry)
- [ ] Registry API functional
- [ ] Plugins can be registered
- [ ] UI displays plugins
- [ ] Configuration works

### Phase 4 (Semantic Visualization)
- [ ] kr0ki-semantic crate compiles
- [ ] WASM module loads
- [ ] Visualization renders
- [ ] Interactive features work

### Phase 5 (Integration)
- [ ] All components integrated
- [ ] Tests pass
- [ ] Performance acceptable
- [ ] Documentation complete

### Phase 6 (Review)
- [ ] Architecture reviewed
- [ ] Gaps identified
- [ ] Improvements planned
- [ ] Production ready

## Next Steps

1. **Immediate**: Start Phase 1 (Foundation)
2. **Week 1**: Set up Quasar project, create SystemStatus
3. **Week 2**: Implement WebSocket, connect to services
4. **Week 3**: Begin Phase 2 (SVG Post-Processing)
5. **Ongoing**: Review progress, adjust plan as needed

## Conclusion

This architecture plan provides a clear path to evolve kr0ki into a comprehensive system status dashboard with intelligent SVG manipulation and semantic visualization. By leveraging existing Rust libraries, WASM, and the b00t ecosystem, we can build a powerful, performant, and extensible platform.

The phased approach allows for incremental delivery, early validation, and risk mitigation. Each phase builds on the previous one, ensuring steady progress toward the target state.

Key success factors:
- Use existing libraries (don't reinvent)
- Leverage Rust/WASM for performance
- Integrate with b00t ecosystem
- Plan-challenge-review workflow
- Agent-assisted testing

With this plan, we're positioned to deliver a world-class system status dashboard that showcases kr0ki's capabilities and integrates seamlessly with the b00t ecosystem.
