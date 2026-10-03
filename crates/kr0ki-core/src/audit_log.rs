//! KR-A08: the audit log — one record per tool invocation, permitted **or denied**, with the
//! caller, the operation, the decision, the model revision and a correlation identifier.
//!
//! The log is what a reviewer reads to learn what an agent did and what it was refused, so it
//! must not depend on the agent's own account:
//!
//! - **Append-only JSONL**, one record per line, fsynced before the call returns.
//! - **Hash-chained.** Each record carries `prev_hash`, the SHA-256 of the previous line, so an
//!   edited, deleted or reordered record breaks the chain at a known sequence number
//!   ([`AuditLog::verify`]). That is tamper-*evident*, not tamper-proof: someone who can
//!   rewrite the whole file can rewrite the chain, so the file belongs on storage the workload
//!   cannot write (KR-A07).
//! - **Written before the operation runs** (phase [`Phase::Decision`]); the status of what
//!   happened is a second record with the same correlation id ([`Phase::Outcome`]). A crash in
//!   between still leaves the decision on record.
//! - **No secrets.** Records hold the path (never the query string), the caller's *identity
//!   id* (never a token), and the revision. Request and response bodies are not recorded.
//!
//! Timestamps are operational wall-clock (`at`): this is an audit trail of when things
//! happened, unlike the modelled-time fields in `ufo-types`, which never read a clock.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::assurance_baseline::sha256_hex;

pub const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Written before the operation runs: who asked for what, and the decision.
    Decision,
    /// Written after: what status the operation produced.
    Outcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Permit,
    Deny,
}

/// What the caller of [`AuditLog::append`] supplies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditDraft {
    pub correlation_id: String,
    pub phase: Phase,
    /// Identity id, or `unauthenticated`.
    pub caller: String,
    /// The operation's name (e.g. `commit_change`).
    pub operation: String,
    pub method: String,
    /// Path only — never the query string.
    pub path: String,
    pub decision: Decision,
    pub reason: Option<String>,
    pub model_revision: Option<String>,
    /// What the caller *claims* its transport is (`http`, `mcp`). Informational: nothing is
    /// authorised on it.
    pub transport: String,
    pub status: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
    pub seq: u64,
    /// RFC 3339, UTC.
    pub at: String,
    pub correlation_id: String,
    pub phase: Phase,
    pub caller: String,
    pub operation: String,
    pub method: String,
    pub path: String,
    pub decision: Decision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_revision: Option<String>,
    pub transport: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    /// SHA-256 of the previous record's line (`GENESIS_HASH` for the first).
    pub prev_hash: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("audit log I/O at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("audit log {path} line {line}: {message}")]
    Malformed {
        path: PathBuf,
        line: usize,
        message: String,
    },
    #[error("the audit log is unavailable: a previous write failed")]
    Poisoned,
}

/// Where the chain stops being valid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainBreak {
    pub line: usize,
    pub seq: Option<u64>,
    pub reason: String,
}

struct Tail {
    next_seq: u64,
    last_hash: String,
    healthy: bool,
}

pub struct AuditLog {
    path: PathBuf,
    tail: Mutex<Tail>,
}

/// Filter for [`AuditLog::query`]; every set field must match.
#[derive(Debug, Clone, Default)]
pub struct AuditQuery {
    pub caller: Option<String>,
    pub operation: Option<String>,
    pub decision: Option<Decision>,
    pub correlation_id: Option<String>,
    pub phase: Option<Phase>,
}

