//! Bounded, in-memory ReqIF / ReqIFz intake for the M2 interchange boundary.
//!
//! This module deliberately accepts bytes rather than paths or URLs.  The
//! caller owns acquisition policy (browser upload, an allow-listed local path,
//! or a separately hardened HTTPS fetch); intake owns format detection, size
//! limits, hashing, parsing, attachment inventory, and normalization.  That
//! division keeps an HTTP route from becoming an unrestricted server-side
//! file or network reader.

use std::io::{Cursor, Read};

use serde::Serialize;
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::requirements::RequirementGraph;
use ufo_types::reqif::{parse_and_lower, ReqIfAdapterConfig, ReqIfAdapterError};

/// Default cap for both the uploaded artifact and an expanded ReqIFz archive.
///
/// The limits are configurable so a deployment can set a smaller policy; the
/// same cap on the expanded archive prevents a small compressed ZIP from
/// bypassing the HTTP upload limit.
pub const DEFAULT_MAX_REQIF_IMPORT_BYTES: usize = 32 * 1024 * 1024;

/// A byte cap alone does not bound CPU spent traversing a ZIP made of many
/// empty entries, so archives also have a conservative entry-count limit.
pub const DEFAULT_MAX_REQIFZ_ENTRIES: usize = 4_096;

/// `reqrs` is intentionally pinned in Cargo.toml. Keep this evidence string
/// in sync with that dependency when upgrading the parser.
pub const REQRS_PARSER_VERSION: &str = "0.2.2";

/// The shared requirements contract is supplied by `ufo-types` v0.15.0.
/// This is evidence metadata, not a second schema definition.
pub const REQUIREMENT_GRAPH_CONTRACT_VERSION: &str = "ufo-types/0.15.0:mbse::requirements";

/// Serialized b00t-type interface carried by every ReqIF import result.
///
/// This is a metadata dimension of the artifact contract: callers can select
/// a compatible deserializer without inferring its shape from a file suffix or
/// the transport that supplied the source bytes.
pub const B00T_REQIF_INTAKE_INTERFACE: &str = "b00t.type/reqif-import/v1";

/// Acquisition-neutral policy and provenance supplied by the caller.
#[derive(Debug, Clone)]
pub struct ReqIfImportConfig {
    /// Opaque provenance for the source.  If absent, import uses the artifact
    /// digest as an immutable `urn:sha256:` locator.
    pub source_uri: Option<String>,
    /// Immutable source revision when an upstream system has one.  Uploads
    /// default to the artifact digest.
    pub revision: Option<String>,
    pub max_artifact_bytes: usize,
    pub max_expanded_bytes: usize,
    pub max_archive_entries: usize,
}

impl Default for ReqIfImportConfig {
    fn default() -> Self {
        Self {
            source_uri: None,
            revision: None,
            max_artifact_bytes: DEFAULT_MAX_REQIF_IMPORT_BYTES,
            max_expanded_bytes: DEFAULT_MAX_REQIF_IMPORT_BYTES,
            max_archive_entries: DEFAULT_MAX_REQIFZ_ENTRIES,
        }
    }
}

/// A parsed document from an imported artifact.  A plain `.reqif` produces
/// one document with no `entry_name`; `.reqifz` may contain several.
#[derive(Debug, Clone, Serialize)]
pub struct ImportedReqIfDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_name: Option<String>,
    /// SHA-256 of this document's normalized semantic graph serialized with
    /// stable struct field order and BTreeMap attribute order.
    pub normalized_graph_sha256: String,
    pub graph: RequirementGraph,
}

/// Versions that made the parser and normalized-graph validation decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReqIfValidationIdentity {
    pub parser: &'static str,
    pub parser_version: &'static str,
    pub normalized_graph_contract: &'static str,
}

/// Inventory only — kr0ki does not make attachments durable.  A Flexo or
/// other requirements store can retain bytes after applying its own policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReqIfAttachment {
    pub entry_name: String,
    pub byte_len: usize,
    pub sha256: String,
}

