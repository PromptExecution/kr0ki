//! The MCP-callable capabilities kr0ki-server exposes, and how each maps onto
//! an HTTP request (kr0ki#mcp-http-parity, 2026-09-16 design). `GET
//! /mcp/tools` (kr0ki-server) serializes `McpTool::ALL` so the stdio MCP
//! bridge (`containers/kr0ki-mcp/bridge.py`) can dispatch every `tools/call`
//! generically instead of hand-coding one branch per tool name.
//!
//! One closed enum for this family only — see
//! `docs/superpowers/specs/2026-09-16-mcp-http-parity-design.md` §1 for why
//! this deliberately isn't unified with `DiagramFormat` or generalized across
//! future families.

/// One callable capability kr0ki-server exposes over both HTTP and MCP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpTool {
    RenderDiagram,
    ListFormats,
    RenderKubeDiagram,
    RenderK8sTopology,
    RenderSysmlV2Snapshot,
    ImportReqIf,
    ImportReqIfUrl,
    ListModelProjects,
    ListModelCommits,
    GetModelSnapshot,
    QueryModelElements,
    GetModelRoots,
    QueryModelRelationships,
    QueryModelGraph,
    RecomputeAndEvaluate,
    SyncDigitalThread,
    ListDiagramTypes,
    SuggestDiagramType,
    NavigateUi,
    ValidateSysml,
    SysmlSymbols,
    SysmlSummary,
    SolveConstraints,
    ListRequirements,
    GetRequirement,
    TraceRequirement,
    RenderView,
    GetEvidence,
    ProposeChange,
    ValidateChange,
    CommitChange,
    RunVerification,
    GetAuditRecords,
}