impl AuditLog {
    /// Open (creating the parent directory) and resume the chain from the existing file's tail.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, AuditError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| AuditError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let (next_seq, last_hash) = match read_lines(&path)? {
            lines if lines.is_empty() => (0, GENESIS_HASH.to_string()),
            lines => {
                let last = lines.last().expect("non-empty");
                let record: AuditRecord =
                    serde_json::from_str(last).map_err(|e| AuditError::Malformed {
                        path: path.clone(),
                        line: lines.len(),
                        message: e.to_string(),
                    })?;
                (record.seq + 1, sha256_hex(last.as_bytes()))
            }
        };
        Ok(Self {
            path,
            tail: Mutex::new(Tail {
                next_seq,
                last_hash,
                healthy: true,
            }),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one record and fsync it. After a failed write the log refuses further appends
    /// (the caller fails closed) rather than risk a gap that looks like tampering.
    pub fn append(&self, draft: AuditDraft, at: &str) -> Result<AuditRecord, AuditError> {
        let mut tail = self.tail.lock().expect("audit tail");
        if !tail.healthy {
            return Err(AuditError::Poisoned);
        }
        let record = AuditRecord {
            seq: tail.next_seq,
            at: at.to_string(),
            correlation_id: draft.correlation_id,
            phase: draft.phase,
            caller: draft.caller,
            operation: draft.operation,
            method: draft.method,
            path: draft.path,
            decision: draft.decision,
            reason: draft.reason,
            model_revision: draft.model_revision,
            transport: draft.transport,
            status: draft.status,
            prev_hash: tail.last_hash.clone(),
        };
        let line = serde_json::to_string(&record).expect("an AuditRecord serializes");
        let write = || -> io::Result<()> {
            let mut f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?;
            f.write_all(line.as_bytes())?;
            f.write_all(b"\n")?;
            f.sync_data()
        };
        if let Err(source) = write() {
            tail.healthy = false;
            return Err(AuditError::Io {
                path: self.path.clone(),
                source,
            });
        }
        tail.next_seq += 1;
        tail.last_hash = sha256_hex(line.as_bytes());
        Ok(record)
    }

    /// Every record, in order.
    pub fn read_all(&self) -> Result<Vec<AuditRecord>, AuditError> {
        read_lines(&self.path)?
            .iter()
            .enumerate()
            .map(|(i, l)| {
                serde_json::from_str(l).map_err(|e| AuditError::Malformed {
                    path: self.path.clone(),
                    line: i + 1,
                    message: e.to_string(),
                })
            })
            .collect()
    }

    pub fn query(&self, q: &AuditQuery) -> Result<Vec<AuditRecord>, AuditError> {
        Ok(self
            .read_all()?
            .into_iter()
            .filter(|r| q.caller.as_ref().is_none_or(|c| &r.caller == c))
            .filter(|r| q.operation.as_ref().is_none_or(|o| &r.operation == o))
            .filter(|r| q.decision.is_none_or(|d| r.decision == d))
            .filter(|r| {
                q.correlation_id
                    .as_ref()
                    .is_none_or(|c| &r.correlation_id == c)
            })
            .filter(|r| q.phase.is_none_or(|p| r.phase == p))
            .collect())
    }

    /// Re-walk the chain. `Ok(n)` is the number of records verified.
    pub fn verify(&self) -> Result<Result<usize, ChainBreak>, AuditError> {
        let lines = read_lines(&self.path)?;
        let mut expected_prev = GENESIS_HASH.to_string();
        for (i, line) in lines.iter().enumerate() {
            let record: AuditRecord = match serde_json::from_str(line) {
                Ok(r) => r,
                Err(e) => {
                    return Ok(Err(ChainBreak {
                        line: i + 1,
                        seq: None,
                        reason: format!("not a record: {e}"),
                    }));
                }
            };
            if record.seq != i as u64 {
                return Ok(Err(ChainBreak {
                    line: i + 1,
                    seq: Some(record.seq),
                    reason: format!(
                        "sequence {} where {} was expected (a record was removed or reordered)",
                        record.seq, i
                    ),
                }));
            }
            if record.prev_hash != expected_prev {
                return Ok(Err(ChainBreak {
                    line: i + 1,
                    seq: Some(record.seq),
                    reason: "prev_hash does not match the previous record (a record was edited or removed)".into(),
                }));
            }
            expected_prev = sha256_hex(line.as_bytes());
        }
        Ok(Ok(lines.len()))
    }
}

fn read_lines(path: &Path) -> Result<Vec<String>, AuditError> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(AuditError::Io {
                path: path.to_path_buf(),
                source,
            })
        }
    };
    BufReader::new(file)
        .lines()
        .filter(|l| l.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(true))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| AuditError::Io {
            path: path.to_path_buf(),
            source,
        })
}

/// `now` as RFC 3339 UTC (`2026-10-02T14:03:07Z`).
pub fn now_rfc3339() -> String {
    rfc3339(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    )
}