/// Result of one validated input artifact.  Its digest identifies exactly the
/// bytes that produced every returned baseline and attachment inventory row.
#[derive(Debug, Clone, Serialize)]
pub struct ReqIfImportResult {
    /// `b00t.type/reqif-import/v1`: stable serialized interface identity for
    /// this result, distinct from the ReqIF source encoding and graph schema.
    pub b00t_type: &'static str,
    pub artifact_sha256: String,
    pub artifact_byte_len: usize,
    pub validation: ReqIfValidationIdentity,
    pub documents: Vec<ImportedReqIfDocument>,
    pub attachments: Vec<ReqIfAttachment>,
}

#[derive(Debug, thiserror::Error)]
pub enum ReqIfImportError {
    #[error("ReqIF artifact is {actual} bytes; maximum is {maximum} bytes")]
    ArtifactTooLarge { actual: usize, maximum: usize },
    #[error("ReqIFz expands beyond the {maximum}-byte policy limit")]
    ExpandedTooLarge { maximum: usize },
    #[error("ReqIFz contains {actual} entries; maximum is {maximum}")]
    TooManyArchiveEntries { actual: usize, maximum: usize },
    #[error("ReqIFz contains no .reqif or .xml document")]
    NoDocuments,
    #[error("invalid ReqIFz archive: {0}")]
    Archive(String),
    #[error(transparent)]
    Adapter(#[from] ReqIfAdapterError),
    #[error("could not serialize the normalized requirements graph: {0}")]
    CanonicalGraph(#[from] serde_json::Error),
}

/// Import a plain ReqIF XML document or a ReqIFz ZIP artifact from bytes.
///
/// ZIP detection intentionally uses the full family of ZIP signatures.  The
/// archive is never unpacked to disk, and every entry contributes to the
/// expanded-size budget, including attachments that this stateless service
/// only inventories.
pub fn import_reqif_artifact(
    bytes: &[u8],
    config: &ReqIfImportConfig,
) -> Result<ReqIfImportResult, ReqIfImportError> {
    if bytes.len() > config.max_artifact_bytes {
        return Err(ReqIfImportError::ArtifactTooLarge {
            actual: bytes.len(),
            maximum: config.max_artifact_bytes,
        });
    }

    let artifact_sha256 = sha256_hex(bytes);
    if is_zip(bytes) {
        import_reqifz(bytes, config, artifact_sha256)
    } else {
        let document = parse_document(bytes, None, config, &artifact_sha256)?;
        Ok(ReqIfImportResult {
            b00t_type: B00T_REQIF_INTAKE_INTERFACE,
            artifact_sha256,
            artifact_byte_len: bytes.len(),
            validation: validation_identity(),
            documents: vec![document],
            attachments: Vec::new(),
        })
    }
}

fn import_reqifz(
    bytes: &[u8],
    config: &ReqIfImportConfig,
    artifact_sha256: String,
) -> Result<ReqIfImportResult, ReqIfImportError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| ReqIfImportError::Archive(error.to_string()))?;
    if archive.len() > config.max_archive_entries {
        return Err(ReqIfImportError::TooManyArchiveEntries {
            actual: archive.len(),
            maximum: config.max_archive_entries,
        });
    }
    let mut expanded_bytes = 0usize;
    let mut documents = Vec::new();
    let mut attachments = Vec::new();

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| ReqIfImportError::Archive(error.to_string()))?;
        if entry.is_dir() {
            continue;
        }

        let entry_name = entry.name().to_string();
        let declared_size =
            usize::try_from(entry.size()).map_err(|_| ReqIfImportError::ExpandedTooLarge {
                maximum: config.max_expanded_bytes,
            })?;
        reserve_expanded(
            &mut expanded_bytes,
            declared_size,
            config.max_expanded_bytes,
        )?;

        let mut entry_bytes = Vec::with_capacity(declared_size);
        entry
            .read_to_end(&mut entry_bytes)
            .map_err(|error| ReqIfImportError::Archive(error.to_string()))?;
        // Do not trust ZIP metadata alone: account for actual bytes too.
        if entry_bytes.len() > declared_size {
            reserve_expanded(
                &mut expanded_bytes,
                entry_bytes.len() - declared_size,
                config.max_expanded_bytes,
            )?;
        }

        if is_reqif_document(&entry_name) {
            let document = parse_document(
                &entry_bytes,
                Some(entry_name.as_str()),
                config,
                &artifact_sha256,
            )?;
            documents.push(document);
        } else {
            attachments.push(ReqIfAttachment {
                entry_name,
                byte_len: entry_bytes.len(),
                sha256: sha256_hex(&entry_bytes),
            });
        }
    }

    if documents.is_empty() {
        return Err(ReqIfImportError::NoDocuments);
    }

    Ok(ReqIfImportResult {
        b00t_type: B00T_REQIF_INTAKE_INTERFACE,
        artifact_sha256,
        artifact_byte_len: bytes.len(),
        validation: validation_identity(),
        documents,
        attachments,
    })
}