impl McpTool {
    pub const ALL: &'static [McpTool] = &[
        Self::RenderDiagram,
        Self::ListFormats,
        Self::RenderKubeDiagram,
        Self::RenderK8sTopology,
        Self::RenderSysmlV2Snapshot,
        Self::ImportReqIf,
        Self::ImportReqIfUrl,
        Self::ListModelProjects,
        Self::ListModelCommits,
        Self::GetModelSnapshot,
        Self::QueryModelElements,
        Self::GetModelRoots,
        Self::QueryModelRelationships,
        Self::QueryModelGraph,
        Self::RecomputeAndEvaluate,
        Self::SyncDigitalThread,
        Self::ListDiagramTypes,
        Self::SuggestDiagramType,
        Self::NavigateUi,
        Self::ValidateSysml,
        Self::SysmlSymbols,
        Self::SysmlSummary,
        Self::SolveConstraints,
        Self::ListRequirements,
        Self::GetRequirement,
        Self::TraceRequirement,
        Self::RenderView,
        Self::GetEvidence,
        Self::ProposeChange,
        Self::ValidateChange,
        Self::CommitChange,
        Self::RunVerification,
        Self::GetAuditRecords,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::RenderDiagram => "render_diagram",
            Self::ListFormats => "list_formats",
            Self::RenderKubeDiagram => "render_kubernetes_manifest",
            Self::RenderK8sTopology => "render_kubernetes_topology",
            Self::RenderSysmlV2Snapshot => "render_sysmlv2_snapshot",
            Self::ImportReqIf => "import_reqif",
            Self::ImportReqIfUrl => "import_reqif_url",
            Self::ListModelProjects => "list_model_projects",
            Self::ListModelCommits => "list_model_commits",
            Self::GetModelSnapshot => "get_model_snapshot",
            Self::QueryModelElements => "query_model_elements",
            Self::GetModelRoots => "get_model_roots",
            Self::QueryModelRelationships => "query_model_relationships",
            Self::QueryModelGraph => "query_model_graph",
            Self::RecomputeAndEvaluate => "recompute_and_evaluate",
            Self::SyncDigitalThread => "sync_digital_thread",
            Self::ListDiagramTypes => "list_diagram_types",
            Self::SuggestDiagramType => "suggest_diagram_type",
            Self::NavigateUi => "navigate_ui",
            Self::ValidateSysml => "validate_sysml",
            Self::SysmlSymbols => "sysml_symbols",
            Self::SysmlSummary => "sysml_summary",
            Self::SolveConstraints => "solve_constraints",
            Self::ListRequirements => "list_requirements",
            Self::GetRequirement => "get_requirement",
            Self::TraceRequirement => "trace_requirement",
            Self::RenderView => "render_view",
            Self::GetEvidence => "get_evidence",
            Self::ProposeChange => "propose_change",
            Self::ValidateChange => "validate_change",
            Self::CommitChange => "commit_change",
            Self::RunVerification => "run_verification",
            Self::GetAuditRecords => "get_audit_records",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::RenderDiagram => {
                "Render supported diagram source through the local kr0ki service."
            }
            Self::ListFormats => "List formats currently supported by the local kr0ki service.",
            Self::RenderKubeDiagram => {
                "Render Kubernetes manifest YAML through the internal KubeDiagrams worker."
            }
            Self::RenderK8sTopology => {
                "Render Kubernetes manifest YAML through kr0ki's own recognizer -> UFO graph -> \
                 SysML v2 relation -> D2 pipeline (docs/PATTERNS-kubernetes.md), not the vendored \
                 KubeDiagrams tool."
            }
            Self::RenderSysmlV2Snapshot => {
                "Render a validated SysML v2 project commit from the configured model server."
            }
            Self::ImportReqIf => {
                "Validate and normalize a UTF-8 ReqIF document. For binary ReqIFz archives, use POST /requirements/import."
            }
            Self::ImportReqIfUrl => {
                "Fetch a ReqIF/ReqIFz artifact from an HTTPS URL (SSRF-hardened: DNS-resolved and range-checked before connecting) and validate/normalize it."
            }
            Self::ListModelProjects => "List SysML v2 projects on the configured model server.",
            Self::ListModelCommits => "List commits (immutable model snapshots) for a project.",
            Self::GetModelSnapshot => {
                "Fetch the full content-hashed element and root set for a project/commit."
            }
            Self::QueryModelElements => "List every element in a project/commit.",
            Self::GetModelRoots => "List the root element ids of a project/commit.",
            Self::QueryModelRelationships => {
                "List a model element's relationships (in/out/both direction)."
            }
            Self::QueryModelGraph => {
                "Query kr0ki-server's in-memory RDF graph (bounded query shapes, not SPARQL)."
            }
            Self::RecomputeAndEvaluate => {
                "Recompute a project's canonical graph from its latest commit, evaluate every \
                 RuleDocument element against it, and fold violations into its requirements \
                 graph as inferred, promotable Satisfies relations."
            }
            Self::SyncDigitalThread => {
                "Write a SysGraph's dbt:-prefixed nodes into a SysML v2 project as one commit \
                 (create/update/delete; other elements untouched). Diffs against the branch head \
                 and retries on concurrent commits."
            }
            Self::ListDiagramTypes => {
                "List the diagram types kr0ki can draw, each with the intent it serves (use_cases), a one-line \
                 'when to choose it', and its syntax. Optionally narrow to one use_case. Also returns the \
                 use-case vocabulary to ask the user about."
            }
            Self::SuggestDiagramType => {
                "Rank diagram types against the user's stated requirements (plain text) and return an explained \
                 shortlist. Deterministic keyword/intent evidence only: weigh it against what the user said \
                 before recommending."
            }
            Self::NavigateUi => {
                "Steer the user's playbook UI: open a view (gallery/editor/agent/setup), filter the gallery by \
                 use_case ('All' resets), highlight a shortlist (suggest) and/or select one diagram type. \
                 Reports how many UI sessions received it; 0 means the UI is not connected."
            }
            Self::ValidateSysml => {
                "Validate SysML v2 textual notation with the SysML MCP sidecar: syntax errors (line/column) and semantic \
                 diagnostics such as unresolved types and unverified requirements. Read-only; it does not know `satisfy` \
                 relations or short names and does not parse pure KerML."
            }
            Self::SysmlSymbols => {
                "List the symbols (parts, requirements, packages, ...) declared in SysML v2 text, with kinds, qualified names \
                 and types, via the SysML MCP sidecar."
            }
            Self::SysmlSummary => {
                "Summarise SysML v2 text: counts of elements by kind (part def, requirement, ...), via the SysML MCP sidecar."
            }
            Self::SolveConstraints => {
                "Solve the numeric constraints of a model with the SysMD sidecar (an interval constraint solver). Send SysML v2 / KerML \
                 text with attributes that have units and ranges; get back each variable's range as a closed interval (already widened \
                 outward for SysMD's 5-digit rounding, so it contains the true range) with its unit, plus the solver's issues and a \
                 verdict: consistent, inconsistent (a constraint cannot hold: the values are not a solution) or error (the model did \
                 not parse). Read-only. A range that is unbounded or empty is reported as such, never as a number."
            }
            Self::ListRequirements => {
                "List the assurance baseline's requirements, each with its status, owner, assurance state (unsatisfied / satisfied_untested / verified / failing / stale) and gap kinds. Filters narrow the result so only what is needed enters your context; call get_requirement for one in full."
            }
            Self::GetRequirement => {
                "Get one requirement in full: its nine profile fields, its thread (source obligations, satisfying elements, enforcing controls, verification cases, evidence with result and freshness), its assurance state and its gaps, all qualified by the model, implementation and configuration revisions they were computed at."
            }
            Self::TraceRequirement => {
                "Resolve one requirement's links - satisfying system elements against the current model revision, implementation paths and test targets against the repository - and report each as resolved or dangling with the reason. Use this to see implementation gaps."
            }
            Self::RenderView => {
                "Render the assurance view. Satisfaction assertions (dashed edges) and verification results (evidence nodes with result and freshness) are drawn apart. format: json (default), table, d2, or svg."
            }
            Self::GetEvidence => {
                "List revision-bound evidence records (result, the three revisions, artifact uri and digest, artifact integrity, freshness now). Filter by requirement and/or verification case."
            }
            Self::ProposeChange => {
                "Draft a change to the requirement baseline and return its diff and diagnostics WITHOUT committing. The body is JSON: {project_id, branch_id?, requirements:[{id,title,statement,source,source_kind,owner,rationale,verification_id,acceptance,status}]}. Each requirement is validated against the profile and linted (one 'shall', a responsible component, no vague terms); a profile error means no change is proposed. The result names the base revision the diff was computed against."
            }
            Self::ValidateChange => {
                "Check a drafted change: is it still based on the current model revision (current=true), or has the model moved on (stale)? Does not commit."
            }
            Self::CommitChange => {
                "Apply a drafted change IF AND ONLY IF the model head is expected_revision. A stale expected_revision is rejected (409, stale_base) with both revisions named and nothing written - re-read, then propose again. Requires the model.commit grant, checked by the service for every transport."
            }
            Self::RunVerification => {
                "Run a declared verification case and store revision-bound evidence. `revision` is the implementation revision you expect to be verifying; if the repository is at a different revision the run is refused (409). Only `cargo test -p <package> --test <name>` cases run; a run that executed zero tests is an error, not a pass."
            }
            Self::GetAuditRecords => {
                "Read the audit log: one record per tool invocation, permitted or denied, with caller, operation, decision, model revision and correlation id. Filter by caller, operation, decision, correlation_id or phase. Requires the audit.read grant. The result says whether the hash chain verifies."
            }
        }
    }

    pub fn input_schema(self) -> serde_json::Value {
        match self {
            Self::ValidateSysml | Self::SysmlSymbols | Self::SysmlSummary => serde_json::json!({
                "type": "object",
                "required": ["code"],
                "properties": {"code": {"type": "string", "description": "SysML v2 textual notation (max 256 KiB)."}}
            }),
            Self::SolveConstraints => serde_json::json!({
                "type": "object",
                "required": ["code"],
                "properties": {
                    "code": {"type": "string", "description": "SysML v2 / KerML / SysMD text (max 256 KiB)."},
                    "language": {"type": "string", "enum": ["sysml", "kerml", "sysmd"], "description": "Notation of `code`; default sysml."}
                }
            }),
            Self::ListRequirements => serde_json::json!({
                "type": "object",
                "properties": {
                    "status": {"type": "string", "enum": ["draft", "review", "validated", "gated", "implemented"]},
                    "owner": {"type": "string"},
                    "state": {"type": "string", "enum": ["unsatisfied", "satisfied_untested", "verified", "failing", "stale"]},
                    "gap": {"type": "string", "description": "Only requirements with this gap kind (e.g. control_not_implemented, satisfied_untested, stale, dangling_element)."}
                }
            }),
            Self::GetRequirement | Self::TraceRequirement => serde_json::json!({
                "type": "object", "required": ["id"],
                "properties": {"id": {"type": "string", "description": "Requirement id, e.g. KR-A01."}}
            }),
            Self::RenderView => serde_json::json!({
                "type": "object",
                "properties": {"format": {"type": "string", "enum": ["json", "table", "d2", "svg"], "default": "json"}}
            }),
            Self::GetEvidence => serde_json::json!({
                "type": "object",
                "properties": {
                    "requirement": {"type": "string", "description": "Requirement id."},
                    "case": {"type": "string", "description": "Verification case id, e.g. VC-A01."}
                }
            }),
            Self::ProposeChange => serde_json::json!({
                "type": "object", "required": ["change"],
                "properties": {"change": {"type": "string", "description": "JSON: {project_id, branch_id?, requirements:[{id,title,statement,source,source_kind,owner,rationale,verification_id,acceptance,status}]}."}}
            }),
            Self::ValidateChange => serde_json::json!({
                "type": "object", "required": ["change_id"],
                "properties": {"change_id": {"type": "string"}}
            }),
            Self::CommitChange => serde_json::json!({
                "type": "object", "required": ["change_id", "expected_revision"],
                "properties": {
                    "change_id": {"type": "string"},
                    "expected_revision": {"type": "string", "description": "The model revision you believe is current: the base_revision the proposal named. Rejected if the head differs."}
                }
            }),
            Self::RunVerification => serde_json::json!({
                "type": "object", "required": ["case_id", "revision"],
                "properties": {
                    "case_id": {"type": "string", "description": "Verification case id, e.g. VC-A01."},
                    "revision": {"type": "string", "description": "The implementation revision you expect to verify."}
                }
            }),
            Self::GetAuditRecords => serde_json::json!({
                "type": "object",
                "properties": {
                    "caller": {"type": "string"}, "operation": {"type": "string"},
                    "decision": {"type": "string", "enum": ["permit", "deny"]},
                    "correlation_id": {"type": "string"},
                    "phase": {"type": "string", "enum": ["decision", "outcome"]}
                }
            }),
            Self::ListDiagramTypes => serde_json::json!({
                "type": "object",
                "properties": {
                    "use_case": {"type": "string", "description": "Only types serving this intent (e.g. 'data model'). Omit or 'All' for every type."}
                }
            }),
            Self::SuggestDiagramType => serde_json::json!({
                "type": "object",
                "required": ["requirements"],
                "properties": {
                    "requirements": {"type": "string", "description": "What the user wants to convey, in their words (max 16 KiB)."},
                    "use_cases": {"type": "string", "description": "Comma-separated intents already confirmed with the user."},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 10, "default": 5}
                }
            }),
            Self::NavigateUi => serde_json::json!({
                "type": "object",
                "required": ["session_id"],
                "properties": {
                    "session_id": {"type": "string", "description": "The playbook UI session (shown in the Planner panel; the storyb00k agent fills it in for you)."},
                    "view": {"type": "string", "enum": ["gallery", "editor", "agent", "setup"]},
                    "use_case": {"type": "string", "description": "Filter the gallery by this intent; 'All' resets."},
                    "type_id": {"type": "string", "description": "Select this diagram type card."},
                    "suggest": {"type": "string", "description": "Comma-separated type ids to highlight as the planner's shortlist (max 8)."},
                    "note": {"type": "string", "description": "Short reason shown with the shortlist (max 280 chars)."}
                }
            }),
            Self::RenderDiagram => serde_json::json!({
                "type": "object",
                "required": ["format", "source"],
                "properties": {
                    "format": {"type": "string", "description": "kr0ki diagram format slug."},
                    "source": {"type": "string", "description": "UTF-8 diagram source."},
                    "output": {"type": "string", "enum": ["svg", "png"], "default": "svg"}
                }
            }),
            Self::ListFormats => serde_json::json!({"type": "object", "properties": {}}),
            Self::RenderKubeDiagram => serde_json::json!({
                "type": "object",
                "required": ["manifest"],
                "properties": {
                    "manifest": {"type": "string", "description": "Kubernetes YAML manifest bundle."},
                    "output": {"type": "string", "enum": ["svg", "dot_json"], "default": "svg"}
                }
            }),
            Self::RenderK8sTopology => serde_json::json!({
                "type": "object",
                "required": ["manifest"],
                "properties": {
                    "manifest": {"type": "string", "description": "Kubernetes multi-doc YAML manifest bundle."},
                    "output": {"type": "string", "enum": ["svg", "png"], "default": "svg"},
                    "view": {"type": "string", "description": "Optional SysmlViewKind slug (e.g. \"interconnection\", \"action_flow\") to render only that view. Omit to render all relations together."}
                }
            }),
            Self::RenderSysmlV2Snapshot => serde_json::json!({
                "type": "object",
                "required": ["project_id", "commit_id"],
                "properties": {
                    "project_id": {"type": "string", "description": "SysML v2 project id."},
                    "commit_id": {"type": "string", "description": "Immutable SysML v2 commit id."},
                    "output": {"type": "string", "enum": ["svg", "png"], "default": "svg"},
                    "view": {"type": "string", "description": "Optional SysmlViewKind slug (e.g. \"interconnection\", \"action_flow\") to render only that view. Omit to render all relations together."}
                }
            }),
            Self::ImportReqIf => serde_json::json!({
                "type": "object",
                "required": ["artifact"],
                "properties": {
                    "artifact": {
                        "type": "string",
                        "description": "UTF-8 ReqIF XML. Use the HTTP upload route for binary ReqIFz."
                    }
                }
            }),
            Self::ImportReqIfUrl => serde_json::json!({
                "type": "object",
                "required": ["url"],
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "HTTPS URL to fetch a ReqIF or ReqIFz artifact from. Only https:// is accepted."
                    }
                }
            }),
            Self::ListModelProjects => serde_json::json!({"type": "object", "properties": {}}),
            Self::ListModelCommits => serde_json::json!({
                "type": "object", "required": ["project_id"],
                "properties": {"project_id": {"type": "string", "description": "SysML v2 project id."}}
            }),
            Self::GetModelSnapshot | Self::QueryModelElements | Self::GetModelRoots => {
                serde_json::json!({
                    "type": "object", "required": ["project_id", "commit_id"],
                    "properties": {"project_id": {"type": "string"}, "commit_id": {"type": "string"}}
                })
            }
            Self::QueryModelRelationships => serde_json::json!({
                "type": "object", "required": ["project_id", "commit_id", "element_id"],
                "properties": {
                    "project_id": {"type": "string"}, "commit_id": {"type": "string"},
                    "element_id": {"type": "string"},
                    "direction": {"type": "string", "enum": ["in", "out", "both"], "default": "both"}
                }
            }),
            Self::QueryModelGraph => serde_json::json!({
                "type": "object", "required": ["shape"],
                "properties": {
                    "shape": {"type": "string", "enum": ["triples_about", "related_via"]},
                    "subject": {"type": "string", "description": "Element id (IRI local name) to query about."}
                }
            }),
            Self::RecomputeAndEvaluate => serde_json::json!({
                "type": "object",
                "required": ["project_id"],
                "properties": {
                    "project_id": {"type": "string", "description": "SysML v2 project id."}
                }
            }),
            Self::SyncDigitalThread => serde_json::json!({
                "type": "object",
                "required": ["project_id", "graph"],
                "properties": {
                    "project_id": {"type": "string", "description": "SysML v2 project id."},
                    "branch_id": {"type": "string", "description": "Target branch; defaults to the project's default branch."},
                    "graph": {"type": "string", "description": "The SysGraph, serialized as JSON."}
                }
            }),
        }
    }

    pub const fn http_binding(self) -> HttpBinding {
        match self {
            Self::ValidateSysml => HttpBinding {
                method: HttpMethod::Post,
                path_template: "/sysml/validate",
                args: &[ArgBinding { name: "code", placement: ArgPlacement::Body }],
            },
            Self::SysmlSymbols => HttpBinding {
                method: HttpMethod::Post,
                path_template: "/sysml/symbols",
                args: &[ArgBinding { name: "code", placement: ArgPlacement::Body }],
            },
            Self::SysmlSummary => HttpBinding {
                method: HttpMethod::Post,
                path_template: "/sysml/summary",
                args: &[ArgBinding { name: "code", placement: ArgPlacement::Body }],
            },
            Self::SolveConstraints => HttpBinding {
                method: HttpMethod::Post,
                path_template: "/sysmd/solve",
                args: &[
                    ArgBinding { name: "code", placement: ArgPlacement::Body },
                    ArgBinding { name: "language", placement: ArgPlacement::Query },
                ],
            },
            Self::ListDiagramTypes => HttpBinding {
                method: HttpMethod::Get,
                path_template: "/api/catalog",
                args: &[ArgBinding { name: "use_case", placement: ArgPlacement::Query }],
            },
            Self::SuggestDiagramType => HttpBinding {
                method: HttpMethod::Post,
                path_template: "/api/catalog/suggest",
                args: &[
                    ArgBinding { name: "requirements", placement: ArgPlacement::Body },
                    ArgBinding { name: "use_cases", placement: ArgPlacement::Query },
                    ArgBinding { name: "limit", placement: ArgPlacement::Query },
                ],
            },
            Self::NavigateUi => HttpBinding {
                method: HttpMethod::Post,
                path_template: "/ui/{session_id}/navigate",
                args: &[
                    ArgBinding { name: "session_id", placement: ArgPlacement::Path },
                    ArgBinding { name: "view", placement: ArgPlacement::Query },
                    ArgBinding { name: "use_case", placement: ArgPlacement::Query },
                    ArgBinding { name: "type_id", placement: ArgPlacement::Query },
                    ArgBinding { name: "suggest", placement: ArgPlacement::Query },
                    ArgBinding { name: "note", placement: ArgPlacement::Query },
                ],
            },
            Self::RenderDiagram => HttpBinding {
                method: HttpMethod::Post,
                path_template: "/render/{format}",
                args: &[
                    ArgBinding {
                        name: "format",
                        placement: ArgPlacement::Path,
                    },
                    ArgBinding {
                        name: "source",
                        placement: ArgPlacement::Body,
                    },
                    ArgBinding {
                        name: "output",
                        placement: ArgPlacement::Query,
                    },
                ],
            },
            Self::ListFormats => HttpBinding {
                method: HttpMethod::Get,
                path_template: "/formats",
                args: &[],
            },
            Self::RenderKubeDiagram => HttpBinding {
                method: HttpMethod::Post,
                path_template: "/render/kubediagram",
                args: &[
                    ArgBinding {
                        name: "manifest",
                        placement: ArgPlacement::Body,
                    },
                    ArgBinding {
                        name: "output",
                        placement: ArgPlacement::Query,
                    },
                ],
            },
            Self::RenderK8sTopology => HttpBinding {
                method: HttpMethod::Post,
                path_template: "/render/k8s-topology",
                args: &[
                    ArgBinding {
                        name: "manifest",
                        placement: ArgPlacement::Body,
                    },
                    ArgBinding {
                        name: "output",
                        placement: ArgPlacement::Query,
                    },
                    ArgBinding {
                        name: "view",
                        placement: ArgPlacement::Query,
                    },
                ],
            },
            Self::RenderSysmlV2Snapshot => HttpBinding {
                method: HttpMethod::Post,
                path_template: "/render/sysmlv2/projects/{project_id}/commits/{commit_id}",
                args: &[
                    ArgBinding { name: "project_id", placement: ArgPlacement::Path },
                    ArgBinding { name: "commit_id", placement: ArgPlacement::Path },
                    ArgBinding { name: "output", placement: ArgPlacement::Query },
                    ArgBinding { name: "view", placement: ArgPlacement::Query },
                ],
            },
            Self::ImportReqIf => HttpBinding {
                method: HttpMethod::Post,
                path_template: "/requirements/import",
                args: &[ArgBinding {
                    name: "artifact",
                    placement: ArgPlacement::Body,
                }],
            },
            Self::ImportReqIfUrl => HttpBinding {
                method: HttpMethod::Post,
                path_template: "/requirements/import/url",
                args: &[ArgBinding {
                    name: "url",
                    placement: ArgPlacement::Body,
                }],
            },
            Self::ListModelProjects => HttpBinding { method: HttpMethod::Get, path_template: "/model/projects", args: &[] },
            Self::ListModelCommits => HttpBinding {
                method: HttpMethod::Get, path_template: "/model/projects/{project_id}/commits",
                args: &[ArgBinding { name: "project_id", placement: ArgPlacement::Path }],
            },
            Self::GetModelSnapshot => HttpBinding {
                method: HttpMethod::Get, path_template: "/model/projects/{project_id}/commits/{commit_id}/snapshot",
                args: &[ArgBinding { name: "project_id", placement: ArgPlacement::Path }, ArgBinding { name: "commit_id", placement: ArgPlacement::Path }],
            },
            Self::QueryModelElements => HttpBinding {
                method: HttpMethod::Get, path_template: "/model/projects/{project_id}/commits/{commit_id}/elements",
                args: &[ArgBinding { name: "project_id", placement: ArgPlacement::Path }, ArgBinding { name: "commit_id", placement: ArgPlacement::Path }],
            },
            Self::GetModelRoots => HttpBinding {
                method: HttpMethod::Get, path_template: "/model/projects/{project_id}/commits/{commit_id}/roots",
                args: &[ArgBinding { name: "project_id", placement: ArgPlacement::Path }, ArgBinding { name: "commit_id", placement: ArgPlacement::Path }],
            },
            Self::QueryModelRelationships => HttpBinding {
                method: HttpMethod::Get,
                path_template: "/model/projects/{project_id}/commits/{commit_id}/elements/{element_id}/relationships",
                args: &[
                    ArgBinding { name: "project_id", placement: ArgPlacement::Path },
                    ArgBinding { name: "commit_id", placement: ArgPlacement::Path },
                    ArgBinding { name: "element_id", placement: ArgPlacement::Path },
                    ArgBinding { name: "direction", placement: ArgPlacement::Query },
                ],
            },
            Self::QueryModelGraph => HttpBinding {
                method: HttpMethod::Get, path_template: "/model/graph/query",
                args: &[
                    ArgBinding { name: "shape", placement: ArgPlacement::Query },
                    ArgBinding { name: "subject", placement: ArgPlacement::Query },
                ],
            },
            Self::RecomputeAndEvaluate => HttpBinding {
                method: HttpMethod::Post,
                path_template: "/model/projects/{project_id}/recompute",
                args: &[ArgBinding {
                    name: "project_id",
                    placement: ArgPlacement::Path,
                }],
            },
            Self::ListRequirements => HttpBinding {
                method: HttpMethod::Get,
                path_template: "/assurance/requirements",
                args: &[
                    ArgBinding { name: "status", placement: ArgPlacement::Query },
                    ArgBinding { name: "owner", placement: ArgPlacement::Query },
                    ArgBinding { name: "state", placement: ArgPlacement::Query },
                    ArgBinding { name: "gap", placement: ArgPlacement::Query }
                ],
            },
            Self::GetRequirement => HttpBinding {
                method: HttpMethod::Get,
                path_template: "/assurance/requirements/{id}",
                args: &[
                    ArgBinding { name: "id", placement: ArgPlacement::Path }
                ],
            },
            Self::TraceRequirement => HttpBinding {
                method: HttpMethod::Get,
                path_template: "/assurance/requirements/{id}/trace",
                args: &[
                    ArgBinding { name: "id", placement: ArgPlacement::Path }
                ],
            },
            Self::RenderView => HttpBinding {
                method: HttpMethod::Get,
                path_template: "/assurance/view",
                args: &[
                    ArgBinding { name: "format", placement: ArgPlacement::Query }
                ],
            },
            Self::GetEvidence => HttpBinding {
                method: HttpMethod::Get,
                path_template: "/assurance/evidence",
                args: &[
                    ArgBinding { name: "requirement", placement: ArgPlacement::Query },
                    ArgBinding { name: "case", placement: ArgPlacement::Query }
                ],
            },
            Self::ProposeChange => HttpBinding {
                method: HttpMethod::Post,
                path_template: "/assurance/changes",
                args: &[
                    ArgBinding { name: "change", placement: ArgPlacement::Body }
                ],
            },
            Self::ValidateChange => HttpBinding {
                method: HttpMethod::Get,
                path_template: "/assurance/changes/{change_id}",
                args: &[
                    ArgBinding { name: "change_id", placement: ArgPlacement::Path }
                ],
            },
            Self::CommitChange => HttpBinding {
                method: HttpMethod::Post,
                path_template: "/assurance/changes/{change_id}/commit",
                args: &[
                    ArgBinding { name: "change_id", placement: ArgPlacement::Path },
                    ArgBinding { name: "expected_revision", placement: ArgPlacement::Query }
                ],
            },
            Self::RunVerification => HttpBinding {
                method: HttpMethod::Post,
                path_template: "/assurance/verify/{case_id}",
                args: &[
                    ArgBinding { name: "case_id", placement: ArgPlacement::Path },
                    ArgBinding { name: "revision", placement: ArgPlacement::Query }
                ],
            },
            Self::GetAuditRecords => HttpBinding {
                method: HttpMethod::Get,
                path_template: "/assurance/audit",
                args: &[
                    ArgBinding { name: "caller", placement: ArgPlacement::Query },
                    ArgBinding { name: "operation", placement: ArgPlacement::Query },
                    ArgBinding { name: "decision", placement: ArgPlacement::Query },
                    ArgBinding { name: "correlation_id", placement: ArgPlacement::Query },
                    ArgBinding { name: "phase", placement: ArgPlacement::Query }
                ],
            },
            Self::SyncDigitalThread => HttpBinding {
                method: HttpMethod::Post,
                path_template: "/model/projects/{project_id}/sync",
                args: &[
                    ArgBinding {
                        name: "project_id",
                        placement: ArgPlacement::Path,
                    },
                    ArgBinding {
                        name: "branch_id",
                        placement: ArgPlacement::Query,
                    },
                    ArgBinding {
                        name: "graph",
                        placement: ArgPlacement::Body,
                    },
                ],
            },
        }
    }

    /// The full manifest entry `GET /mcp/tools` serves for this tool — both
    /// the MCP schema half (used verbatim as an MCP `tools/list` entry) and
    /// the HTTP binding half (used by the stdio bridge's generic dispatcher
    /// to place a `tools/call`'s arguments on the wire). See module docs on
    /// why these aren't split.
    pub fn to_manifest_json(self) -> serde_json::Value {
        let binding = self.http_binding();
        serde_json::json!({
            "name": self.name(),
            "description": self.description(),
            "inputSchema": self.input_schema(),
            "httpBinding": {
                "method": match binding.method {
                    HttpMethod::Get => "GET",
                    HttpMethod::Post => "POST",
                },
                "pathTemplate": binding.path_template,
                "args": binding.args.iter().map(|a| serde_json::json!({
                    "name": a.name,
                    "placement": match a.placement {
                        ArgPlacement::Path => "path",
                        ArgPlacement::Query => "query",
                        ArgPlacement::Body => "body",
                    }
                })).collect::<Vec<_>>()
            }
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum HttpMethod {
    Get,
    Post,
}

/// Where in the HTTP request a `tools/call` argument by this name lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgPlacement {
    /// Substituted into the path template at `{name}`.
    Path,
    /// Becomes a `?name=value` query parameter.
    Query,
    /// This argument's string value becomes the entire raw request body.
    Body,
}

#[derive(Debug, Clone, Copy)]
pub struct ArgBinding {
    pub name: &'static str,
    pub placement: ArgPlacement,
}

#[derive(Debug, Clone, Copy)]
pub struct HttpBinding {
    pub method: HttpMethod,
    pub path_template: &'static str,
    pub args: &'static [ArgBinding],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recompute_and_evaluate_binds_to_the_post_recompute_route() {
        let binding = McpTool::RecomputeAndEvaluate.http_binding();
        assert!(matches!(binding.method, HttpMethod::Post));
        assert_eq!(
            binding.path_template,
            "/model/projects/{project_id}/recompute"
        );
        assert_eq!(binding.args.len(), 1);
        assert_eq!(binding.args[0].name, "project_id");
        assert!(matches!(binding.args[0].placement, ArgPlacement::Path));
    }

    #[test]
    fn recompute_and_evaluate_is_listed_in_all() {
        assert!(McpTool::ALL.contains(&McpTool::RecomputeAndEvaluate));
    }

    #[test]
    fn import_reqif_url_binds_url_to_body_as_a_post() {
        let binding = McpTool::ImportReqIfUrl.http_binding();
        assert!(matches!(binding.method, HttpMethod::Post));
        assert_eq!(binding.path_template, "/requirements/import/url");
        assert_eq!(binding.args.len(), 1);
        assert!(binding
            .args
            .iter()
            .any(|a| a.name == "url" && matches!(a.placement, ArgPlacement::Body)));
    }
    #[test]
    fn assurance_tools_bind_to_the_assurance_routes() {
        let b = |t: McpTool| {
            let h = t.http_binding();
            let m = match h.method {
                HttpMethod::Get => "GET",
                HttpMethod::Post => "POST",
            };
            let args: Vec<(&str, &str)> = h
                .args
                .iter()
                .map(|a| {
                    (
                        a.name,
                        match a.placement {
                            ArgPlacement::Path => "path",
                            ArgPlacement::Query => "query",
                            ArgPlacement::Body => "body",
                        },
                    )
                })
                .collect();
            (m, h.path_template, args)
        };
        assert_eq!(
            b(McpTool::GetRequirement),
            ("GET", "/assurance/requirements/{id}", vec![("id", "path")])
        );
        assert_eq!(
            b(McpTool::TraceRequirement),
            (
                "GET",
                "/assurance/requirements/{id}/trace",
                vec![("id", "path")]
            )
        );
        assert_eq!(
            b(McpTool::RenderView),
            ("GET", "/assurance/view", vec![("format", "query")])
        );
        assert_eq!(
            b(McpTool::CommitChange),
            (
                "POST",
                "/assurance/changes/{change_id}/commit",
                vec![("change_id", "path"), ("expected_revision", "query")]
            ),
            "the commit tool carries the expected revision, so a stale caller can be refused"
        );
        assert_eq!(
            b(McpTool::RunVerification),
            (
                "POST",
                "/assurance/verify/{case_id}",
                vec![("case_id", "path"), ("revision", "query")]
            )
        );
        assert_eq!(
            b(McpTool::ProposeChange),
            ("POST", "/assurance/changes", vec![("change", "body")])
        );
        assert_eq!(
            b(McpTool::ValidateChange).0,
            "GET",
            "validating a draft must not be a write"
        );
        assert_eq!(b(McpTool::GetAuditRecords).1, "/assurance/audit");
        assert_eq!(b(McpTool::GetEvidence).1, "/assurance/evidence");
        assert_eq!(b(McpTool::ListRequirements).1, "/assurance/requirements");
    }

    #[test]
    fn every_required_argument_in_a_schema_has_a_binding() {
        // A required argument with nowhere to go on the wire would be silently dropped.
        for t in McpTool::ALL {
            let schema = t.input_schema();
            let bound: Vec<&str> = t.http_binding().args.iter().map(|a| a.name).collect();
            for r in schema["required"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str())
            {
                assert!(
                    bound.contains(&r),
                    "{}: required `{r}` has no HTTP binding",
                    t.name()
                );
            }
        }
    }

    #[test]
    fn all_tools_have_unique_names() {
        let mut names: Vec<&str> = McpTool::ALL.iter().map(|t| t.name()).collect();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), before, "duplicate McpTool name in ALL");
        assert_eq!(McpTool::ALL.len(), 33);
    }

    #[test]

    fn render_sysmlv2_snapshot_binds_only_model_identity_and_output() {
        let binding = McpTool::RenderSysmlV2Snapshot.http_binding();
        assert!(matches!(binding.method, HttpMethod::Post));
        assert_eq!(
            binding.path_template,
            "/render/sysmlv2/projects/{project_id}/commits/{commit_id}"
        );
        assert_eq!(binding.args.len(), 4);
        assert!(binding
            .args
            .iter()
            .any(|arg| arg.name == "project_id" && matches!(arg.placement, ArgPlacement::Path)));
        assert!(binding
            .args
            .iter()
            .any(|arg| arg.name == "commit_id" && matches!(arg.placement, ArgPlacement::Path)));
        assert!(binding
            .args
            .iter()
            .any(|arg| arg.name == "output" && matches!(arg.placement, ArgPlacement::Query)));
        assert!(binding
            .args
            .iter()
            .any(|arg| arg.name == "view" && matches!(arg.placement, ArgPlacement::Query)));
        assert!(binding
            .args
            .iter()
            .all(|arg| !matches!(arg.placement, ArgPlacement::Body)));
        // `view` is optional, so it must not appear in the schema's `required`.
        let schema = McpTool::RenderSysmlV2Snapshot.input_schema();
        assert!(!schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("view")));
        assert!(schema["properties"]["view"].is_object());
    }

    #[test]

    fn render_k8s_topology_binds_manifest_to_body_output_and_view_to_query() {
        let binding = McpTool::RenderK8sTopology.http_binding();
        assert!(matches!(binding.method, HttpMethod::Post));
        assert_eq!(binding.path_template, "/render/k8s-topology");
        assert_eq!(binding.args.len(), 3);
        assert!(binding
            .args
            .iter()
            .any(|a| a.name == "manifest" && matches!(a.placement, ArgPlacement::Body)));
        assert!(binding
            .args
            .iter()
            .any(|a| a.name == "output" && matches!(a.placement, ArgPlacement::Query)));
        assert!(binding
            .args
            .iter()
            .any(|a| a.name == "view" && matches!(a.placement, ArgPlacement::Query)));
        // Unlike RenderKubeDiagram (svg/dot_json, the vendored tool's own output
        // kinds), this route renders through kr0ki's own service, so its output
        // enum matches RenderDiagram's (svg/png).
        let schema = McpTool::RenderK8sTopology.input_schema();
        assert_eq!(
            schema["properties"]["output"]["enum"],
            serde_json::json!(["svg", "png"])
        );
        // `view` is optional, so it must not appear in the schema's `required`.
        assert!(!schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("view")));
        assert!(schema["properties"]["view"].is_object());
    }

    #[test]

    fn render_diagram_binds_format_to_path_source_to_body_output_to_query() {
        let binding = McpTool::RenderDiagram.http_binding();
        assert!(matches!(binding.method, HttpMethod::Post));
        assert_eq!(binding.path_template, "/render/{format}");
        assert_eq!(binding.args.len(), 3);
        assert!(binding
            .args
            .iter()
            .any(|a| a.name == "format" && matches!(a.placement, ArgPlacement::Path)));
        assert!(binding
            .args
            .iter()
            .any(|a| a.name == "source" && matches!(a.placement, ArgPlacement::Body)));
        assert!(binding
            .args
            .iter()
            .any(|a| a.name == "output" && matches!(a.placement, ArgPlacement::Query)));
    }

    #[test]

    fn list_formats_binds_to_a_plain_get_with_no_args() {
        let binding = McpTool::ListFormats.http_binding();
        assert!(matches!(binding.method, HttpMethod::Get));
        assert_eq!(binding.path_template, "/formats");
        assert!(binding.args.is_empty());
    }

    #[test]

    fn render_kube_diagram_binds_manifest_to_body_output_to_query() {
        let binding = McpTool::RenderKubeDiagram.http_binding();
        assert!(matches!(binding.method, HttpMethod::Post));
        assert_eq!(binding.path_template, "/render/kubediagram");
        assert_eq!(binding.args.len(), 2);
        assert!(binding
            .args
            .iter()
            .any(|a| a.name == "manifest" && matches!(a.placement, ArgPlacement::Body)));
        assert!(binding
            .args
            .iter()
            .any(|a| a.name == "output" && matches!(a.placement, ArgPlacement::Query)));
    }

    #[test]

    fn to_manifest_json_carries_both_the_mcp_schema_and_the_http_binding() {
        let json = McpTool::RenderKubeDiagram.to_manifest_json();
        assert_eq!(json["name"], "render_kubernetes_manifest");
        assert!(json["description"].is_string());
        assert!(json["inputSchema"]["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("manifest")));
        assert_eq!(json["httpBinding"]["method"], "POST");
        assert_eq!(json["httpBinding"]["pathTemplate"], "/render/kubediagram");
        let args = json["httpBinding"]["args"].as_array().unwrap();
        assert!(args
            .iter()
            .any(|a| a["name"] == "manifest" && a["placement"] == "body"));
    }

    #[test]

    fn model_tools_have_the_expected_get_bindings() {
        let commits = McpTool::ListModelCommits.http_binding();
        assert!(matches!(commits.method, HttpMethod::Get));
        assert_eq!(
            commits.path_template,
            "/model/projects/{project_id}/commits"
        );
        assert_eq!(commits.args[0].name, "project_id");

        let relationships = McpTool::QueryModelRelationships.http_binding();
        assert_eq!(relationships.args.len(), 4);
        assert!(relationships
            .args
            .iter()
            .any(|arg| arg.name == "direction" && matches!(arg.placement, ArgPlacement::Query)));

        let graph = McpTool::QueryModelGraph.http_binding();
        assert_eq!(graph.path_template, "/model/graph/query");
        assert_eq!(graph.args.len(), 2);
    }

    #[test]
    fn sync_digital_thread_binds_graph_to_body_and_branch_to_query() {
        let binding = McpTool::SyncDigitalThread.http_binding();
        assert!(matches!(binding.method, HttpMethod::Post));
        assert_eq!(binding.path_template, "/model/projects/{project_id}/sync");
        let placement = |name: &str| {
            binding
                .args
                .iter()
                .find(|a| a.name == name)
                .map(|a| a.placement)
        };
        assert!(matches!(placement("project_id"), Some(ArgPlacement::Path)));
        assert!(matches!(placement("branch_id"), Some(ArgPlacement::Query)));
        assert!(matches!(placement("graph"), Some(ArgPlacement::Body)));
    }

    #[test]
    fn planner_tools_bind_to_the_catalog_and_ui_routes() {
        let b = McpTool::ListDiagramTypes.http_binding();
        assert!(matches!(b.method, HttpMethod::Get));
        assert_eq!(b.path_template, "/api/catalog");
        let b = McpTool::SuggestDiagramType.http_binding();
        assert!(matches!(b.method, HttpMethod::Post));
        assert!(b
            .args
            .iter()
            .any(|a| a.name == "requirements" && matches!(a.placement, ArgPlacement::Body)));
        let b = McpTool::NavigateUi.http_binding();
        assert_eq!(b.path_template, "/ui/{session_id}/navigate");
        assert!(b
            .args
            .iter()
            .any(|a| a.name == "session_id" && matches!(a.placement, ArgPlacement::Path)));
        // every `{placeholder}` in a path template must have a Path arg bound to it
        for t in McpTool::ALL {
            let b = t.http_binding();
            for seg in b.path_template.split('/').filter(|s| s.starts_with('{')) {
                let name = seg.trim_matches(|c| c == '{' || c == '}');
                assert!(
                    b.args
                        .iter()
                        .any(|a| a.name == name && matches!(a.placement, ArgPlacement::Path)),
                    "{} lacks path arg {name}",
                    t.name()
                );
            }
        }
    }
}
