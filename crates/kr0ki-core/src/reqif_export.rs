//! `RequirementGraph` -> ReqIF XML export. The missing direction:
//! `ufo_types::reqif` only lowers ReqIF -> `RequirementGraph`; this module
//! builds a `reqrs::model::ReqIfBundle` from a `RequirementGraph` and
//! serializes it back to XML. See
//! docs/superpowers/specs/2026-09-23-flexo-baseline-adapter-design.md.

use crate::requirements::RequirementGraph;

#[derive(Debug, thiserror::Error)]
pub enum ReqIfExportError {
    #[error(transparent)]
    Unparse(#[from] reqrs::ReqIfError),
}

pub fn export_bundle(_graph: &RequirementGraph) -> Result<reqrs::model::ReqIfBundle, ReqIfExportError> {
    todo!("Task 5")
}

pub fn export_bundle_to_xml(graph: &RequirementGraph) -> Result<String, ReqIfExportError> {
    let bundle = export_bundle(graph)?;
    Ok(reqrs::ReqIfUnparser::unparse(&bundle, reqrs::unparse::FormatMode::Canonical)?)
}