fn reserve_expanded(
    used: &mut usize,
    additional: usize,
    maximum: usize,
) -> Result<(), ReqIfImportError> {
    *used = used
        .checked_add(additional)
        .filter(|total| *total <= maximum)
        .ok_or(ReqIfImportError::ExpandedTooLarge { maximum })?;
    Ok(())
}

fn parse_document(
    bytes: &[u8],
    entry_name: Option<&str>,
    config: &ReqIfImportConfig,
    artifact_sha256: &str,
) -> Result<ImportedReqIfDocument, ReqIfImportError> {
    let source_uri = config
        .source_uri
        .clone()
        .unwrap_or_else(|| format!("urn:sha256:{artifact_sha256}"));
    let source_uri = entry_name
        .map(|name| format!("{source_uri}#{name}"))
        .unwrap_or(source_uri);
    let revision = config
        .revision
        .clone()
        .unwrap_or_else(|| format!("sha256:{artifact_sha256}"));
    let graph = parse_and_lower(
        bytes,
        &ReqIfAdapterConfig {
            source_uri,
            revision,
            import_artifact_sha256: Some(artifact_sha256.to_string()),
        },
    )?;
    let normalized_graph_sha256 = sha256_hex(&serde_json::to_vec(&graph)?);
    Ok(ImportedReqIfDocument {
        entry_name: entry_name.map(str::to_owned),
        normalized_graph_sha256,
        graph,
    })
}

fn validation_identity() -> ReqIfValidationIdentity {
    ReqIfValidationIdentity {
        parser: "reqrs",
        parser_version: REQRS_PARSER_VERSION,
        normalized_graph_contract: REQUIREMENT_GRAPH_CONTRACT_VERSION,
    }
}

fn is_zip(bytes: &[u8]) -> bool {
    matches!(bytes, [b'P', b'K', 3 | 5 | 7, 4 | 6 | 8, ..])
}

