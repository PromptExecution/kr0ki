//! Client for the **SysMD sidecar** (`containers/kr0ki-sysmd`: tukcps/SysMD, an interval constraint solver, run headless on :8081).
//!
//! SysMD is a black box: kr0ki sends SysML v2 / KerML / SysMD text and reads back the solved variables; it never links SysMD
//! code. One call to [`SysmdClient::solve`] is one throwaway project + session, deleted afterwards.
//!
//! # Reading SysMD's numbers soundly
//! SysMD answers with *strings* formatted for people (`Representer`, precision 5), not with intervals. Verified live against
//! SysMD 4.3.0 on 2026-10-05, and in `Representer.kt`, the formatting loses information in ways that matter for a solver:
//!
//! * every bound is rounded to 5 decimals of its mantissa (`0.33333`, `12.34568e6`, `500e-6`);
//! * a value whose bounds are within a relative 1e-5 of each other, or that are both below 5e-6, prints **only the lower
//!   bound** (`0..1e-7` prints `0`);
//! * when the upper bound exceeds the lower by more than 1e6 the lower bound prints as a literal `0`;
//! * `*` means unbounded, `∅` empty, `NaN`, and non-numbers (`true`, `Unknown`, dates) are free text.
//!
//! [`parse_value`] therefore never returns a printed number as exact: it *widens outward* by those bounds, so the interval
//! it returns contains the solver's true range. [`Interval`] cannot hold an infinite bound, so unbounded results are a
//! separate variant instead of a poisoned interval.
//!
//! Values are always in SI base units (`5 mm` comes back as `0.005` with unit `m`; `20 °C` as `293.15` with unit `K`), and the
//! unit string is `kg m / s^2`-shaped: factors by whitespace, one `/`, integer `^` powers. Units kr0ki cannot map are reported
//! as such, never guessed.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::{OnceCell, Semaphore};
use ufo_types::quantity::{Dimension, Interval, Quantity, Unit};

/// Model text larger than this is refused before it reaches the sidecar.
pub const MAX_SOURCE_BYTES: usize = 256 * 1024;
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
/// Solves run at most this many at once: each session holds the whole standard library in memory.
const CONCURRENT_SOLVES: usize = 2;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SysmdError {
    #[error("model text is larger than {MAX_SOURCE_BYTES} bytes")]
    TooLarge,
    #[error("the SysMD sidecar is unavailable: {0}")]
    Unavailable(String),
    #[error("the SysMD sidecar refused the request ({status}): {body}")]
    Rejected { status: u16, body: String },
    #[error("unexpected response from the SysMD sidecar: {0}")]
    Protocol(String),
}

/// The notation of the text sent to SysMD (its `Language::toString()`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SysmdLanguage {
    #[default]
    Sysml,
    Kerml,
    Sysmd,
}

impl SysmdLanguage {
    fn wire(self) -> &'static str {
        match self {
            Self::Sysml => "SysML",
            Self::Kerml => "KerML",
            Self::Sysmd => "SysMD",
        }
    }
}

/// What SysMD said a variable's range is, with every printed number already widened outward (see the module docs).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ValueRange {
    /// A closed, finite interval that contains the solver's range.
    Bounded {
        range: Interval,
    },
    /// At least one bound is infinite (`*`); a finite bound is already widened.
    Unbounded {
        lo: Option<f64>,
        hi: Option<f64>,
    },
    /// `∅`: no value (an unbound variable, or an unsatisfiable one: the issues say which).
    Empty,
    NotANumber,
    /// Not a number at all (`true`, `Unknown`, a date, a string).
    Other {
        text: String,
    },
}

/// One unit of imprecision, relative to the numeral's own exponent: SysMD prints 5 decimals of the mantissa.
const MANTISSA_ULP: f64 = 1e-5;
/// Below this magnitude two bounds both print as 0 and SysMD keeps only the lower one.
const ZERO_BAND: f64 = 5e-6;

struct Numeral {
    value: f64,
    /// The size of one unit in the last printed place (a decade of the mantissa, times 1e-5).
    ulp: f64,
}

