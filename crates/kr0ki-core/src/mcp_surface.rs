//! MCP **resources** and **prompts** for the assurance thread — the two surfaces
//! [`crate::mcp_tool`] does not cover. MCP separates the three on purpose: *resources* supply
//! context, *tools* execute operations, *prompts* are user-selected workflows.
//!
//! Like tools, both are data the server publishes (`GET /mcp/resources`, `GET /mcp/prompts`) and
//! the stdio bridge dispatches generically, so adding one needs no bridge change.
//!
//! - A **resource template** maps a URI (`kr0ki://requirement/KR-A01`) onto an existing
//!   `GET` route. It carries no logic of its own, so the gateway authorises a resource read
//!   exactly as it authorises the equivalent HTTP request, and a resource can never reach
//!   something its route does not.
//! - A **prompt** is a text template with named arguments. It tells the model *which tools to
//!   call in what order*; it grants nothing.
//!
//! Every resource answer is revision-qualified by the route that serves it, which is how an
//! agent retrieves only the context it needs, at a stated revision.

use std::collections::BTreeMap;

use crate::mcp_tool::ArgPlacement;

#[derive(Debug, Clone, Copy)]
pub struct ResourceTemplate {
    /// RFC 6570-style: `kr0ki://requirement/{id}`.
    pub uri_template: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub mime_type: &'static str,
    /// The `GET` route that serves it.
    pub path_template: &'static str,
    /// Every `{name}` of `uri_template`, and where it goes on the wire.
    pub args: &'static [(&'static str, ArgPlacement)],
}

pub const RESOURCES: &[ResourceTemplate] = &[
    ResourceTemplate {
        uri_template: "kr0ki://requirement/{id}",
        name: "requirement",
        description: "One requirement in full: profile fields, thread, assurance state, gaps, qualified by revisions.",
        mime_type: "application/json",
        path_template: "/assurance/requirements/{id}",
        args: &[("id", ArgPlacement::Path)],
    },
    ResourceTemplate {
        uri_template: "kr0ki://trace/{id}",
        name: "requirement-trace",
        description: "A requirement's links resolved at the current model revision, dangling ones named.",
        mime_type: "application/json",
        path_template: "/assurance/requirements/{id}/trace",
        args: &[("id", ArgPlacement::Path)],
    },
    ResourceTemplate {
        uri_template: "kr0ki://evidence/{requirement}",
        name: "requirement-evidence",
        description: "Revision-bound evidence for one requirement, each record with its freshness now.",
        mime_type: "application/json",
        path_template: "/assurance/evidence",
        args: &[("requirement", ArgPlacement::Query)],
    },
    ResourceTemplate {
        uri_template: "kr0ki://view/{format}",
        name: "assurance-view",
        description: "The assurance view (format: json, table, d2).",
        mime_type: "text/plain",
        path_template: "/assurance/view",
        args: &[("format", ArgPlacement::Query)],
    },
    ResourceTemplate {
        uri_template: "kr0ki://skill/{name}",
        name: "skill",
        description: "A skill's instructions (SKILL.md), e.g. requirements-authoring.",
        mime_type: "text/markdown",
        path_template: "/assurance/skills/{name}",
        args: &[("name", ArgPlacement::Path)],
    },
    ResourceTemplate {
        uri_template: "kr0ki://model/{project_id}/{commit_id}/elements",
        name: "model-elements",
        description: "Every element of a model-server project at one commit (a revision-qualified read).",
        mime_type: "application/json",
        path_template: "/model/projects/{project_id}/commits/{commit_id}/elements",
        args: &[("project_id", ArgPlacement::Path), ("commit_id", ArgPlacement::Path)],
    },
];

