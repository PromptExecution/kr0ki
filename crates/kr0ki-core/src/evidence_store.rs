//! Revision-bound evidence storage.
//!
//! One JSON file per [`EvidenceRecord`] plus the artifact it points at, both content-addressed:
//!
//! ```text
//! <root>/records/<sha256(record json)>.json
//! <root>/artifacts/<sha256(bytes)>.log
//! ```
//!
//! - **Append-only.** A record's file name is the digest of its content, so storing the same
//!   record twice is a no-op, and two *different* results for the same case at the same
//!   revisions (a flaky case) both survive. Nothing is overwritten and there is no delete.
//! - **Verifiable.** [`FsEvidenceStore::verify`] re-hashes every artifact against the digest in
//!   its record, so tampering or loss is reported, not assumed away.
//! - **Deterministic reads.** [`FsEvidenceStore::list`] is sorted, so a view regenerated from
//!   the same store is byte-identical.
//!
//! The store neither decides freshness nor interprets results; that is
//! `ufo_types::mbse::assurance`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ufo_types::mbse::assurance::{EvidenceError, EvidenceRecord};

use crate::assurance_baseline::sha256_hex;

#[derive(Debug, thiserror::Error)]
pub enum EvidenceStoreError {
    #[error("evidence store I/O at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid evidence record: {0}")]
    Invalid(#[from] EvidenceError),
    #[error("malformed evidence record file {path}: {message}")]
    Malformed { path: PathBuf, message: String },
}

fn io_err(path: &Path) -> impl FnOnce(io::Error) -> EvidenceStoreError + '_ {
    move |source| EvidenceStoreError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// What [`FsEvidenceStore::verify`] found for one record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactCheck {
    Intact,
    /// The artifact file is missing or unreadable.
    Missing,
    /// The artifact exists but its digest differs from the record's.
    DigestMismatch {
        actual: String,
    },
    /// The artifact is not in this store (e.g. a remote URI): nothing to check here.
    External,
}

#[derive(Debug, Clone)]
pub struct FsEvidenceStore {
    root: PathBuf,
}

impl FsEvidenceStore {
    /// A store rooted at `root`. Directories are created on first write.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn records_dir(&self) -> PathBuf {
        self.root.join("records")
    }

    fn artifacts_dir(&self) -> PathBuf {
        self.root.join("artifacts")
    }

    /// Store `bytes` as an artifact; returns `(uri, digest)` for the record.
    /// Idempotent: the same bytes are the same artifact.
    pub fn put_artifact(&self, bytes: &[u8]) -> Result<(String, String), EvidenceStoreError> {
        let digest = sha256_hex(bytes);
        let dir = self.artifacts_dir();
        fs::create_dir_all(&dir).map_err(io_err(&dir))?;
        let path = dir.join(format!("{digest}.log"));
        if !path.exists() {
            write_atomic(&path, bytes)?;
        }
        Ok((
            format!("file://{}", path.display()),
            format!("sha256:{digest}"),
        ))
    }

    /// Store one record. Returns its file stem (the digest of its content). Idempotent.
    pub fn put(&self, record: &EvidenceRecord) -> Result<String, EvidenceStoreError> {
        record.validate()?;
        let json = serde_json::to_vec_pretty(record).expect("an EvidenceRecord serializes");
        let stem = sha256_hex(&json);
        let dir = self.records_dir();
        fs::create_dir_all(&dir).map_err(io_err(&dir))?;
        let path = dir.join(format!("{stem}.json"));
        if !path.exists() {
            write_atomic(&path, &json)?;
        }
        Ok(stem)
    }

    /// Every record, sorted by `(key, result, artifact_digest)`.
    pub fn list(&self) -> Result<Vec<EvidenceRecord>, EvidenceStoreError> {
        let dir = self.records_dir();
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(io_err(&dir)(e)),
        };
        let mut out = Vec::new();
        for entry in entries {
            let path = entry.map_err(io_err(&dir))?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let bytes = fs::read(&path).map_err(io_err(&path))?;
            let record: EvidenceRecord =
                serde_json::from_slice(&bytes).map_err(|e| EvidenceStoreError::Malformed {
                    path: path.clone(),
                    message: e.to_string(),
                })?;
            record
                .validate()
                .map_err(|e| EvidenceStoreError::Malformed {
                    path: path.clone(),
                    message: e.to_string(),
                })?;
            out.push(record);
        }
        out.sort_by(|a, b| {
            (a.key(), format!("{:?}", a.result), &a.artifact_digest).cmp(&(
                b.key(),
                format!("{:?}", b.result),
                &b.artifact_digest,
            ))
        });
        Ok(out)
    }

    pub fn for_requirement(
        &self,
        requirement_id: &str,
    ) -> Result<Vec<EvidenceRecord>, EvidenceStoreError> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|r| r.requirement_id == requirement_id)
            .collect())
    }

    pub fn for_case(
        &self,
        verification_id: &str,
    ) -> Result<Vec<EvidenceRecord>, EvidenceStoreError> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|r| r.verification_id == verification_id)
            .collect())
    }

    /// Re-hash each record's artifact and compare with the recorded digest.
    pub fn verify(&self) -> Result<Vec<(EvidenceRecord, ArtifactCheck)>, EvidenceStoreError> {
        let own_prefix = format!("file://{}", self.artifacts_dir().display());
        Ok(self
            .list()?
            .into_iter()
            .map(|r| {
                let check = match r.artifact_uri.strip_prefix("file://") {
                    Some(path) if r.artifact_uri.starts_with(&own_prefix) => match fs::read(path) {
                        Ok(bytes) => {
                            let actual = format!("sha256:{}", sha256_hex(&bytes));
                            if actual == r.artifact_digest {
                                ArtifactCheck::Intact
                            } else {
                                ArtifactCheck::DigestMismatch { actual }
                            }
                        }
                        Err(_) => ArtifactCheck::Missing,
                    },
                    _ => ArtifactCheck::External,
                };
                (r, check)
            })
            .collect())
    }
}

