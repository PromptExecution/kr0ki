# Kr0ki SVG Enrichment Layer - Priority #1

## Executive Summary

**Focus**: SVG enrichment and transformation layer
**Priority**: #1 - Core value proposition
**Delivery**: WASM module for embedding by ledgrrr, app4dog, and other tools
**Scope**: Graph representation, 3D isometric rendering, relationship maintenance

## What Kr0ki Does

Kr0ki is NOT a data analysis tool. It's a **graph representation engine** that:
- Uses linear solvers to maintain relationships
- Renders in 3D isometric plane
- Enriches SVG with interactive elements
- Provides WASM module for embedding

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│              Kr0ki SVG Enrichment Layer                  │
│                    (Rust + WASM)                         │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ SVG Parser   │  │ Graph Solver │  │ 3D Isometric │  │
│  │ (usvg)       │  │ (Relationship│  │ Renderer     │  │
│  │              │  │  Maintenance)│  │              │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
│                                                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ Enrichment   │  │ Animation    │  │ Interactive  │  │
│  │ Engine       │  │ Hooks        │  │ Elements     │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
│                                                          │
└─────────────────────────────────────────────────────────┘
                            │
                            ▼
                    WASM Module Export
                            │
            ┌───────────────┼───────────────┐
            ▼               ▼               ▼
    ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
    │ ledgrrr      │ │ app4dog      │ │ Other Tools  │
    │ (embeds WASM)│ │ (embeds WASM)│ │ (embed WASM) │
    └──────────────┘ └──────────────┘ └──────────────┘
```

## Core Components

### 1. SVG Parser (usvg + svgtypes)

**Purpose**: Parse and simplify SVG input
**Technology**: Rust usvg library
**Output**: Type-safe SVG tree structure

```rust
use usvg::{Tree, Options};
use svgtypes::{PathParser, Transform};

pub fn parse_svg(input: &str) -> Result<EnrichedSvg, Error> {
    let tree = Tree::from_str(input, &Options::default())?;
    Ok(EnrichedSvg::from_tree(tree))
}
```

**Features**:
- Parse SVG 1.1 and SVG 2.0
- Simplify complex paths
- Extract elements and attributes
- Build type-safe DOM tree

### 2. Graph Solver (Relationship Maintenance)

**Purpose**: Maintain relationships between graph elements
**Technology**: Linear solver (custom Rust implementation)
**Output**: Relationship graph with constraints

```rust
pub struct GraphSolver {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    constraints: Vec<Constraint>,
}

impl GraphSolver {
    pub fn add_node(&mut self, node: Node) {
        self.nodes.push(node);
    }
    
    pub fn add_edge(&mut self, from: NodeId, to: NodeId, constraint: Constraint) {
        self.edges.push(Edge { from, to, constraint });
        self.constraints.push(constraint);
    }
    
    pub fn solve(&mut self) -> Result<SolvedGraph, Error> {
        // Linear solver to maintain relationships
        // while respecting constraints
        let solution = self.linear_solve()?;
        Ok(SolvedGraph::from_solution(solution))
    }
}
```

**Features**:
- Maintain node relationships
- Enforce constraints (distance, angle, alignment)
- Linear solver for optimal layout
- Incremental updates (don't re-solve entire graph)

### 3. 3D Isometric Renderer

**Purpose**: Render graph in 3D isometric projection
**Technology**: Custom Rust renderer + tiny-skia
**Output**: 3D isometric SVG

```rust
pub struct IsometricRenderer {
    angle: f64,  // 30° for isometric
    scale: f64,
    offset: (f64, f64),
}

impl IsometricRenderer {
    pub fn render(&self, graph: &SolvedGraph) -> Svg {
        let mut svg = Svg::new();
        
        for node in &graph.nodes {
            let pos_3d = self.to_isometric(node.position);
            let element = self.render_node(node, pos_3d);
            svg.add(element);
        }
        
        for edge in &graph.edges {
            let element = self.render_edge(edge);
            svg.add(element);
        }
        
        svg
    }
    
    fn to_isometric(&self, pos: (f64, f64, f64)) -> (f64, f64) {
        // Isometric projection formula
        let x = (pos.0 - pos.1) * cos(30°) * self.scale + self.offset.0;
        let y = (pos.0 + pos.1) * sin(30°) * self.scale - pos.2 * self.scale + self.offset.1;
        (x, y)
    }
}
```

**Features**:
- 3D to 2D isometric projection
- Depth sorting (painter's algorithm)
- Customizable angle and scale
- Perspective correction

### 4. Enrichment Engine

**Purpose**: Add interactive elements and metadata
**Technology**: Custom Rust + WASM
**Output**: Enriched SVG with hooks

```rust
pub struct EnrichmentEngine {
    hooks: Vec<Hook>,
    animations: Vec<Animation>,
    metadata: HashMap<String, Value>,
}

