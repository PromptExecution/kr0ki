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
    ListModelProjects,
    ListModelCommits,
    GetModelSnapshot,
    QueryModelElements,
    GetModelRoots,
    QueryModelRelationships,
    QueryModelGraph,
}

impl McpTool {
    pub const ALL: &'static [McpTool] = &[
        Self::RenderDiagram,
        Self::ListFormats,
        Self::RenderKubeDiagram,
        Self::RenderK8sTopology,
        Self::RenderSysmlV2Snapshot,
        Self::ListModelProjects,
        Self::ListModelCommits,
        Self::GetModelSnapshot,
        Self::QueryModelElements,
        Self::GetModelRoots,
        Self::QueryModelRelationships,
        Self::QueryModelGraph,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::RenderDiagram => "render_diagram",
            Self::ListFormats => "list_formats",
            Self::RenderKubeDiagram => "render_kubernetes_manifest",
            Self::RenderK8sTopology => "render_kubernetes_topology",
            Self::RenderSysmlV2Snapshot => "render_sysmlv2_snapshot",
            Self::ListModelProjects => "list_model_projects",
            Self::ListModelCommits => "list_model_commits",
            Self::GetModelSnapshot => "get_model_snapshot",
            Self::QueryModelElements => "query_model_elements",
            Self::GetModelRoots => "get_model_roots",
            Self::QueryModelRelationships => "query_model_relationships",
            Self::QueryModelGraph => "query_model_graph",
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
        }
    }

    pub fn input_schema(self) -> serde_json::Value {
        match self {
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
                    "output": {"type": "string", "enum": ["svg", "png"], "default": "svg"}
                }
            }),
            Self::RenderSysmlV2Snapshot => serde_json::json!({
                "type": "object",
                "required": ["project_id", "commit_id"],
                "properties": {
                    "project_id": {"type": "string", "description": "SysML v2 project id."},
                    "commit_id": {"type": "string", "description": "Immutable SysML v2 commit id."},
                    "output": {"type": "string", "enum": ["svg", "png"], "default": "svg"}
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
        }
    }

    pub const fn http_binding(self) -> HttpBinding {
        match self {
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
                ],
            },
            Self::RenderSysmlV2Snapshot => HttpBinding {
                method: HttpMethod::Post,
                path_template: "/render/sysmlv2/projects/{project_id}/commits/{commit_id}",
                args: &[
                    ArgBinding { name: "project_id", placement: ArgPlacement::Path },
                    ArgBinding { name: "commit_id", placement: ArgPlacement::Path },
                    ArgBinding { name: "output", placement: ArgPlacement::Query },
                ],
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
#[derive(Debug, Clone, Copy)]
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
    fn all_twelve_tools_have_unique_names() {
        let mut names: Vec<&str> = McpTool::ALL.iter().map(|t| t.name()).collect();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), before, "duplicate McpTool name in ALL");
        assert_eq!(McpTool::ALL.len(), 12);
    }

    #[test]
    fn render_sysmlv2_snapshot_binds_only_model_identity_and_output() {
        let binding = McpTool::RenderSysmlV2Snapshot.http_binding();
        assert!(matches!(binding.method, HttpMethod::Post));
        assert_eq!(
            binding.path_template,
            "/render/sysmlv2/projects/{project_id}/commits/{commit_id}"
        );
        assert_eq!(binding.args.len(), 3);
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
            .all(|arg| !matches!(arg.placement, ArgPlacement::Body)));
    }

    #[test]
    fn render_k8s_topology_binds_manifest_to_body_output_to_query() {
        let binding = McpTool::RenderK8sTopology.http_binding();
        assert!(matches!(binding.method, HttpMethod::Post));
        assert_eq!(binding.path_template, "/render/k8s-topology");
        assert_eq!(binding.args.len(), 2);
        assert!(binding
            .args
            .iter()
            .any(|a| a.name == "manifest" && matches!(a.placement, ArgPlacement::Body)));
        assert!(binding
            .args
            .iter()
            .any(|a| a.name == "output" && matches!(a.placement, ArgPlacement::Query)));
        // Unlike RenderKubeDiagram (svg/dot_json, the vendored tool's own output
        // kinds), this route renders through kr0ki's own service, so its output
        // enum matches RenderDiagram's (svg/png).
        let schema = McpTool::RenderK8sTopology.input_schema();
        assert_eq!(
            schema["properties"]["output"]["enum"],
            serde_json::json!(["svg", "png"])
        );
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
}