fn parse_numeral(text: &str) -> Option<Numeral> {
    // no trimming: SysMD prints numeric ranges as `a..b`; a spaced `a .. b` is its date format
    let t = text;
    if t.is_empty()
        || !t
            .bytes()
            .all(|b| b.is_ascii_digit() || b"+-.eE".contains(&b))
    {
        return None; // also keeps `inf` / `nan`, which Rust's f64 parser would accept, out
    }
    let value: f64 = t.parse().ok()?;
    if !value.is_finite() {
        return None;
    }
    let exponent: i32 = match t.find(['e', 'E']) {
        Some(i) => t[i + 1..].parse().ok()?,
        None => 0,
    };
    Some(Numeral {
        value,
        ulp: MANTISSA_ULP * 10f64.powi(exponent),
    })
}

/// Parse one SysMD value string into a range that contains the solver's true range.
pub fn parse_value(raw: &str) -> ValueRange {
    let t = raw.trim();
    let other = || ValueRange::Other { text: t.to_owned() };
    match t {
        "∅" => return ValueRange::Empty,
        "NaN" => return ValueRange::NotANumber,
        _ => {}
    }
    let Some((lo_text, hi_text)) = t.split_once("..") else {
        // One numeral: SysMD may have dropped the upper bound (relative 1e-5 apart, or both inside the zero band).
        let Some(n) = parse_numeral(t) else {
            return other();
        };
        let slack = n.ulp
            + MANTISSA_ULP * n.value.abs()
            + if n.value.abs() < 2.0 * ZERO_BAND {
                ZERO_BAND
            } else {
                0.0
            };
        return bounded(n.value - slack, n.value + slack).unwrap_or_else(other);
    };
    let side = |s: &str| -> Result<Option<Numeral>, ()> {
        if s == "*" {
            Ok(None)
        } else {
            parse_numeral(s).map(Some).ok_or(())
        }
    };
    let (Ok(lo), Ok(hi)) = (side(lo_text), side(hi_text)) else {
        return other();
    };
    match (lo, hi) {
        (None, None) => ValueRange::Unbounded { lo: None, hi: None },
        (Some(lo), None) => ValueRange::Unbounded {
            lo: Some(lo.value - lo.ulp),
            hi: None,
        },
        (None, Some(hi)) => ValueRange::Unbounded {
            lo: None,
            hi: Some(hi.value + hi.ulp),
        },
        (Some(lo), Some(hi)) => {
            let mut lo_v = lo.value - lo.ulp;
            // A literal `0` lower bound stands for anything in (-hi/1e6, hi/1e6): the printer replaces a tiny minimum by 0.
            if lo.value == 0.0 {
                lo_v = lo_v.min(-hi.value.abs() * 1e-6);
            }
            bounded(lo_v, hi.value + hi.ulp).unwrap_or_else(other)
        }
    }
}

fn bounded(lo: f64, hi: f64) -> Option<ValueRange> {
    Interval::new(lo, hi)
        .ok()
        .map(|range| ValueRange::Bounded { range })
}

