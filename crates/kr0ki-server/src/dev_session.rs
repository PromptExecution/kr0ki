//! Plan 004 Phase 0 item 4 — the authenticated session/grant interface.
//!
//! Local development stands in for the production identity provider with a
//! signed dev session (HMAC-SHA256 over a canonical claim set). Plan 004 is
//! explicit that a bare thread ID is NOT a session; even this dev path
//! requires a signed token and a workspace grant that bounds every write to
//! one workspace with an allowed format allowlist. The production IdP decision
//! replaces `DevSession::issue`/`verify` implementations behind the same
//! `SessionIdentity` seam — call sites never learn how signing works.
//!
//! Not yet called from routes: Phase 1's workspace service consumes this seam
//! when it lands (Plan 004 §6 Phase 1.1). Until then the bin crate has no
//! non-test caller, hence the module-scoped dead-code allowance.

#![allow(dead_code)] // Phase-1 seam: consumed by the workspace service (see module docs)

use crate::workspace_types::{Actor, ProjectGrant};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// What a session is allowed to do inside its one workspace. Derived from the
/// workspace's `ProjectGrant` at issue time; never supplied by the client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionClaims {
    /// `dev:<subject>` — the human/agent identity this session was minted for.
    pub subject: String,
    pub workspace_id: String,
    /// Capability strings: `diagram.read`, `diagram.propose`, `diagram.accept`.
    pub capabilities: Vec<String>,
    /// Unix seconds; 0 = no expiry (explicit dev convenience, not production).
    pub expires_at: u64,
}

/// Errors a session check can produce. Typed so the HTTP layer maps each to
/// its own status and the audit log records exactly why a write was denied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    Malformed,
    BadSignature,
    Expired,
    /// Signed fine, but the capability is not in this session's grant.
    CapabilityDenied {
        required: &'static str,
    },
}

/// The seam the workspace service uses. `issue` is dev-only; production
/// sessions arrive from the IdP via a future adapter of this trait.
pub trait SessionIdentity: Send + Sync {
    fn verify(&self, token: &str) -> Result<SessionClaims, SessionError>;
}

/// HMAC-signed dev session. `KR0KI_DEV_SESSION_KEY` keys it; absent or empty
/// key = dev sessions disabled server-side (the gate must be on, not open).
pub struct DevSession {
    key: Vec<u8>,
}

impl DevSession {
    /// Returns `None` when dev sessions are disabled (empty/unset key).
    pub fn from_env_key(key: Option<&str>) -> Option<Self> {
        let key = key?.trim();
        if key.is_empty() {
            None
        } else {
            Some(Self {
                key: key.as_bytes().to_vec(),
            })
        }
    }

    /// Mint a token for local development. Panics only on a poisoned HMAC
    /// (impossible with HMAC-SHA256 over fixed key material).
    pub fn issue(&self, claims: &SessionClaims) -> String {
        let payload = serde_json::to_string(claims).expect("claims serialize");
        let mut mac = HmacSha256::new_from_slice(&self.key).expect("hmac key");
        mac.update(payload.as_bytes());
        let sig = hex(&mac.finalize().into_bytes());
        // payload is JSON (no dots); base64-avoidance: hex payload keeps the
        // token dot-splittable without an encoder dependency.
        let payload_hex = payload
            .bytes()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        format!("{payload_hex}.{sig}")
    }

    fn sign_hex(&self, payload_hex: &str) -> Option<String> {
        let mut payload = Vec::with_capacity(payload_hex.len() / 2);
        let bytes = payload_hex.as_bytes();
        if !bytes.len().is_multiple_of(2) {
            return None;
        }
        for chunk in bytes.chunks(2) {
            let hi = std::str::from_utf8(&chunk[..1]).ok()?;
            let lo = std::str::from_utf8(&chunk[1..]).ok()?;
            payload.push(u8::from_str_radix(hi, 16).ok()? * 16 + u8::from_str_radix(lo, 16).ok()?);
        }
        let mut mac = HmacSha256::new_from_slice(&self.key).ok()?;
        mac.update(&payload);
        Some(hex(&mac.finalize().into_bytes()))
    }
}