impl EnrichmentEngine {
    pub fn enrich(&mut self, svg: &mut Svg) -> Result<(), Error> {
        // Add interactive hooks
        for hook in &self.hooks {
            self.add_hook(svg, hook)?;
        }
        
        // Add animation definitions
        for anim in &self.animations {
            self.add_animation(svg, anim)?;
        }
        
        // Add metadata
        self.add_metadata(svg, &self.metadata)?;
        
        Ok(())
    }
    
    fn add_hook(&self, svg: &mut Svg, hook: &Hook) -> Result<(), Error> {
        // Add data attributes for JavaScript hooks
        let element = svg.find_element(&hook.target)?;
        element.set_attribute("data-hook", &hook.name);
        element.set_attribute("data-action", &hook.action);
        Ok(())
    }
    
    fn add_animation(&self, svg: &mut Svg, anim: &Animation) -> Result<(), Error> {
        // Add SVG animation elements
        let anim_elem = match anim.kind {
            AnimationKind::Transform => self.create_transform_animation(anim),
            AnimationKind::Opacity => self.create_opacity_animation(anim),
            AnimationKind::Path => self.create_path_animation(anim),
        };
        svg.add(anim_elem);
        Ok(())
    }
}
```

**Features**:
- Add data attributes for JavaScript hooks
- Insert SVG animation elements
- Embed metadata (JSON-LD)
- Custom enrichment rules

### 5. Animation Hooks

**Purpose**: Enable external animation control
**Technology**: WASM exports + JavaScript bridge
**Output**: Animation API

```rust
#[wasm_bindgen]
pub fn add_animation_hook(
    svg: &str,
    element_id: &str,
    hook_name: &str,
) -> Result<String, JsValue> {
    let mut doc = parse_svg(svg)?;
    let element = doc.find_by_id(element_id)?;
    
    // Add animation hook
    element.set_attribute("data-animation-hook", hook_name);
    
    // Add default animation
    let anim = create_default_animation(hook_name);
    element.add_child(anim);
    
    Ok(doc.to_string())
}

#[wasm_bindgen]
pub fn trigger_animation(
    svg: &str,
    hook_name: &str,
) -> Result<String, JsValue> {
    let mut doc = parse_svg(svg)?;
    
    // Find element with hook
    let element = doc.find_by_attribute("data-animation-hook", hook_name)?;
    
    // Trigger animation (add class or modify attribute)
    element.set_attribute("data-animate", "true");
    
    Ok(doc.to_string())
}
```

**Features**:
- Add animation hooks to elements
- Trigger animations programmatically
- Custom animation definitions
- JavaScript API for control

### 6. Interactive Elements

**Purpose**: Add clickable/hoverable elements
**Technology**: WASM + JavaScript event handlers
**Output**: Interactive SVG

```rust
#[wasm_bindgen]
pub fn add_interactive_element(
    svg: &str,
    element_id: &str,
    on_click: &str,
    on_hover: &str,
) -> Result<String, JsValue> {
    let mut doc = parse_svg(svg)?;
    let element = doc.find_by_id(element_id)?;
    
    // Add event handlers
    element.set_attribute("onclick", on_click);
    element.set_attribute("onmouseover", on_hover);
    
    // Add cursor style
    element.set_attribute("style", "cursor: pointer;");
    
    Ok(doc.to_string())
}
```

**Features**:
- Click handlers
- Hover effects
- Tooltip support
- Custom event handlers

## WASM API

### Core Functions

```rust
// Parse and enrich SVG
#[wasm_bindgen]
pub fn enrich_svg(
    input: &str,
    config: JsValue,
) -> Result<String, JsValue> {
    let config: EnrichmentConfig = serde_wasm_bindgen::from_value(config)?;
    let mut engine = EnrichmentEngine::new(config);
    let mut svg = parse_svg(input)?;
    engine.enrich(&mut svg)?;
    Ok(svg.to_string())
}

