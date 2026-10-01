//! Per-session channel that lets the planning agent (or any MCP client) steer the playbook UI.
//!
//! The UI subscribes with a session id (`GET /ui/{session}/events`, SSE); an MCP tool call publishes a
//! [`UiCommand`] to that session (`POST /ui/{session}/navigate`). The bus carries *intent* only (filter, select,
//! open a view, highlight a shortlist), never markup or script, and every command is validated against the
//! catalog before it is published, so a caller cannot make the UI show anything the catalog does not contain.
//!
//! Sessions exist only while someone is subscribed: publishing to an unknown session delivers to nobody and
//! allocates nothing, so random ids cannot grow memory.

use crate::catalog;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tokio::sync::broadcast;

const CHANNEL_CAPACITY: usize = 64;
const MAX_SESSIONS: usize = 256;
const MAX_SUGGESTED: usize = 8;
const MAX_NOTE_CHARS: usize = 280;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiView {
    Gallery,
    Editor,
    Agent,
    Setup,
}

impl UiView {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "gallery" => Some(Self::Gallery),
            "editor" => Some(Self::Editor),
            "agent" => Some(Self::Agent),
            "setup" => Some(Self::Setup),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiCommand {
    /// Switch the top-level view.
    OpenView { view: UiView },
    /// Filter the gallery by intent; `None` resets to "All".
    FilterGallery { use_case: Option<String> },
    /// Select (focus and scroll to) one diagram type card.
    SelectType { type_id: String },
    /// Mark a shortlist of diagram types as the planner's suggestions.
    Suggest {
        type_ids: Vec<String>,
        note: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum UiCommandError {
    #[error("unknown view '{0}' (expected gallery, editor, agent or setup)")]
    UnknownView(String),
    #[error("unknown use case '{0}' (see list_diagram_types for the vocabulary)")]
    UnknownUseCase(String),
    #[error("unknown diagram type '{0}' (see list_diagram_types)")]
    UnknownType(String),
    #[error("suggest needs 1 to {MAX_SUGGESTED} diagram types")]
    BadShortlist,
    #[error("note is longer than {MAX_NOTE_CHARS} characters")]
    NoteTooLong,
    #[error("nothing to do: give at least one of view, use_case, type_id or suggest")]
    Empty,
}

impl UiCommand {
    /// Check every reference against the catalog.
    pub fn validate(&self) -> Result<(), UiCommandError> {
        match self {
            Self::OpenView { .. } => Ok(()),
            Self::FilterGallery { use_case } => match use_case {
                Some(u) if !catalog::USE_CASES.contains(&u.as_str()) => {
                    Err(UiCommandError::UnknownUseCase(u.clone()))
                }
                _ => Ok(()),
            },
            Self::SelectType { type_id } => catalog::by_id(type_id)
                .map(|_| ())
                .ok_or_else(|| UiCommandError::UnknownType(type_id.clone())),
            Self::Suggest { type_ids, note } => {
                if type_ids.is_empty() || type_ids.len() > MAX_SUGGESTED {
                    return Err(UiCommandError::BadShortlist);
                }
                if note
                    .as_deref()
                    .is_some_and(|n| n.chars().count() > MAX_NOTE_CHARS)
                {
                    return Err(UiCommandError::NoteTooLong);
                }
                type_ids
                    .iter()
                    .find(|id| catalog::by_id(id).is_none())
                    .map_or(Ok(()), |id| Err(UiCommandError::UnknownType(id.clone())))
            }
        }
    }
}

fn non_empty(s: Option<&str>) -> Option<&str> {
    s.map(str::trim).filter(|s| !s.is_empty())
}

/// Turn the flat arguments of the `navigate_ui` tool into validated commands, in a sensible order: open the
/// view, filter, mark the shortlist, then select. `use_case` of `"All"` (any case) or empty resets the filter.
pub fn commands_from_navigation(
    view: Option<&str>,
    use_case: Option<&str>,
    type_id: Option<&str>,
    suggest: Option<&str>,
    note: Option<&str>,
) -> Result<Vec<UiCommand>, UiCommandError> {
    let mut out = Vec::new();
    if let Some(v) = non_empty(view) {
        let view = UiView::parse(&v.to_lowercase())
            .ok_or_else(|| UiCommandError::UnknownView(v.to_owned()))?;
        out.push(UiCommand::OpenView { view });
    }
    if let Some(u) = use_case {
        let u = u.trim();
        let reset = u.is_empty() || u.eq_ignore_ascii_case("all");
        out.push(UiCommand::FilterGallery {
            use_case: (!reset).then(|| u.to_lowercase()),
        });
    }
    if let Some(list) = non_empty(suggest) {
        let type_ids = list
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();
        out.push(UiCommand::Suggest {
            type_ids,
            note: non_empty(note).map(str::to_owned),
        });
    }
    if let Some(t) = non_empty(type_id) {
        out.push(UiCommand::SelectType {
            type_id: t.to_owned(),
        });
    }
    if out.is_empty() {
        return Err(UiCommandError::Empty);
    }
    out.iter().try_for_each(UiCommand::validate)?;
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UiEnvelope {
    pub seq: u64,
    pub command: UiCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum UiBusError {
    #[error("session id must be 1-64 characters of letters, digits, '-' or '_'")]
    BadSessionId,
    #[error("too many active UI sessions")]
    Full,
}

pub fn valid_session_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

#[derive(Default)]
pub struct UiBus {
    sessions: Mutex<HashMap<String, broadcast::Sender<UiEnvelope>>>,
    seq: AtomicU64,
}

impl UiBus {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, broadcast::Sender<UiEnvelope>>> {
        // A poisoned lock only means another thread panicked mid-insert; the map is still structurally valid.
        self.sessions.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Join (creating if needed) a session. Dropping the receiver leaves it; empty sessions are pruned lazily.
    pub fn subscribe(&self, session: &str) -> Result<broadcast::Receiver<UiEnvelope>, UiBusError> {
        if !valid_session_id(session) {
            return Err(UiBusError::BadSessionId);
        }
        let mut map = self.lock();
        if !map.contains_key(session) && map.len() >= MAX_SESSIONS {
            map.retain(|_, tx| tx.receiver_count() > 0);
            if map.len() >= MAX_SESSIONS {
                return Err(UiBusError::Full);
            }
        }
        Ok(map
            .entry(session.to_owned())
            .or_insert_with(|| broadcast::channel(CHANNEL_CAPACITY).0)
            .subscribe())
    }

    /// Publish already-validated commands; returns how many listeners received the *last* one (0 = the UI is
    /// not connected, which the caller should report rather than pretend success).
    pub fn publish(&self, session: &str, commands: Vec<UiCommand>) -> Result<usize, UiBusError> {
        if !valid_session_id(session) {
            return Err(UiBusError::BadSessionId);
        }
        let tx = self.lock().get(session).cloned();
        let Some(tx) = tx else { return Ok(0) };
        let mut delivered = 0;
        for command in commands {
            let seq = self.seq.fetch_add(1, Ordering::Relaxed) + 1;
            delivered = tx.send(UiEnvelope { seq, command }).unwrap_or(0);
        }
        Ok(delivered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_builds_ordered_validated_commands() {
        // ids come from the real catalog; assert on shape and order rather than hard-coding ids that may change
        let first = catalog::by_use_case("data model")
            .into_iter()
            .map(|t| t.id)
            .collect::<Vec<_>>();
        assert!(first.len() >= 2);
        let c = commands_from_navigation(
            Some("Gallery"),
            Some("data model"),
            Some(first[0]),
            Some(&format!("{},{}", first[0], first[1])),
            Some("fits"),
        )
        .unwrap();
        assert!(matches!(
            c[0],
            UiCommand::OpenView {
                view: UiView::Gallery
            }
        ));
        assert!(matches!(
            c[1],
            UiCommand::FilterGallery { use_case: Some(_) }
        ));
        assert!(matches!(c[2], UiCommand::Suggest { .. }));
        assert!(matches!(c[3], UiCommand::SelectType { .. }));
    }

    #[test]
    fn invalid_references_are_rejected_not_forwarded() {
        assert_eq!(
            commands_from_navigation(None, None, None, None, None),
            Err(UiCommandError::Empty)
        );
        assert!(matches!(
            commands_from_navigation(Some("nowhere"), None, None, None, None),
            Err(UiCommandError::UnknownView(_))
        ));
        assert!(matches!(
            commands_from_navigation(None, Some("vibes"), None, None, None),
            Err(UiCommandError::UnknownUseCase(_))
        ));
        assert!(matches!(
            commands_from_navigation(None, None, Some("<script>"), None, None),
            Err(UiCommandError::UnknownType(_))
        ));
        assert_eq!(
            commands_from_navigation(None, None, None, Some(" , "), None),
            Err(UiCommandError::BadShortlist)
        );
        let long = "x".repeat(MAX_NOTE_CHARS + 1);
        let id = catalog::TYPES[0].id;
        assert_eq!(
            commands_from_navigation(None, None, None, Some(id), Some(&long)),
            Err(UiCommandError::NoteTooLong)
        );
    }

    #[test]
    fn all_resets_the_filter() {
        for reset in ["All", "all", "  ", ""] {
            let c = commands_from_navigation(None, Some(reset), None, None, None).unwrap();
            assert_eq!(c, vec![UiCommand::FilterGallery { use_case: None }]);
        }
    }

    #[test]
    fn commands_serialize_as_tagged_snake_case_json() {
        let json = serde_json::to_value(UiCommand::OpenView {
            view: UiView::Agent,
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({"type": "open_view", "view": "agent"})
        );
        let json = serde_json::to_value(UiCommand::FilterGallery { use_case: None }).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"type": "filter_gallery", "use_case": null})
        );
    }

    #[tokio::test]
    async fn publish_reaches_subscribers_in_order_and_reports_zero_when_nobody_listens() {
        let bus = UiBus::new();
        assert_eq!(
            bus.publish(
                "s1",
                vec![UiCommand::OpenView {
                    view: UiView::Gallery
                }]
            ),
            Ok(0)
        );
        let mut rx = bus.subscribe("s1").unwrap();
        let n = bus
            .publish(
                "s1",
                vec![
                    UiCommand::OpenView {
                        view: UiView::Editor,
                    },
                    UiCommand::FilterGallery { use_case: None },
                ],
            )
            .unwrap();
        assert_eq!(n, 1);
        let a = rx.recv().await.unwrap();
        let b = rx.recv().await.unwrap();
        assert!(b.seq > a.seq);
        assert!(matches!(
            a.command,
            UiCommand::OpenView {
                view: UiView::Editor
            }
        ));
        assert!(matches!(b.command, UiCommand::FilterGallery { .. }));
        // other sessions are isolated
        let mut other = bus.subscribe("s2").unwrap();
        bus.publish(
            "s1",
            vec![UiCommand::OpenView {
                view: UiView::Setup,
            }],
        )
        .unwrap();
        assert!(other.try_recv().is_err());
    }

    #[test]
    fn session_ids_are_validated_and_publishing_to_strangers_allocates_nothing() {
        let bus = UiBus::new();
        for bad in ["", "a b", "../x", &"a".repeat(65)] {
            assert_eq!(bus.subscribe(bad).unwrap_err(), UiBusError::BadSessionId);
            assert_eq!(
                bus.publish(bad, vec![]).unwrap_err(),
                UiBusError::BadSessionId
            );
        }
        for i in 0..1000 {
            bus.publish(
                &format!("ghost{i}"),
                vec![UiCommand::OpenView {
                    view: UiView::Gallery,
                }],
            )
            .unwrap();
        }
        assert_eq!(bus.lock().len(), 0);
    }

    #[test]
    fn session_count_is_capped_but_abandoned_sessions_are_reclaimed() {
        let bus = UiBus::new();
        let mut held = Vec::new();
        for i in 0..MAX_SESSIONS {
            held.push(bus.subscribe(&format!("s{i}")).unwrap());
        }
        assert_eq!(bus.subscribe("one-too-many").unwrap_err(), UiBusError::Full);
        held.pop(); // one session loses its last listener
        assert!(bus.subscribe("one-too-many").is_ok());
    }
}
