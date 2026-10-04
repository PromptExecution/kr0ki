use crate::{digest, Diagnostic, Severity, StateMachine, Transition, ValidationError};

/// Applications bind named guard/effect identifiers to their Rust implementation.
/// The docgen/runtime never interprets these strings as Rust, JavaScript or SQL.
pub trait RuntimeContext {
    /// Guards must be pure predicates; transition ordering carries no priority.
    fn guard(&self, name: &str) -> Result<bool, String>;
    fn effect(&mut self, name: &str) -> Result<(), String>;
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TransitionTrace {
    pub transition: String,
    pub from: String,
    pub to: String,
    pub event: String,
    pub guard: Option<String>,
    pub effect: Option<String>,
    pub terminal: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error(transparent)]
    InvalidModel(#[from] ValidationError),
    #[error("terminal state {0} cannot consume another event")]
    Terminal(String),
    #[error("no enabled transition for {state} / {event}")]
    NoTransition { state: String, event: String },
    #[error("multiple enabled transitions for {state} / {event}")]
    Ambiguous { state: String, event: String },
    #[error("guard {name} failed: {message}")]
    Guard { name: String, message: String },
    #[error("effect {name} failed: {message}")]
    Effect { name: String, message: String },
}

/// A small executor over the same immutable table used by diagram exporters.
/// Effects run only after exactly one enabled transition has been selected.
pub struct StateMachineRuntime<'a> {
    machine: &'a StateMachine,
    state: String,
    trace: Vec<TransitionTrace>,
}

impl<'a> StateMachineRuntime<'a> {
    pub fn new(machine: &'a StateMachine) -> Result<Self, RuntimeError> {
        machine.ensure_valid()?;
        Ok(Self {
            machine,
            state: machine.initial.clone(),
            trace: Vec::new(),
        })
    }

    pub fn state(&self) -> &str {
        &self.state
    }

    pub fn trace(&self) -> &[TransitionTrace] {
        &self.trace
    }

    pub fn is_terminal(&self) -> bool {
        self.machine
            .states
            .iter()
            .any(|s| s.id == self.state && s.terminal)
    }

    pub fn step(
        &mut self,
        event: &str,
        context: &mut impl RuntimeContext,
    ) -> Result<TransitionTrace, RuntimeError> {
        if self.is_terminal() {
            return Err(RuntimeError::Terminal(self.state.clone()));
        }
        let mut enabled: Option<&Transition> = None;
        for transition in self
            .machine
            .transitions
            .iter()
            .filter(|t| t.from == self.state && t.event == event)
        {
            let selected = match &transition.guard {
                Some(name) => context.guard(name).map_err(|message| RuntimeError::Guard {
                    name: name.clone(),
                    message,
                })?,
                None => true,
            };
            if selected && enabled.replace(transition).is_some() {
                return Err(RuntimeError::Ambiguous {
                    state: self.state.clone(),
                    event: event.into(),
                });
            }
        }
        let transition = enabled.ok_or_else(|| RuntimeError::NoTransition {
            state: self.state.clone(),
            event: event.into(),
        })?;
        if let Some(name) = &transition.effect {
            context
                .effect(name)
                .map_err(|message| RuntimeError::Effect {
                    name: name.clone(),
                    message,
                })?;
        }
        self.state.clone_from(&transition.to);
        let trace = TransitionTrace {
            transition: transition.id.clone(),
            from: transition.from.clone(),
            to: transition.to.clone(),
            event: event.into(),
            guard: transition.guard.clone(),
            effect: transition.effect.clone(),
            terminal: self.is_terminal(),
        };
        self.trace.push(trace.clone());
        Ok(trace)
    }
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
        .replace('\n', "&#10;")
        .replace('\r', "&#13;")
        .replace('\t', "&#9;")
}

fn state_id(value: &str) -> String {
    format!("s_{}", digest(value))
}

impl StateMachine {
    /// Export deterministic SCXML 1.0 structure, with source ids/provenance in
    /// the `kr0ki` namespace. Guards are application bindings; effects use an
    /// extension element and are never emitted as executable script source.
    pub fn to_scxml(&self) -> Result<String, ValidationError> {
        self.ensure_valid()?;
        let mut labels: Vec<&str> = vec![&self.id, &self.name, &self.initial];
        labels.extend(self.states.iter().map(|s| s.id.as_str()));
        for transition in &self.transitions {
            labels.extend([transition.id.as_str(), transition.event.as_str()]);
            labels.extend(transition.guard.as_deref());
            labels.extend(transition.effect.as_deref());
        }
        if labels.into_iter().any(|label| label.chars().any(|c| !matches!(c as u32, 0x9 | 0xa | 0xd | 0x20..=0xd7ff | 0xe000..=0xfffd | 0x10000..=0x10ffff))) {
            return Err(ValidationError::Invariants(vec![Diagnostic { code: "scxml_character".into(), severity: Severity::Error, message: "SCXML labels contain characters forbidden by XML 1.0".into(), anchor: None }]));
        }
        let mut out = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<scxml xmlns=\"http://www.w3.org/2005/07/scxml\" xmlns:kr0ki=\"urn:kr0ki:behavior:v1\" version=\"1.0\" initial=\"{}\" name=\"{}\" kr0ki:id=\"{}\">\n", state_id(&self.initial), escape(&self.name), escape(&self.id));
        let mut states: Vec<_> = self.states.iter().collect();
        states.sort_by(|a, b| a.id.cmp(&b.id));
        for state in states {
            let tag = if state.terminal { "final" } else { "state" };
            out.push_str(&format!(
                "  <{tag} id=\"{}\" kr0ki:id=\"{}\"",
                state_id(&state.id),
                escape(&state.id)
            ));
            if let Some(anchor) = &state.anchor {
                out.push_str(&format!(
                    " kr0ki:anchor=\"{}\"",
                    escape(&serde_json::to_string(anchor)?)
                ));
            }
            out.push_str(">\n");
            let mut transitions: Vec<_> = self
                .transitions
                .iter()
                .filter(|t| t.from == state.id)
                .collect();
            transitions.sort_by(|a, b| a.id.cmp(&b.id));
            for transition in transitions {
                out.push_str(&format!(
                    "    <transition event=\"{}\" target=\"{}\" kr0ki:id=\"{}\"",
                    escape(&transition.event),
                    state_id(&transition.to),
                    escape(&transition.id)
                ));
                if let Some(guard) = &transition.guard {
                    out.push_str(&format!(" cond=\"{}\"", escape(guard)));
                }
                if let Some(anchor) = &transition.anchor {
                    out.push_str(&format!(
                        " kr0ki:anchor=\"{}\"",
                        escape(&serde_json::to_string(anchor)?)
                    ));
                }
                match &transition.effect {
                    None => out.push_str("/>\n"),
                    Some(effect) => out.push_str(&format!(
                        ">\n      <kr0ki:effect name=\"{}\"/>\n    </transition>\n",
                        escape(effect)
                    )),
                }
            }
            out.push_str(&format!("  </{tag}>\n"));
        }
        out.push_str("</scxml>\n");
        Ok(out)
    }
}