fn is_reqif_document(name: &str) -> bool {
    name.rsplit_once('.')
        .map(|(_, extension)| matches!(extension.to_ascii_lowercase().as_str(), "reqif" | "xml"))
        .unwrap_or(false)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::write::{SimpleFileOptions, ZipWriter};

    use super::*;

    const MINIMAL_REQIF: &[u8] = include_bytes!("../tests/fixtures/reqif/minimal.reqif");
    const NO_NAMESPACE_REQIF: &[u8] =
        include_bytes!("../tests/fixtures/reqif/minimal-no-namespace.reqif");

    #[test]
    fn imports_namespace_less_reqif_with_digest_provenance() {
        let imported = import_reqif_artifact(NO_NAMESPACE_REQIF, &ReqIfImportConfig::default())
            .expect("reqrs supports namespace-less ReqIF");
        let repeated = import_reqif_artifact(NO_NAMESPACE_REQIF, &ReqIfImportConfig::default())
            .expect("the same artifact remains reproducible");

        assert_eq!(imported.documents.len(), 1);
        assert_eq!(imported.documents[0].graph.baseline.id, "BL-NO-NAMESPACE");
        assert_eq!(
            imported.documents[0]
                .graph
                .baseline
                .import_artifact_sha256
                .as_deref(),
            Some(imported.artifact_sha256.as_str())
        );
        assert_eq!(
            imported.documents[0].graph.requirements.len(),
            0,
            "minimal interchange fixture is a valid empty baseline"
        );
        assert_eq!(
            imported.documents[0].normalized_graph_sha256,
            repeated.documents[0].normalized_graph_sha256,
            "canonical graph bytes have a stable digest"
        );
        assert_eq!(imported.validation.parser, "reqrs");
        assert_eq!(imported.b00t_type, B00T_REQIF_INTAKE_INTERFACE);
        assert_eq!(imported.validation.parser_version, REQRS_PARSER_VERSION);
        assert_eq!(
            imported.validation.normalized_graph_contract,
            REQUIREMENT_GRAPH_CONTRACT_VERSION
        );
    }

    #[test]
    fn rejects_malformed_xml_with_a_typed_parse_error() {
        let error = import_reqif_artifact(b"<REQ-IF><CORE-CONTENT>", &ReqIfImportConfig::default())
            .unwrap_err();
        assert!(matches!(error, ReqIfImportError::Adapter(_)));
    }

    #[test]
    fn rejects_artifacts_over_the_configured_upload_limit() {
        let config = ReqIfImportConfig {
            max_artifact_bytes: MINIMAL_REQIF.len() - 1,
            ..ReqIfImportConfig::default()
        };
        let error = import_reqif_artifact(MINIMAL_REQIF, &config).unwrap_err();
        assert!(matches!(error, ReqIfImportError::ArtifactTooLarge { .. }));
    }

    #[test]
    fn imports_reqifz_and_inventories_attachments() {
        let mut bytes = Vec::new();
        {
            let mut archive = ZipWriter::new(Cursor::new(&mut bytes));
            let options = SimpleFileOptions::default();
            archive.start_file("baseline.reqif", options).unwrap();
            archive.write_all(MINIMAL_REQIF).unwrap();
            archive.start_file("evidence.txt", options).unwrap();
            archive.write_all(b"review evidence").unwrap();
            archive.finish().unwrap();
        }

        let imported = import_reqif_artifact(&bytes, &ReqIfImportConfig::default()).unwrap();
        assert_eq!(imported.documents.len(), 1);
        assert_eq!(
            imported.documents[0].entry_name.as_deref(),
            Some("baseline.reqif")
        );
        assert_eq!(imported.attachments.len(), 1);
        assert_eq!(imported.attachments[0].entry_name, "evidence.txt");
        assert_eq!(imported.attachments[0].byte_len, b"review evidence".len());
    }

    #[test]
    fn rejects_reqifz_that_exceeds_the_expanded_limit() {
        let mut bytes = Vec::new();
        {
            let mut archive = ZipWriter::new(Cursor::new(&mut bytes));
            let options = SimpleFileOptions::default();
            archive.start_file("baseline.reqif", options).unwrap();
            archive.write_all(MINIMAL_REQIF).unwrap();
            archive.start_file("large-attachment.bin", options).unwrap();
            archive.write_all(&[0; 128]).unwrap();
            archive.finish().unwrap();
        }
        let config = ReqIfImportConfig {
            max_expanded_bytes: MINIMAL_REQIF.len(),
            ..ReqIfImportConfig::default()
        };

        let error = import_reqif_artifact(&bytes, &config).unwrap_err();
        assert!(matches!(error, ReqIfImportError::ExpandedTooLarge { .. }));
    }

    #[test]
    fn rejects_reqifz_with_too_many_entries_before_reading_them() {
        let mut bytes = Vec::new();
        {
            let mut archive = ZipWriter::new(Cursor::new(&mut bytes));
            let options = SimpleFileOptions::default();
            archive.start_file("one.txt", options).unwrap();
            archive.write_all(b"1").unwrap();
            archive.start_file("two.txt", options).unwrap();
            archive.write_all(b"2").unwrap();
            archive.finish().unwrap();
        }
        let config = ReqIfImportConfig {
            max_archive_entries: 1,
            ..ReqIfImportConfig::default()
        };

        let error = import_reqif_artifact(&bytes, &config).unwrap_err();
        assert!(matches!(
            error,
            ReqIfImportError::TooManyArchiveEntries { .. }
        ));
    }
}