// Render in 3D isometric
#[wasm_bindgen]
pub fn render_isometric(
    graph_json: &str,
    options: JsValue,
) -> Result<String, JsValue> {
    let graph: GraphData = serde_json::from_str(graph_json)?;
    let options: IsometricOptions = serde_wasm_bindgen::from_value(options)?;
    
    let mut solver = GraphSolver::new();
    solver.load_graph(graph);
    let solved = solver.solve()?;
    
    let renderer = IsometricRenderer::new(options);
    let svg = renderer.render(&solved);
    
    Ok(svg.to_string())
}

// Add animation hook
#[wasm_bindgen]
pub fn add_animation_hook(
    svg: &str,
    element_id: &str,
    hook_name: &str,
) -> Result<String, JsValue> {
    // Implementation above
}

// Trigger animation
#[wasm_bindgen]
pub fn trigger_animation(
    svg: &str,
    hook_name: &str,
) -> Result<String, JsValue> {
    // Implementation above
}
```

### JavaScript Usage

```javascript
import { enrich_svg, render_isometric, add_animation_hook } from './kr0ki_svg.wasm';

// Parse and enrich SVG
const enriched = await enrich_svg(rawSvg, {
    addHooks: true,
    addAnimations: true,
    metadata: { title: "My Diagram" }
});

// Render graph in 3D isometric
const isometricSvg = await render_isometric(graphJson, {
    angle: 30,
    scale: 1.0,
    offset: { x: 100, y: 100 }
});

// Add animation hook
const withHook = await add_animation_hook(enriched, "node-1", "pulse");

// Trigger animation (from JavaScript)
document.querySelector('[data-animation-hook="pulse"]').classList.add('animate');
```

## Implementation Plan

### Phase 1: Core Parser (Week 1-2)

**Goal**: Parse SVG and build type-safe tree
**Deliverables**:
- kr0ki-svg crate
- SVG parsing with usvg
- Type-safe tree structure
- Basic tests

**Tasks**:
1. Create kr0ki-svg crate
2. Add usvg, svgtypes dependencies
3. Implement parse_svg function
4. Build EnrichedSvg type
5. Write unit tests
6. Compile to WASM
7. Test in browser

**Success Criteria**:
- [ ] Parses SVG correctly
- [ ] Builds type-safe tree
- [ ] WASM module loads
- [ ] Tests pass

### Phase 2: Graph Solver (Week 3-4)

**Goal**: Maintain relationships with linear solver
**Deliverables**:
- GraphSolver implementation
- Constraint system
- Linear solver
- Tests

**Tasks**:
1. Design GraphSolver API
2. Implement node/edge structures
3. Add constraint system
4. Implement linear solver
5. Write solver tests
6. Integrate with parser
7. Test with real graphs

**Success Criteria**:
- [ ] Maintains relationships
- [ ] Enforces constraints
- [ ] Solver produces valid layout
- [ ] Tests pass

### Phase 3: 3D Isometric Renderer (Week 5-6)

**Goal**: Render graph in 3D isometric projection
**Deliverables**:
- IsometricRenderer implementation
- 3D to 2D projection
- Depth sorting
- Tests

**Tasks**:
1. Implement isometric projection
2. Add depth sorting
3. Render nodes and edges
4. Add customization (angle, scale)
5. Write renderer tests
6. Integrate with solver
7. Test with real data

**Success Criteria**:
- [ ] Correct isometric projection
- [ ] Proper depth sorting
- [ ] Customizable rendering
- [ ] Tests pass

### Phase 4: Enrichment Engine (Week 7-8)

**Goal**: Add interactive elements and hooks
**Deliverables**:
- EnrichmentEngine implementation
- Hook system
- Animation definitions
- Metadata embedding
- Tests

**Tasks**:
1. Design EnrichmentEngine API
2. Implement hook system
3. Add animation definitions
4. Embed metadata (JSON-LD)
5. Write enrichment tests
6. Integrate with renderer
7. Test end-to-end

**Success Criteria**:
- [ ] Adds hooks correctly
- [ ] Animations work
- [ ] Metadata embedded
- [ ] Tests pass

### Phase 5: WASM API & Integration (Week 9-10)

**Goal**: Export WASM API and integrate with tools
**Deliverables**:
- WASM module
- JavaScript bindings
- Documentation
- Integration examples

**Tasks**:
1. Add wasm-bindgen exports
2. Compile to WASM
3. Create JavaScript wrapper
4. Write API documentation
5. Create integration examples
6. Test with ledgrrr
7. Test with app4dog

**Success Criteria**:
- [ ] WASM module works
- [ ] JavaScript API functional
- [ ] Documentation complete
- [ ] Integration examples work

### Phase 6: Testing & Optimization (Week 11-12)

**Goal**: Comprehensive testing and performance optimization
**Deliverables**:
- Test suite
- Performance benchmarks
- Optimizations
- Production build

**Tasks**:
1. Write comprehensive tests
2. Benchmark performance
3. Optimize critical paths
4. Reduce WASM bundle size
5. Test with real diagrams
6. Fix bugs
7. Prepare production build

**Success Criteria**:
- [ ] All tests pass
- [ ] Performance acceptable
- [ ] Bundle size reasonable
- [ ] Production ready

## Technology Stack

### Core Libraries

| Library | Purpose | Version |
|---------|---------|---------|
| usvg | SVG parsing & simplification | 0.1 |
| svgtypes | Type-safe SVG structures | 0.1 |
| tiny-skia | 2D rendering | 0.11 |
| kurbo | 2D curves & paths | 0.10 |
| wasm-bindgen | JavaScript interop | 0.2 |
| serde | Serialization | 1.0 |
| serde-wasm-bindgen | WASM serialization | 0.6 |

### Build Tools

| Tool | Purpose |
|------|---------|
| wasm-pack | WASM compilation |
| cargo | Rust build system |
| vite | Frontend build (for testing) |

## Integration with ledgrrr & app4dog

### Embedding Pattern

```javascript
// ledgrrr/app4dog imports kr0ki WASM
import { enrich_svg, render_isometric } from '@kr0ki/svg-wasm';

