use kr0ki_behavior::{RuntimeContext, StateMachine, StateMachineRuntime};

#[derive(Default)]
struct OodaContext {
    observation: bool,
    effects: Vec<String>,
}

impl RuntimeContext for OodaContext {
    fn guard(&self, name: &str) -> Result<bool, String> {
        match name {
            "observation_available" => Ok(self.observation),
            "goal_met" => Ok(true),
            "goal_pending" => Ok(false),
            _ => Err(format!("unknown guard {name}")),
        }
    }

    fn effect(&mut self, name: &str) -> Result<(), String> {
        match name {
            "read_observation" => self.observation = true,
            "update_context" | "choose_action" | "apply_action" => (),
            _ => return Err(format!("unknown effect {name}")),
        }
        self.effects.push(name.into());
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let machine: StateMachine = serde_json::from_str(include_str!("../../ooda.json"))?;
    let mut runtime = StateMachineRuntime::new(&machine)?;
    let mut context = OodaContext::default();
    for event in ["observe", "orient", "decide", "act", "finish"] {
        runtime.step(event, &mut context)?;
    }
    println!("{}", serde_json::to_string_pretty(runtime.trace())?);
    Ok(())
}
