//! Deterministic fakes for the model and the renderer, plus small builders.
#![allow(dead_code)]

use kr0ki_raster_ingest::*;
use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::Duration,
};

type Reply = Result<VisionResponse, ModelError>;

/// A model that answers from a per-purpose script and records every request it receives.
pub struct ScriptedModel {
    id: String,
    script: Mutex<HashMap<Purpose, VecDeque<Reply>>>,
    fallback: Mutex<HashMap<Purpose, Reply>>,
    pub seen: Mutex<Vec<VisionRequest>>,
    delay: Duration,
    tokens: u64,
}

impl ScriptedModel {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.into(),
            script: Default::default(),
            fallback: Default::default(),
            seen: Default::default(),
            delay: Duration::ZERO,
            tokens: 10,
        }
    }
    pub fn on(self, purpose: Purpose, replies: Vec<Reply>) -> Self {
        self.script.lock().unwrap().insert(purpose, replies.into());
        self
    }
    /// Used once the scripted replies for `purpose` run out.
    pub fn otherwise(self, purpose: Purpose, reply: Reply) -> Self {
        self.fallback.lock().unwrap().insert(purpose, reply);
        self
    }
    pub fn delay(mut self, d: Duration) -> Self {
        self.delay = d;
        self
    }
    pub fn tokens_per_call(mut self, t: u64) -> Self {
        self.tokens = t;
        self
    }
    pub fn calls(&self) -> usize {
        self.seen.lock().unwrap().len()
    }
    pub fn calls_for(&self, p: Purpose) -> usize {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.purpose == p)
            .count()
    }
    pub fn prompts_for(&self, p: Purpose) -> Vec<String> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.purpose == p)
            .map(|r| r.prompt.clone())
            .collect()
    }
}

impl VisionModel for ScriptedModel {
    fn id(&self) -> &str {
        &self.id
    }
    async fn complete(&self, req: VisionRequest) -> Result<VisionResponse, ModelError> {
        let purpose = req.purpose;
        self.seen.lock().unwrap().push(req);
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        let scripted = self
            .script
            .lock()
            .unwrap()
            .get_mut(&purpose)
            .and_then(|q| q.pop_front());
        let reply = scripted
            .or_else(|| self.fallback.lock().unwrap().get(&purpose).cloned())
            .unwrap_or_else(|| Err(ModelError::Rejected(format!("unscripted {purpose:?} call"))));
        reply.map(|mut r| {
            r.usage = Usage {
                prompt_tokens: self.tokens,
                completion_tokens: 0,
            };
            r
        })
    }
}

pub struct FakeRenderer {
    script: Mutex<VecDeque<Result<Rendered, RenderError>>>,
    fallback: Result<Rendered, RenderError>,
    pub sources: Mutex<Vec<String>>,
}

impl FakeRenderer {
    pub fn new(fallback: Result<Rendered, RenderError>) -> Self {
        Self {
            script: Default::default(),
            fallback,
            sources: Default::default(),
        }
    }
    pub fn then(self, r: Result<Rendered, RenderError>) -> Self {
        self.script.lock().unwrap().push_back(r);
        self
    }
    pub fn calls(&self) -> usize {
        self.sources.lock().unwrap().len()
    }
}

impl Renderer for FakeRenderer {
    async fn render(&self, _format: &str, source: &str) -> Result<Rendered, RenderError> {
        self.sources.lock().unwrap().push(source.to_string());
        self.script
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| self.fallback.clone())
    }
}

pub fn ok(text: &str) -> Reply {
    Ok(VisionResponse {
        text: text.into(),
        usage: Usage::default(),
    })
}
pub fn rendered(labels: Option<&[&str]>) -> Result<Rendered, RenderError> {
    Ok(Rendered {
        png: vec![0x89, b'P', b'N', b'G'],
        labels: labels.map(|l| l.iter().map(|s| s.to_string()).collect()),
    })
}
pub fn describe(labels: &[&str]) -> Reply {
    let l: Vec<String> = labels.iter().map(|s| format!("\"{s}\"")).collect();
    ok(&format!(
        "{{\"diagram_kind\":\"flowchart\",\"labels\":[{}],\"nodes\":[{}]}}",
        l.join(","),
        l.join(",")
    ))
}
pub fn verdict(matches: bool, score: f64) -> Reply {
    ok(&format!("{{\"match\":{matches},\"score\":{score}}}"))
}
pub fn verdict_with_missing(score: f64, missing: &str) -> Reply {
    ok(&format!(
        "{{\"match\":false,\"score\":{score},\"missing_nodes\":[\"{missing}\"]}}"
    ))
}
pub fn source(s: &str) -> Reply {
    ok(&format!("```d2\n{s}\n```"))
}

/// A real, valid normalized image (an 8x8 PNG) so the loop is exercised with the production type.
pub fn image() -> NormalizedImage {
    let mut png = Vec::new();
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(8, 8, image::Rgb([200, 30, 30])))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    normalize(&png, &NormalizeConfig::default()).unwrap()
}