// Each tool manages its own data
const myData = loadMyData();

// Use kr0ki for SVG enrichment only
const enriched = await enrich_svg(myData.svg, config);

// Render in 3D isometric if needed
const isometric = await render_isometric(myData.graph, options);

// Display result
document.getElementById('diagram').innerHTML = isometric;
```

### Data Separation

- **ledgrrr**: Manages ledger data, uses kr0ki for visualization
- **app4dog**: Manages app data, uses kr0ki for visualization
- **kr0ki**: Only handles SVG enrichment, no data storage

### Cost Sharing

- ledgrrr and app4dog share kr0ki development costs
- Each tool embeds kr0ki WASM (no server needed)
- kr0ki stabilizes the enrichment layer first
- Other tools integrate when stable

## Success Metrics

### Performance
- Parse SVG: < 100ms for 1MB file
- Solve graph: < 500ms for 1000 nodes
- Render isometric: < 200ms for 1000 nodes
- Enrich SVG: < 100ms for 100 elements
- WASM bundle size: < 500KB

### Quality
- 100% test coverage for core functions
- No memory leaks
- No panics in production
- Graceful error handling

### Integration
- Works in ledgrrr
- Works in app4dog
- JavaScript API documented
- Examples provided

## Next Steps

### Immediate (This Week)
1. **Create kr0ki-svg crate**
   ```bash
   cargo new crates/kr0ki-svg --lib
   ```

2. **Add dependencies**
   ```toml
   [dependencies]
   usvg = "0.1"
   svgtypes = "0.1"
   wasm-bindgen = "0.2"
   ```

3. **Implement basic parser**
   ```rust
   pub fn parse_svg(input: &str) -> Result<EnrichedSvg, Error> {
       // Implementation
   }
   ```

4. **Compile to WASM**
   ```bash
   wasm-pack build --target web
   ```

5. **Test in browser**
   - Create test HTML
   - Load WASM module
   - Call parse_svg
   - Verify output

### Week 1-2: Core Parser
- Complete Phase 1 tasks
- Deliver working SVG parser
- WASM module functional

### Week 3-4: Graph Solver
- Complete Phase 2 tasks
- Deliver relationship maintenance
- Linear solver working

### Week 5-6: 3D Renderer
- Complete Phase 3 tasks
- Deliver isometric rendering
- 3D projection working

### Week 7-12: Enrichment & Integration
- Complete Phase 4-6 tasks
- Deliver enrichment engine
- Integrate with ledgrrr/app4dog

## Conclusion

**Priority #1**: SVG enrichment layer
**Focus**: Graph representation, 3D isometric rendering, relationship maintenance
**Delivery**: WASM module for embedding
**Timeline**: 12 weeks to production

This is the core value proposition of kr0ki. By focusing on this, we deliver a powerful SVG enrichment engine that ledgrrr, app4dog, and other tools can embed to visualize their data in 3D isometric space with interactive elements and animations.

**Next Action**: Begin Phase 1 (Core Parser) implementation.