/// Write via a temporary sibling and rename, so a reader never sees a half-written file.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), EvidenceStoreError> {
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&tmp, bytes).map_err(io_err(&tmp))?;
    fs::rename(&tmp, path).map_err(io_err(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ufo_types::mbse::assurance::VerificationResult;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "kr0ki-evidence-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&d);
        d
    }

    fn record(
        store: &FsEvidenceStore,
        case: &str,
        impl_rev: &str,
        result: VerificationResult,
        log: &str,
    ) -> EvidenceRecord {
        let (uri, digest) = store.put_artifact(log.as_bytes()).unwrap();
        EvidenceRecord {
            requirement_id: "KR-X".into(),
            verification_id: case.into(),
            model_revision: "m1".into(),
            implementation_revision: impl_rev.into(),
            configuration_digest: format!("sha256:{}", "1".repeat(64)),
            result,
            artifact_uri: uri,
            artifact_digest: digest,
        }
    }

    #[test]
    fn put_then_list_round_trips_and_is_idempotent() {
        let store = FsEvidenceStore::new(dir("rt"));
        let r = record(&store, "VC-1", "i1", VerificationResult::Pass, "ok");
        let a = store.put(&r).unwrap();
        let b = store.put(&r).unwrap();
        assert_eq!(a, b);
        assert_eq!(store.list().unwrap(), vec![r]);
    }

    #[test]
    fn an_empty_or_missing_store_lists_nothing() {
        assert_eq!(FsEvidenceStore::new(dir("none")).list().unwrap(), vec![]);
    }

    #[test]
    fn two_results_for_the_same_revisions_both_survive() {
        let store = FsEvidenceStore::new(dir("flaky"));
        let pass = record(&store, "VC-1", "i1", VerificationResult::Pass, "ok");
        let fail = record(&store, "VC-1", "i1", VerificationResult::Fail, "boom");
        store.put(&pass).unwrap();
        store.put(&fail).unwrap();
        assert_eq!(
            store.list().unwrap().len(),
            2,
            "a flaky case must not hide its failure"
        );
    }

    #[test]
    fn invalid_records_are_refused_not_stored() {
        let store = FsEvidenceStore::new(dir("bad"));
        let mut r = record(&store, "VC-1", "i1", VerificationResult::Pass, "ok");
        r.artifact_digest = "md5:nope".into();
        assert!(matches!(store.put(&r), Err(EvidenceStoreError::Invalid(_))));
        assert_eq!(store.list().unwrap(), vec![]);
    }

    #[test]
    fn listing_is_sorted_and_filterable() {
        let store = FsEvidenceStore::new(dir("sort"));
        for (case, rev) in [("VC-2", "i1"), ("VC-1", "i2"), ("VC-1", "i1")] {
            store
                .put(&record(&store, case, rev, VerificationResult::Pass, case))
                .unwrap();
        }
        let keys: Vec<_> = store.list().unwrap().iter().map(|r| r.key()).collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
        assert_eq!(store.for_case("VC-1").unwrap().len(), 2);
        assert_eq!(store.for_requirement("KR-X").unwrap().len(), 3);
        assert_eq!(store.for_requirement("KR-NOPE").unwrap().len(), 0);
    }

    #[test]
    fn verify_detects_tampering_and_loss() {
        let store = FsEvidenceStore::new(dir("verify"));
        let r = record(
            &store,
            "VC-1",
            "i1",
            VerificationResult::Pass,
            "the real log",
        );
        store.put(&r).unwrap();
        let checks = store.verify().unwrap();
        assert_eq!(checks[0].1, ArtifactCheck::Intact);

        let path = r.artifact_uri.strip_prefix("file://").unwrap().to_string();
        fs::write(&path, b"a forged log").unwrap();
        assert!(matches!(
            store.verify().unwrap()[0].1,
            ArtifactCheck::DigestMismatch { .. }
        ));

        fs::remove_file(&path).unwrap();
        assert_eq!(store.verify().unwrap()[0].1, ArtifactCheck::Missing);
    }

    #[test]
    fn an_artifact_outside_this_store_is_external_not_intact() {
        let store = FsEvidenceStore::new(dir("ext"));
        let mut r = record(&store, "VC-1", "i1", VerificationResult::Pass, "ok");
        r.artifact_uri = "https://ci.example.test/run/1".into();
        store.put(&r).unwrap();
        assert_eq!(store.verify().unwrap()[0].1, ArtifactCheck::External);
    }

    #[test]
    fn a_malformed_record_file_is_an_error_not_silently_skipped() {
        let store = FsEvidenceStore::new(dir("malformed"));
        let d = store.records_dir();
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("junk.json"), b"{not json").unwrap();
        assert!(matches!(
            store.list(),
            Err(EvidenceStoreError::Malformed { .. })
        ));
    }
}