/// Seconds since the Unix epoch as RFC 3339 UTC.
pub fn rfc3339(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(tag: &str) -> PathBuf {
        let p = std::env::temp_dir()
            .join(format!("kr0ki-audit-{tag}-{}", std::process::id()))
            .join("audit.jsonl");
        let _ = fs::remove_dir_all(p.parent().unwrap());
        p
    }

    fn draft(corr: &str, caller: &str, op: &str, decision: Decision, phase: Phase) -> AuditDraft {
        AuditDraft {
            correlation_id: corr.into(),
            phase,
            caller: caller.into(),
            operation: op.into(),
            method: "POST".into(),
            path: "/x".into(),
            decision,
            reason: (decision == Decision::Deny).then(|| "no grant".to_string()),
            model_revision: Some("rev-1".into()),
            transport: "http".into(),
            status: None,
        }
    }

    #[test]
    fn rfc3339_matches_known_instants() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z"); // a leap day
        assert_eq!(rfc3339(1_790_908_026), "2026-10-02T02:27:06Z"); // matches the pilot's created stamp
        assert_eq!(rfc3339(4_102_444_799), "2099-12-31T23:59:59Z");
        assert!(now_rfc3339().ends_with('Z') && now_rfc3339().len() == 20);
    }

    #[test]
    fn records_are_appended_in_order_and_chained() {
        let log = AuditLog::open(path("chain")).unwrap();
        let a = log
            .append(
                draft(
                    "c1",
                    "reader",
                    "get_requirement",
                    Decision::Permit,
                    Phase::Decision,
                ),
                "t1",
            )
            .unwrap();
        let b = log
            .append(
                draft(
                    "c2",
                    "reader",
                    "commit_change",
                    Decision::Deny,
                    Phase::Decision,
                ),
                "t2",
            )
            .unwrap();
        assert_eq!((a.seq, b.seq), (0, 1));
        assert_eq!(a.prev_hash, GENESIS_HASH);
        assert_ne!(b.prev_hash, GENESIS_HASH);
        assert_eq!(log.verify().unwrap(), Ok(2));
        assert_eq!(log.read_all().unwrap(), vec![a, b]);
    }

    #[test]
    fn the_chain_survives_a_restart() {
        let p = path("restart");
        {
            let log = AuditLog::open(&p).unwrap();
            log.append(
                draft("c1", "u", "op", Decision::Permit, Phase::Decision),
                "t1",
            )
            .unwrap();
        }
        let log = AuditLog::open(&p).unwrap();
        let r = log
            .append(
                draft("c2", "u", "op", Decision::Permit, Phase::Decision),
                "t2",
            )
            .unwrap();
        assert_eq!(r.seq, 1);
        assert_eq!(log.verify().unwrap(), Ok(2));
    }

    #[test]
    fn editing_removing_or_reordering_a_record_breaks_the_chain_at_a_known_place() {
        let p = path("tamper");
        let log = AuditLog::open(&p).unwrap();
        for i in 0..4 {
            log.append(
                draft(
                    &format!("c{i}"),
                    "u",
                    "op",
                    Decision::Permit,
                    Phase::Decision,
                ),
                "t",
            )
            .unwrap();
        }
        let original = fs::read_to_string(&p).unwrap();
        let lines: Vec<&str> = original.lines().collect();

        // edit record 1's caller
        let edited = original.replacen("\"caller\":\"u\"", "\"caller\":\"admin\"", 2);
        fs::write(&p, &edited).unwrap();
        let broken = AuditLog::open(&p).unwrap().verify().unwrap().unwrap_err();
        assert!(broken.line >= 2, "{broken:?}");

        // delete a middle record
        fs::write(&p, format!("{}\n{}\n{}\n", lines[0], lines[2], lines[3])).unwrap();
        assert_eq!(
            AuditLog::open(&p)
                .unwrap()
                .verify()
                .unwrap()
                .unwrap_err()
                .line,
            2
        );

        // reorder
        fs::write(
            &p,
            format!("{}\n{}\n{}\n{}\n", lines[0], lines[2], lines[1], lines[3]),
        )
        .unwrap();
        assert!(AuditLog::open(&p).unwrap().verify().unwrap().is_err());

        // restored: valid again
        fs::write(&p, &original).unwrap();
        assert_eq!(AuditLog::open(&p).unwrap().verify().unwrap(), Ok(4));
    }

    #[test]
    fn query_filters_by_caller_decision_and_correlation() {
        let log = AuditLog::open(path("query")).unwrap();
        log.append(
            draft(
                "c1",
                "reader",
                "get_requirement",
                Decision::Permit,
                Phase::Decision,
            ),
            "t",
        )
        .unwrap();
        log.append(
            draft(
                "c2",
                "reader",
                "commit_change",
                Decision::Deny,
                Phase::Decision,
            ),
            "t",
        )
        .unwrap();
        log.append(
            draft(
                "c2",
                "reader",
                "commit_change",
                Decision::Deny,
                Phase::Outcome,
            ),
            "t",
        )
        .unwrap();
        log.append(
            draft(
                "c3",
                "committer",
                "commit_change",
                Decision::Permit,
                Phase::Decision,
            ),
            "t",
        )
        .unwrap();
        let q = |f: &dyn Fn(&mut AuditQuery)| {
            let mut q = AuditQuery::default();
            f(&mut q);
            log.query(&q).unwrap().len()
        };
        assert_eq!(q(&|q| q.caller = Some("reader".into())), 3);
        assert_eq!(q(&|q| q.decision = Some(Decision::Deny)), 2);
        assert_eq!(q(&|q| q.correlation_id = Some("c2".into())), 2);
        assert_eq!(
            q(&|q| {
                q.operation = Some("commit_change".into());
                q.phase = Some(Phase::Decision);
            }),
            2
        );
        assert_eq!(q(&|_| {}), 4);
    }

    #[test]
    fn a_missing_log_is_empty_and_valid() {
        let log = AuditLog::open(path("empty")).unwrap();
        assert_eq!(log.read_all().unwrap(), vec![]);
        assert_eq!(log.verify().unwrap(), Ok(0));
    }

    #[test]
    fn a_failed_write_poisons_the_log_so_the_caller_fails_closed() {
        let p = path("poison");
        let log = AuditLog::open(&p).unwrap();
        log.append(
            draft("c1", "u", "op", Decision::Permit, Phase::Decision),
            "t",
        )
        .unwrap();
        // Make the file unwritable by replacing it with a directory.
        fs::remove_file(&p).unwrap();
        fs::create_dir(&p).unwrap();
        assert!(matches!(
            log.append(
                draft("c2", "u", "op", Decision::Permit, Phase::Decision),
                "t"
            ),
            Err(AuditError::Io { .. })
        ));
        assert!(matches!(
            log.append(
                draft("c3", "u", "op", Decision::Permit, Phase::Decision),
                "t"
            ),
            Err(AuditError::Poisoned)
        ));
    }
}
