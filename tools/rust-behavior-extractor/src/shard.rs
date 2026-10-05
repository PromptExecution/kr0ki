use kr0ki_behavior::{Node, NodeKind, RustBehaviorIr};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Compiler-only metadata. The published IR and its schema remain unchanged.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CompilerShard {
    pub ir: RustBehaviorIr,
    pub confirmed_definitions: BTreeSet<String>,
}

pub(crate) fn merge_node(
    nodes: &mut BTreeMap<String, (Node, bool)>,
    node: Node,
    confirmed: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some((old, old_confirmed)) = nodes.get_mut(&node.id) else {
        nodes.insert(node.id.clone(), (node, confirmed));
        return Ok(());
    };
    if old == &node {
        *old_confirmed |= confirmed;
        return Ok(());
    }
    let compatible = old.name == node.name
        && (old.kind == node.kind
            || matches!(
                (old.kind, node.kind),
                (NodeKind::External, NodeKind::Function) | (NodeKind::Function, NodeKind::External)
            ));
    if !compatible || (*old_confirmed && confirmed) {
        return Err(format!("conflicting node shard {}", node.id).into());
    }
    if confirmed {
        *old = node;
        *old_confirmed = true;
    } else if !*old_confirmed {
        // Only trait references and external function placeholders may vary by
        // evidence span. Every use remains independently anchored on its edge.
        if old.kind == NodeKind::Trait && node.kind == NodeKind::Trait
            || old.kind == NodeKind::External && node.kind == NodeKind::External
        {
            if node.anchor < old.anchor {
                *old = node;
            }
        } else {
            return Err(format!("conflicting node shard {}", node.id).into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kr0ki_behavior::{Anchor, SourceAnnotation};

    fn trait_node(file: &str) -> Node {
        Node {
            id: "trait-id".into(),
            name: "shared::Trait".into(),
            kind: NodeKind::Trait,
            anchor: Some(Anchor {
                file: file.into(),
                symbol: "shared::Trait".into(),
                start: 0,
                end: 10,
            }),
            annotations: vec![],
        }
    }

    #[test]
    fn references_and_declaration_merge_in_every_order() {
        let first = trait_node("a.rs");
        let second = trait_node("b.rs");
        let mut declaration = trait_node("z.rs");
        declaration.annotations.push(SourceAnnotation {
            path: "doc".into(),
            text: "#[doc = \"declared\"]".into(),
            anchor: declaration.anchor.clone().unwrap(),
        });
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let candidates = [(&first, false), (&second, false), (&declaration, true)];
            let mut nodes = BTreeMap::new();
            for i in order {
                let (node, confirmed) = candidates[i];
                merge_node(&mut nodes, node.clone(), confirmed).unwrap();
            }
            assert_eq!(nodes["trait-id"], (declaration.clone(), true));
        }
        for order in [[&first, &second], [&second, &first]] {
            let mut nodes = BTreeMap::new();
            for node in order {
                merge_node(&mut nodes, node.clone(), false).unwrap();
            }
            assert_eq!(nodes["trait-id"], (first.clone(), false));
        }
    }

    #[test]
    fn incompatible_references_and_conflicting_declarations_fail_closed() {
        let first = trait_node("a.rs");
        let second = trait_node("b.rs");
        let mut nodes = BTreeMap::new();
        merge_node(&mut nodes, first.clone(), true).unwrap();
        assert!(merge_node(&mut nodes, second.clone(), true).is_err());
        assert_eq!(nodes["trait-id"], (first.clone(), true));
        for other in [
            Node {
                name: "other::Trait".into(),
                ..second.clone()
            },
            Node {
                kind: NodeKind::Type,
                ..second
            },
        ] {
            assert!(merge_node(&mut nodes, other, false).is_err());
        }
    }
}