#[derive(Debug, Clone, Copy)]
pub struct PromptArg {
    pub name: &'static str,
    pub description: &'static str,
    pub required: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Prompt {
    pub name: &'static str,
    pub description: &'static str,
    pub arguments: &'static [PromptArg],
    /// `{{arg}}` placeholders are replaced by [`render_prompt`].
    pub template: &'static str,
}

pub const PROMPTS: &[Prompt] = &[
    Prompt {
        name: "review_requirement",
        description: "Review one requirement against the authoring rules, separating deterministic findings from your own judgment.",
        arguments: &[PromptArg { name: "id", description: "Requirement id, e.g. KR-A01.", required: true }],
        template: "Review requirement {{id}}.\n\
1. Call get_requirement with id={{id}}.\n\
2. Read the authoring rules: the resource kr0ki://skill/requirements-authoring.\n\
3. Report two lists, kept apart.\n   (a) Deterministic findings the tool gave you: statement_lints and gaps. Quote them as returned.\n   (b) Your own judgment: ambiguity, contradiction with neighbouring requirements (use list_requirements), undefined terms, and whether the behaviour is observable. Quote the words you object to and propose a rewrite.\n\
4. State both axes: satisfaction (elements asserted to satisfy it) and verification (assurance state and whether evidence is fresh), and the revisions the answer was computed at.\n\
Do not propose or commit a change unless asked.",
    },
    Prompt {
        name: "explain_gap",
        description: "Explain why a requirement is not yet verified and the smallest step that closes each gap.",
        arguments: &[PromptArg { name: "id", description: "Requirement id, e.g. KR-A06.", required: true }],
        template: "Explain why requirement {{id}} is not verified.\n\
1. Call trace_requirement with id={{id}}, get_requirement with id={{id}}, and get_evidence with requirement={{id}}.\n\
2. Name each gap by its kind and say in one sentence what it means (for example control_not_implemented: the control records no implementation yet; satisfied_untested: the design asserts it but no case has run; stale: evidence exists only for other revisions).\n\
3. For each, give the smallest step that closes it. If evidence exists, compare its revisions with the current ones in the answer and say whether it is stale and which revision moved.\n\
4. Never suggest closing a gap by editing an assertion or an implementation path that is not true.",
    },
    Prompt {
        name: "implement_next_slice",
        description: "Pick the next unimplemented requirement and carry it through test, implementation, evidence and a regenerated view.",
        arguments: &[],
        template: "Implement the next slice of the assurance baseline.\n\
1. list_requirements with gap=control_not_implemented; choose the lowest-numbered requirement whose predecessors are verified.\n\
2. get_requirement and trace_requirement for it. State its acceptance case.\n\
3. Write the failing test for that case first (the case's `cargo test -p <package> --test <name>` target), then implement until it passes.\n\
4. Run `just check` and `just test`. Run any live check the change needs before calling it verified.\n\
5. Record the control's implementation path in the baseline (docs/assurance/kr0ki.assurance.toml) once the code exists.\n\
6. run_verification with case_id and the repository's current implementation revision, then get_requirement again and report the new state and revisions.\n\
Do not mark a requirement implemented, or verified, on anything but evidence at the current revisions.",
    },
];

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum PromptError {
    #[error("prompt `{prompt}` requires argument `{argument}`")]
    MissingArgument {
        prompt: &'static str,
        argument: &'static str,
    },
    #[error("unknown argument `{0}`")]
    UnknownArgument(String),
}

/// Fill a prompt's placeholders. A required argument must be present and non-empty; an
/// argument the prompt does not declare is refused rather than ignored.
pub fn render_prompt(
    prompt: &Prompt,
    args: &BTreeMap<String, String>,
) -> Result<String, PromptError> {
    if let Some(unknown) = args
        .keys()
        .find(|k| !prompt.arguments.iter().any(|a| a.name == k.as_str()))
    {
        return Err(PromptError::UnknownArgument(unknown.clone()));
    }
    let mut text = prompt.template.to_string();
    for a in prompt.arguments {
        match args.get(a.name).map(|v| v.trim()).filter(|v| !v.is_empty()) {
            Some(v) => text = text.replace(&format!("{{{{{}}}}}", a.name), v),
            None if a.required => {
                return Err(PromptError::MissingArgument {
                    prompt: prompt.name,
                    argument: a.name,
                });
            }
            None => {}
        }
    }
    Ok(text)
}

pub fn find_prompt(name: &str) -> Option<&'static Prompt> {
    PROMPTS.iter().find(|p| p.name == name)
}

fn placement(p: ArgPlacement) -> &'static str {
    match p {
        ArgPlacement::Path => "path",
        ArgPlacement::Query => "query",
        ArgPlacement::Body => "body",
    }
}

/// `GET /mcp/resources`.
pub fn resources_manifest_json() -> serde_json::Value {
    serde_json::Value::Array(
        RESOURCES
            .iter()
            .map(|r| {
                serde_json::json!({
                    "uriTemplate": r.uri_template,
                    "name": r.name,
                    "description": r.description,
                    "mimeType": r.mime_type,
                    "httpBinding": {
                        "method": "GET",
                        "pathTemplate": r.path_template,
                        "args": r.args.iter().map(|(n, p)| serde_json::json!({"name": n, "placement": placement(*p)})).collect::<Vec<_>>(),
                    }
                })
            })
            .collect(),
    )
}

