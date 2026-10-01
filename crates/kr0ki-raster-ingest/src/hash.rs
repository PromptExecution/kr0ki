//! The one SHA-256 hex helper the crate uses (content identity, source identity, cache keys).

use sha2::{Digest, Sha256};

/// Lower-case hex of a digest or any byte string.
pub(crate) fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn sha256_hex(bytes: impl AsRef<[u8]>) -> String {
    hex(Sha256::digest(bytes.as_ref()))
}
