//! Raster diagram -> diagram-as-code (PLAN-KR0KI-007, Phase 0).
//!
//! A visual front-end: [`normalize`] turns an untrusted upload into a bounded PNG, then [`run_loop`] drives a
//! model to propose diagram-as-code, renders every attempt with the real renderer, and asks a judge whether the
//! render matches, all inside caps the orchestrator enforces. The model and the renderer are traits
//! ([`VisionModel`], [`Renderer`]) so the loop is testable with fakes; the server wires real ones in Phase 1.

mod cache;
mod hash;
mod model;
mod normalize;
pub mod prompts;
mod run;
pub mod scoring;
mod types;

pub use cache::result_cache_key;
pub use model::{
    ImagePart, ModelError, Purpose, RenderError, Rendered, Renderer, Usage, VisionModel,
    VisionRequest, VisionResponse,
};
pub use normalize::{normalize, NormalizeConfig, NormalizeError, NormalizedImage, SourceFormat};
pub use run::{
    run_loop, AcceptedVia, AttemptRecord, Confirmation, ExhaustReason, LoopConfig, LoopError,
    LoopResult, Outcome,
};
pub use types::{
    DescribedEdge, Description, DiagramKind, EdgeDiff, EdgeIssue, LabelDiff, Verdict, VerdictError,
};