/// Map SysMD's unit string (`m`, `kg m / s^2`, `1 / s`, `EUR`) to a [`Unit`], or say why not.
pub fn parse_unit(raw: &str) -> Result<Unit, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Err("empty unit".into());
    }
    if !t.contains([' ', '/', '^']) {
        return Unit::lookup(t).ok_or_else(|| format!("unknown unit '{t}'"));
    }
    let mut parts = t.split('/');
    let numerator = parts.next().unwrap_or_default();
    let denominator = parts.next().unwrap_or_default();
    if parts.next().is_some() {
        return Err(format!("more than one '/' in unit '{t}'"));
    }
    let mut dimension = Dimension::DIMENSIONLESS;
    let mut scale = 1.0_f64;
    for (factors, divide) in [(numerator, false), (denominator, true)] {
        for token in factors.split_whitespace() {
            if token == "1" {
                continue;
            }
            let (symbol, power) = match token.split_once('^') {
                Some((s, p)) => (
                    s,
                    p.parse::<i32>()
                        .map_err(|_| format!("bad power in unit '{t}'"))?,
                ),
                None => (token, 1),
            };
            if !(1..=8).contains(&power) {
                return Err(format!("unsupported power in unit '{t}'"));
            }
            let atom =
                Unit::lookup(symbol).ok_or_else(|| format!("unknown unit '{symbol}' in '{t}'"))?;
            if atom.has_offset() || atom.currency.is_some() {
                return Err(format!("'{symbol}' cannot be combined in '{t}'"));
            }
            for _ in 0..power {
                dimension = if divide {
                    dimension.div(&atom.dimension)
                } else {
                    dimension.mul(&atom.dimension)
                }
                .map_err(|e| format!("{e} in unit '{t}'"))?;
                scale = if divide {
                    scale / atom.scale
                } else {
                    scale * atom.scale
                };
            }
        }
    }
    Unit::new(t, dimension, scale, 0.0, None).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SolvedVariable {
    pub path: String,
    /// SysMD's own unit string.
    pub unit: String,
    /// SysMD's own value string, kept so a reader can see what was parsed.
    pub raw: String,
    pub range: ValueRange,
    /// `Some` when the range is bounded and the unit maps to a ufo-types [`Unit`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantity: Option<Quantity>,
    /// Why `quantity` is `None` although the range is bounded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_problem: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolveIssue {
    pub kind: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "elementId")]
    pub element_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SolveVerdict {
    /// No issue of kind `ERROR*` or `WARN_INCONSISTENCY`.
    Consistent,
    /// SysMD reported an inconsistency (a constraint or dependency that cannot hold): the values are not a valid solution.
    Inconsistent,
    /// The model did not parse or analyse; there are no trustworthy values.
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SolveReport {
    pub verdict: SolveVerdict,
    pub issues: Vec<SolveIssue>,
    pub iterations: u32,
    pub variables: Vec<SolvedVariable>,
}

pub fn verdict_of(issues: &[SolveIssue]) -> SolveVerdict {
    if issues.iter().any(|i| i.kind.starts_with("ERROR")) {
        SolveVerdict::Error
    } else if issues.iter().any(|i| i.kind == "WARN_INCONSISTENCY") {
        SolveVerdict::Inconsistent
    } else {
        SolveVerdict::Consistent
    }
}

fn solved(path: String, unit: String, raw: String) -> SolvedVariable {
    let range = parse_value(&raw);
    let (quantity, unit_problem) = match &range {
        ValueRange::Bounded { range } => match parse_unit(&unit) {
            Ok(u) => (Some(Quantity::new(*range, u)), None),
            Err(e) => (None, Some(e)),
        },
        _ => (None, None),
    };
    SolvedVariable {
        path,
        unit,
        raw,
        range,
        quantity,
        unit_problem,
    }
}

/// `a/2/1`-style names are solver bookkeeping, not model variables.
fn is_bookkeeping(path: &str) -> bool {
    let mut parts = path.rsplitn(3, '/');
    let (last, mid, rest) = (parts.next(), parts.next(), parts.next());
    matches!((last, mid, rest), (Some(l), Some(m), Some(_)) if l.bytes().all(|b| b.is_ascii_digit()) && m.bytes().all(|b| b.is_ascii_digit()) && !l.is_empty() && !m.is_empty())
}

struct RawRun {
    issues: Vec<SolveIssue>,
    iterations: u32,
    variables: Vec<(String, String, String)>,
}

pub struct SysmdClient {
    base: String,
    http: reqwest::Client,
    slots: Semaphore,
    /// Variables of an empty model: the standard library, which every session carries and no caller asked about.
    library: OnceCell<HashSet<String>>,
    counter: AtomicU64,
}

impl SysmdClient {
    /// `base` is the sidecar's HTTP root, e.g. `http://127.0.0.1:8081`.
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            base: base.into().trim_end_matches('/').to_owned(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .expect("reqwest client builds"),
            slots: Semaphore::new(CONCURRENT_SOLVES),
            library: OnceCell::new(),
            counter: AtomicU64::new(1),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    /// `true` when the sidecar answers `GET /projects` (SysMD has no health route of its own).
    pub async fn ping(&self) -> Result<(), SysmdError> {
        let resp = self
            .http
            .get(format!("{}/projects", self.base))
            .send()
            .await
            .map_err(|e| SysmdError::Unavailable(e.to_string()))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(SysmdError::Unavailable(format!(
                "GET /projects returned HTTP {}",
                resp.status()
            )))
        }
    }

    /// Solve `code` and report every variable it defines, with ranges parsed soundly and units mapped.
    pub async fn solve(
        &self,
        code: &str,
        language: SysmdLanguage,
    ) -> Result<SolveReport, SysmdError> {
        if code.len() > MAX_SOURCE_BYTES {
            return Err(SysmdError::TooLarge);
        }
        let library = self
            .library
            .get_or_try_init(|| async {
                let run = self.run("", SysmdLanguage::Sysml).await?;
                Ok::<_, SysmdError>(run.variables.into_iter().map(|(p, _, _)| p).collect())
            })
            .await?;
        let run = self.run(code, language).await?;
        let variables = run
            .variables
            .into_iter()
            .filter(|(path, _, _)| !library.contains(path) && !is_bookkeeping(path))
            .map(|(path, unit, raw)| solved(path, unit, raw))
            .collect();
        Ok(SolveReport {
            verdict: verdict_of(&run.issues),
            issues: run.issues,
            iterations: run.iterations,
            variables,
        })
    }

    async fn run(&self, code: &str, language: SysmdLanguage) -> Result<RawRun, SysmdError> {
        let _slot = self
            .slots
            .acquire()
            .await
            .map_err(|_| SysmdError::Unavailable("shutting down".into()))?;
        let name = format!(
            "kr0ki-{}-{}",
            std::process::id(),
            self.counter.fetch_add(1, Ordering::Relaxed)
        );
        let project = self
            .json(
                self.http
                    .post(format!("{}/projects", self.base))
                    .json(&json!({"name": name, "description": "kr0ki solve"})),
            )
            .await?;
        let project_id = project
            .get("@id")
            .and_then(Value::as_str)
            .ok_or_else(|| SysmdError::Protocol("project response has no @id".into()))?
            .to_owned();
        let result = self.run_in_project(&name, code, language).await;
        // Best effort: a failed delete only leaks memory until the sidecar restarts, and must not hide the real result.
        let _ = self
            .http
            .delete(format!("{}/projects/{}", self.base, project_id))
            .send()
            .await;
        result
    }

    async fn run_in_project(
        &self,
        project: &str,
        code: &str,
        language: SysmdLanguage,
    ) -> Result<RawRun, SysmdError> {
        let resp = self
            .http
            .post(format!("{}/session", self.base))
            .header("content-type", "text/plain")
            .body(project.to_owned())
            .send()
            .await
            .map_err(|e| SysmdError::Unavailable(e.to_string()))?;
        let session = String::from_utf8(self.read(resp).await?)
            .map_err(|_| SysmdError::Protocol("session id is not text".into()))?
            .trim()
            .to_owned();
        let result = self.run_in_session(&session, code, language).await;
        let _ = self
            .http
            .delete(format!("{}/session", self.base))
            .header("SessionId", &session)
            .send()
            .await;
        result
    }

    async fn run_in_session(
        &self,
        session: &str,
        code: &str,
        language: SysmdLanguage,
    ) -> Result<RawRun, SysmdError> {
        let body = json!({"language": language.wire(), "body": code, "runlevel": "ALL"});
        let resp = self
            .http
            .put(format!("{}/session/model", self.base))
            .header("SessionId", session)
            .json(&body)
            .send()
            .await
            .map_err(|e| SysmdError::Unavailable(e.to_string()))?;
        // A 500 still carries the issues (SysMD's way of saying "your model did not analyse"), so it is a report, not a failure.
        let status = resp.status();
        let bytes = self.read_any(resp).await?;
        let status_json: Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(_) => {
                return Err(SysmdError::Rejected {
                    status: status.as_u16(),
                    body: String::from_utf8_lossy(&bytes).chars().take(300).collect(),
                })
            }
        };
        if !status.is_success() && status_json.get("issues").is_none() {
            return Err(SysmdError::Rejected {
                status: status.as_u16(),
                body: status_json.to_string().chars().take(300).collect(),
            });
        }
        let issues: Vec<SolveIssue> =
            serde_json::from_value(status_json.get("issues").cloned().unwrap_or(json!([])))
                .map_err(|e| SysmdError::Protocol(format!("issues: {e}")))?;
        let iterations = status_json
            .get("numberOfPropagateIterations")
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32;
        let resp = self
            .http
            .get(format!("{}/session/variables", self.base))
            .header("SessionId", session)
            .send()
            .await
            .map_err(|e| SysmdError::Unavailable(e.to_string()))?;
        // 404 with an empty list is how SysMD says "no variables".
        let vars: Value = if resp.status() == reqwest::StatusCode::NOT_FOUND {
            json!({"variables": []})
        } else {
            serde_json::from_slice(&self.read(resp).await?)
                .map_err(|e| SysmdError::Protocol(format!("variables: {e}")))?
        };
        let variables = vars
            .get("variables")
            .and_then(Value::as_array)
            .ok_or_else(|| SysmdError::Protocol("no variables array".into()))?
            .iter()
            .filter_map(|v| {
                Some((
                    v.get("qualifiedName")?.as_str()?.to_owned(),
                    v.get("unit")
                        .and_then(Value::as_str)
                        .unwrap_or("1")
                        .to_owned(),
                    v.get("value")?.as_str()?.to_owned(),
                ))
            })
            .collect();
        Ok(RawRun {
            issues,
            iterations,
            variables,
        })
    }

    async fn json(&self, req: reqwest::RequestBuilder) -> Result<Value, SysmdError> {
        let resp = req
            .send()
            .await
            .map_err(|e| SysmdError::Unavailable(e.to_string()))?;
        serde_json::from_slice(&self.read(resp).await?)
            .map_err(|e| SysmdError::Protocol(format!("not JSON: {e}")))
    }

    /// Body of a successful response.
    async fn read(&self, resp: reqwest::Response) -> Result<Vec<u8>, SysmdError> {
        let status = resp.status();
        let bytes = self.read_any(resp).await?;
        if status.is_success() {
            Ok(bytes)
        } else {
            Err(SysmdError::Rejected {
                status: status.as_u16(),
                body: String::from_utf8_lossy(&bytes).chars().take(300).collect(),
            })
        }
    }

    async fn read_any(&self, resp: reqwest::Response) -> Result<Vec<u8>, SysmdError> {
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| SysmdError::Unavailable(e.to_string()))?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err(SysmdError::Protocol("response is too large".into()));
        }
        Ok(bytes.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn bounded_of(v: &ValueRange) -> (f64, f64) {
        match v {
            ValueRange::Bounded { range } => (range.lo(), range.hi()),
            other => panic!("expected a bounded range, got {other:?}"),
        }
    }

    /// A printed number is never exact: the returned interval must contain it and the true range behind it.
    #[test]
    fn a_single_numeral_is_widened_not_taken_as_exact() {
        let (lo, hi) = bounded_of(&parse_value("6"));
        assert!(lo < 6.0 && hi > 6.0, "[{lo}, {hi}]");
        assert!(hi - lo < 1e-3, "widening must stay small: [{lo}, {hi}]");
        // 1/3 printed as 0.33333 (true 0.333333...), 1/7 as 0.14286 (true 0.142857...): rounding up and down both covered
        let (lo, hi) = bounded_of(&parse_value("0.33333"));
        assert!(lo <= 1.0 / 3.0 && 1.0 / 3.0 <= hi);
        let (lo, hi) = bounded_of(&parse_value("0.14286"));
        assert!(lo <= 1.0 / 7.0 && 1.0 / 7.0 <= hi);
    }

    /// `2.000001` prints as `2`; `0..1e-7` prints as `0` (verified live): both truths must lie inside what we return.
    #[test]
    fn values_the_printer_collapsed_are_still_contained() {
        let (lo, hi) = bounded_of(&parse_value("2"));
        assert!(lo <= 2.000001 && 2.000001 <= hi, "[{lo}, {hi}]");
        let (lo, hi) = bounded_of(&parse_value("0"));
        assert!(lo <= 0.0 && hi >= 1e-7, "[{lo}, {hi}]");
        let (lo, hi) = bounded_of(&parse_value("1.234e-9"));
        assert!(lo <= 1.234e-9 && hi >= 4e-6, "the zero band: [{lo}, {hi}]");
    }

    #[test]
    fn engineering_notation_scales_the_widening_with_the_exponent() {
        // 12.34568e6 stands for 12345678.9 (printed from 12345678.9): the 5th decimal of the mantissa is worth 10
        let (lo, hi) = bounded_of(&parse_value("12.34568e6"));
        assert!(lo <= 12_345_678.9 && 12_345_678.9 <= hi, "[{lo}, {hi}]");
        let (lo, hi) = bounded_of(&parse_value("500e-6"));
        assert!(lo <= 0.0005 && 0.0005 <= hi);
        assert!(hi - lo < 1e-4);
    }

    #[test]
    fn a_range_widens_each_bound_and_a_literal_zero_lower_bound_is_not_trusted() {
        let (lo, hi) = bounded_of(&parse_value("0.1..0.3"));
        assert!(lo < 0.1 && 0.3 < hi);
        // `1..1e9` prints as `0..1e9` (verified live): the true lower bound 1 is inside; a tiny negative one cannot be excluded
        let (lo, hi) = bounded_of(&parse_value("0..1e9"));
        assert!(lo <= 0.0 && hi >= 1e9, "[{lo}, {hi}]");
        assert!(lo >= -1e4, "and not absurdly loose: {lo}");
    }

    #[test]
    fn unbounded_empty_nan_and_free_text_are_not_numbers() {
        assert_eq!(
            parse_value("*..*"),
            ValueRange::Unbounded { lo: None, hi: None }
        );
        assert!(
            matches!(parse_value("3..*"), ValueRange::Unbounded { lo: Some(l), hi: None } if l < 3.0 && l > 2.999)
        );
        assert!(
            matches!(parse_value("*..-1e3"), ValueRange::Unbounded { lo: None, hi: Some(h) } if h > -1e3)
        );
        assert_eq!(parse_value("∅"), ValueRange::Empty);
        assert_eq!(parse_value("NaN"), ValueRange::NotANumber);
        for text in [
            "Unknown",
            "true",
            "inf",
            "nan",
            "1 .. 2",
            "2026-10-05 .. 2026-10-06",
            "",
            "1e999",
            "--3",
        ] {
            assert!(
                matches!(parse_value(text), ValueRange::Other { .. }),
                "{text:?} -> {:?}",
                parse_value(text)
            );
        }
    }

    #[test]
    fn units_are_mapped_by_dimension_and_unknown_ones_are_reported() {
        let n = parse_unit("kg m / s^2").unwrap();
        assert_eq!(
            (n.dimension.mass, n.dimension.length, n.dimension.time),
            (1, 1, -2)
        );
        assert_eq!(n.scale, 1.0);
        let w = parse_unit("kg m^2 / s^3").unwrap();
        assert_eq!(
            (w.dimension.mass, w.dimension.length, w.dimension.time),
            (1, 2, -3)
        );
        let hz = parse_unit("1 / s").unwrap();
        assert_eq!((hz.dimension.length, hz.dimension.time), (0, -1));
        assert_eq!(parse_unit("m^2").unwrap().dimension.length, 2);
        assert_eq!(parse_unit("1").unwrap(), Unit::one());
        assert_eq!(
            parse_unit("EUR").unwrap().currency.map(|c| c.to_string()),
            Some("EUR".into())
        );
        assert!(parse_unit("furlong").unwrap_err().contains("furlong"));
        assert!(parse_unit("m / s / s").is_err());
        assert!(parse_unit("m^99").is_err());
        assert!(parse_unit("EUR / h").is_err(), "money does not combine");
        assert!(parse_unit("").is_err());
    }

    #[test]
    fn a_bounded_value_with_an_unknown_unit_is_kept_but_explained() {
        let v = solved("x".into(), "furlong".into(), "2".into());
        assert!(v.quantity.is_none());
        assert!(v.unit_problem.unwrap().contains("furlong"));
        assert!(matches!(v.range, ValueRange::Bounded { .. }));
        let v = solved("y".into(), "m".into(), "0.1..0.3".into());
        assert_eq!(v.quantity.unwrap().unit.dimension.length, 1);
    }

    #[test]
    fn verdict_follows_the_issue_kinds() {
        let issue = |k: &str| SolveIssue {
            kind: k.into(),
            message: String::new(),
            element_id: None,
        };
        assert_eq!(verdict_of(&[]), SolveVerdict::Consistent);
        assert_eq!(verdict_of(&[issue("WARN_UNIT")]), SolveVerdict::Consistent);
        assert_eq!(
            verdict_of(&[issue("WARN_INCONSISTENCY")]),
            SolveVerdict::Inconsistent
        );
        assert_eq!(
            verdict_of(&[issue("WARN_INCONSISTENCY"), issue("ERROR_SYNTACTICAL")]),
            SolveVerdict::Error
        );
    }

    #[test]
    fn solver_bookkeeping_names_are_recognised() {
        assert!(is_bookkeeping("a/2/1"));
        assert!(is_bookkeeping("b::inner/12/3"));
        assert!(!is_bookkeeping("a"));
        assert!(!is_bookkeeping("b::mass"));
        assert!(!is_bookkeeping("a/x/1"));
    }

    async fn sysmd(model_status: u16, model_body: Value, variables: Value) -> MockServer {
        let s = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/projects"))
            .respond_with(
                ResponseTemplate::new(201).set_body_json(json!({"@id": "P1", "name": "n"})),
            )
            .mount(&s)
            .await;
        Mock::given(method("POST"))
            .and(path("/session"))
            .and(header("content-type", "text/plain"))
            .respond_with(ResponseTemplate::new(201).set_body_string("S1"))
            .mount(&s)
            .await;
        Mock::given(method("PUT"))
            .and(path("/session/model"))
            .and(header("SessionId", "S1"))
            .respond_with(ResponseTemplate::new(model_status).set_body_json(model_body))
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path("/session/variables"))
            .respond_with(ResponseTemplate::new(200).set_body_json(variables))
            .mount(&s)
            .await;
        Mock::given(method("DELETE"))
            .and(path("/session"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&s)
            .await;
        Mock::given(method("DELETE"))
            .and(path("/projects/P1"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&s)
            .await;
        s
    }

    #[tokio::test]
    async fn solve_drops_library_variables_and_bookkeeping_and_cleans_up() {
        let vars = json!({"variables": [
            {"qualifiedName": "Base::things::multiplicity", "unit": "1", "value": "*..*"},
            {"qualifiedName": "w", "unit": "m", "value": "0.2..0.6"},
            {"qualifiedName": "w/2/1", "unit": "1", "value": "*..*"},
        ]});
        let s = sysmd(
            200,
            json!({"issues": [], "numberOfPropagateIterations": 4, "updates": {}}),
            vars,
        )
        .await;
        let c = SysmdClient::new(s.uri());
        let report = c
            .solve(
                "attribute w: ISQ::LengthValue = 0.1 .. 0.3;",
                SysmdLanguage::Sysml,
            )
            .await
            .unwrap();
        // the mock answers the baseline run with the same variables, so `w` is library there; use a second client view
        // to prove the filter: everything the baseline saw is dropped, nothing else is
        assert!(report.variables.is_empty());
        assert_eq!(report.verdict, SolveVerdict::Consistent);
        let deletes = s
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|r| r.method.as_str() == "DELETE")
            .count();
        assert_eq!(
            deletes, 4,
            "baseline run and solve each delete their session and project"
        );
    }

    #[tokio::test]
    async fn a_500_with_issues_is_a_report_not_an_error() {
        let s = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/projects"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({"@id": "P"})))
            .mount(&s)
            .await;
        Mock::given(method("POST"))
            .and(path("/session"))
            .respond_with(ResponseTemplate::new(201).set_body_string("S"))
            .mount(&s)
            .await;
        // the empty baseline model succeeds, the user's model does not
        Mock::given(method("PUT")).and(path("/session/model"))
            .respond_with(|req: &wiremock::Request| {
                let body: Value = serde_json::from_slice(&req.body).unwrap();
                if body["body"] == "" {
                    ResponseTemplate::new(200).set_body_json(json!({"issues": [], "numberOfPropagateIterations": 1, "updates": {}}))
                } else {
                    ResponseTemplate::new(500).set_body_json(json!({"issues": [{"kind": "ERROR_SYNTACTICAL", "message": "after '[': expected ']'", "line": 1}], "numberOfPropagateIterations": 0, "updates": {}}))
                }
            })
            .mount(&s).await;
        Mock::given(method("GET"))
            .and(path("/session/variables"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({"variables": []})))
            .mount(&s)
            .await;
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&s)
            .await;
        let c = SysmdClient::new(s.uri());
        let report = c.solve("part x {{{", SysmdLanguage::Sysml).await.unwrap();
        assert_eq!(report.verdict, SolveVerdict::Error);
        assert_eq!(report.issues[0].kind, "ERROR_SYNTACTICAL");
        assert!(report.variables.is_empty());
    }

    #[tokio::test]
    async fn too_large_and_unreachable_are_distinct_errors() {
        let c = SysmdClient::new("http://127.0.0.1:9");
        let big = "x".repeat(MAX_SOURCE_BYTES + 1);
        assert_eq!(
            c.solve(&big, SysmdLanguage::Sysml).await.unwrap_err(),
            SysmdError::TooLarge
        );
        assert!(matches!(
            c.solve("x", SysmdLanguage::Sysml).await.unwrap_err(),
            SysmdError::Unavailable(_)
        ));
        assert!(matches!(
            c.ping().await.unwrap_err(),
            SysmdError::Unavailable(_)
        ));
    }
}