impl SessionIdentity for DevSession {
    fn verify(&self, token: &str) -> Result<SessionClaims, SessionError> {
        let (payload_hex, sig) = token.split_once('.').ok_or(SessionError::Malformed)?;
        let expected = self.sign_hex(payload_hex).ok_or(SessionError::Malformed)?;
        if !constant_time_eq(sig.as_bytes(), expected.as_bytes()) {
            return Err(SessionError::BadSignature);
        }
        let payload = {
            let bytes = payload_hex.as_bytes();
            if !bytes.len().is_multiple_of(2) {
                return Err(SessionError::Malformed);
            }
            let mut out = Vec::with_capacity(bytes.len() / 2);
            for chunk in bytes.chunks(2) {
                let hi = std::str::from_utf8(&chunk[..1]).map_err(|_| SessionError::Malformed)?;
                let lo = std::str::from_utf8(&chunk[1..]).map_err(|_| SessionError::Malformed)?;
                let hi = u8::from_str_radix(hi, 16).map_err(|_| SessionError::Malformed)?;
                let lo = u8::from_str_radix(lo, 16).map_err(|_| SessionError::Malformed)?;
                out.push(hi * 16 + lo);
            }
            out
        };
        let claims: SessionClaims =
            serde_json::from_slice(&payload).map_err(|_| SessionError::Malformed)?;
        if claims.expires_at != 0 {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(u64::MAX);
            if now >= claims.expires_at {
                return Err(SessionError::Expired);
            }
        }
        Ok(claims)
    }
}

impl SessionClaims {
    /// Capability check used by every workspace write path.
    pub fn require(&self, capability: &str) -> Result<(), SessionError> {
        if self.capabilities.iter().any(|c| c == capability) {
            Ok(())
        } else {
            Err(SessionError::CapabilityDenied { required: "" })
        }
    }

    /// The actor this session speaks for (Plan 004 `Revision.author`).
    pub fn actor(&self) -> Actor {
        Actor::DevSession {
            subject: self.subject.clone(),
        }
    }

    /// Check this session's grant covers a proposed render format.
    pub fn format_allowed(&self, grant: &ProjectGrant, format: &str) -> bool {
        grant.allowed_formats.iter().any(|f| f == format)
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims() -> SessionClaims {
        SessionClaims {
            subject: "dev:operator".into(),
            workspace_id: "ws_1".into(),
            capabilities: vec!["diagram.read".into(), "diagram.propose".into()],
            expires_at: 0,
        }
    }

    #[test]
    fn issue_then_verify_roundtrips() {
        let dev = DevSession::from_env_key(Some("test-key")).unwrap();
        let token = dev.issue(&claims());
        let got = dev.verify(&token).unwrap();
        assert_eq!(got, claims());
        assert_eq!(
            got.actor(),
            Actor::DevSession {
                subject: "dev:operator".into()
            }
        );
    }

    #[test]
    fn tampered_payload_is_bad_signature() {
        let dev = DevSession::from_env_key(Some("test-key")).unwrap();
        let token = dev.issue(&claims());
        // flip one payload char
        let (payload, sig) = token.split_once('.').unwrap();
        let flipped = if payload.starts_with('a') {
            format!("b{payload}")[..payload.len()].to_string()
        } else {
            format!("a{}", &payload[1..])
        };
        assert_eq!(
            dev.verify(&format!("{flipped}.{sig}")),
            Err(SessionError::BadSignature)
        );
    }

    #[test]
    fn wrong_key_is_bad_signature() {
        let dev_a = DevSession::from_env_key(Some("key-a")).unwrap();
        let dev_b = DevSession::from_env_key(Some("key-b")).unwrap();
        let token = dev_a.issue(&claims());
        assert_eq!(dev_b.verify(&token), Err(SessionError::BadSignature));
    }

    #[test]
    fn missing_or_malformed_token_is_malformed() {
        let dev = DevSession::from_env_key(Some("k")).unwrap();
        assert_eq!(dev.verify("no-dot"), Err(SessionError::Malformed));
        assert_eq!(dev.verify(""), Err(SessionError::Malformed));
    }

    #[test]
    fn empty_key_disables_dev_sessions() {
        assert!(DevSession::from_env_key(Some("")).is_none());
        assert!(DevSession::from_env_key(Some("  ")).is_none());
        assert!(DevSession::from_env_key(None).is_none());
    }

    #[test]
    fn expired_claims_are_rejected() {
        let dev = DevSession::from_env_key(Some("k")).unwrap();
        let past = SessionClaims {
            expires_at: 1,
            ..claims()
        };
        assert_eq!(dev.verify(&dev.issue(&past)), Err(SessionError::Expired));
    }

    #[test]
    fn capability_check_denies_missing_capability() {
        let c = claims();
        assert!(c.require("diagram.read").is_ok());
        assert!(c.require("diagram.propose").is_ok());
        assert!(c.require("diagram.accept").is_err());
        assert!(c.require("workspace.admin").is_err());
    }

    #[test]
    fn format_allowlist_is_grant_checked() {
        let c = claims();
        let grant = ProjectGrant {
            project_id: "p1".into(),
            project_root: "/tmp/demo".into(),
            allowed_formats: vec!["d2".into()],
            can_read: true,
            can_write: true,
            can_materialize: false,
        };
        assert!(c.format_allowed(&grant, "d2"));
        assert!(!c.format_allowed(&grant, "plantuml"));
    }
}