/// `GET /mcp/prompts`.
pub fn prompts_manifest_json() -> serde_json::Value {
    serde_json::Value::Array(
        PROMPTS
            .iter()
            .map(|p| {
                serde_json::json!({
                    "name": p.name,
                    "description": p.description,
                    "arguments": p.arguments.iter().map(|a| serde_json::json!({
                        "name": a.name, "description": a.description, "required": a.required
                    })).collect::<Vec<_>>(),
                    "template": p.template,
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp_tool::McpTool;

    #[test]
    fn every_uri_template_variable_is_a_bound_argument_and_vice_versa() {
        for r in RESOURCES {
            let mut vars: Vec<&str> = r
                .uri_template
                .split('{')
                .skip(1)
                .filter_map(|s| s.split('}').next())
                .collect();
            let mut bound: Vec<&str> = r.args.iter().map(|(n, _)| *n).collect();
            vars.sort_unstable();
            bound.sort_unstable();
            assert_eq!(
                vars, bound,
                "{}: URI variables and bound args differ",
                r.name
            );
            for (name, placement) in r.args {
                if *placement == ArgPlacement::Path {
                    assert!(
                        r.path_template.contains(&format!("{{{name}}}")),
                        "{}: path arg {name}",
                        r.name
                    );
                }
                assert_ne!(
                    *placement,
                    ArgPlacement::Body,
                    "{}: a resource read has no body",
                    r.name
                );
            }
        }
    }

    #[test]
    fn resource_and_prompt_names_are_unique() {
        let mut r: Vec<_> = RESOURCES.iter().map(|r| r.name).collect();
        let mut u: Vec<_> = RESOURCES.iter().map(|r| r.uri_template).collect();
        let mut p: Vec<_> = PROMPTS.iter().map(|p| p.name).collect();
        for v in [&mut r, &mut u, &mut p] {
            let n = v.len();
            v.sort_unstable();
            v.dedup();
            assert_eq!(v.len(), n);
        }
    }

    #[test]
    fn prompts_only_name_tools_that_exist() {
        let tools: Vec<&str> = McpTool::ALL.iter().map(|t| t.name()).collect();
        for p in PROMPTS {
            // every `identifier(` or `identifier with` that looks like a tool call
            for tool in [
                "get_requirement",
                "trace_requirement",
                "get_evidence",
                "list_requirements",
                "run_verification",
                "propose_change",
                "commit_change",
                "validate_change",
            ] {
                if p.template.contains(tool) {
                    assert!(
                        tools.contains(&tool),
                        "{}: names unknown tool {tool}",
                        p.name
                    );
                }
            }
        }
        assert!(PROMPTS
            .iter()
            .any(|p| p.template.contains("get_requirement")));
    }

    #[test]
    fn prompts_render_required_arguments_and_refuse_missing_or_unknown_ones() {
        let p = find_prompt("explain_gap").unwrap();
        let args = BTreeMap::from([("id".to_string(), "KR-A06".to_string())]);
        let text = render_prompt(p, &args).unwrap();
        assert!(text.contains("trace_requirement with id=KR-A06"));
        assert!(!text.contains("{{"), "no placeholder may survive rendering");
        assert_eq!(
            render_prompt(p, &BTreeMap::new()),
            Err(PromptError::MissingArgument {
                prompt: "explain_gap",
                argument: "id"
            })
        );
        assert_eq!(
            render_prompt(p, &BTreeMap::from([("id".to_string(), " ".to_string())])),
            Err(PromptError::MissingArgument {
                prompt: "explain_gap",
                argument: "id"
            })
        );
        assert!(matches!(
            render_prompt(
                p,
                &BTreeMap::from([("id".into(), "x".into()), ("evil".into(), "y".into())])
            ),
            Err(PromptError::UnknownArgument(_))
        ));
        // A prompt with no arguments renders as is.
        assert!(render_prompt(
            find_prompt("implement_next_slice").unwrap(),
            &BTreeMap::new()
        )
        .unwrap()
        .contains("run_verification"));
    }

    #[test]
    fn a_prompt_argument_cannot_inject_another_placeholder() {
        let p = find_prompt("review_requirement").unwrap();
        let args = BTreeMap::from([("id".to_string(), "{{id}}".to_string())]);
        // The value is inserted once; it is not re-expanded.
        let text = render_prompt(p, &args).unwrap();
        assert!(text.contains("id={{id}}"));
    }

    #[test]
    fn manifests_are_json_arrays_with_bindings_and_templates() {
        let r = resources_manifest_json();
        assert_eq!(r.as_array().unwrap().len(), RESOURCES.len());
        assert_eq!(r[0]["httpBinding"]["method"], "GET");
        let p = prompts_manifest_json();
        assert_eq!(p.as_array().unwrap().len(), PROMPTS.len());
        assert!(p[0]["template"].as_str().unwrap().len() > 50);
    }
}
