//! SHA-256 digests.

use sha2::{Digest, Sha256};

/// The SHA-256 of the bytes as lowercase hex.
pub fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// A SHA-256 as lowercase hex.
pub fn is_sha256(value: &str, _context: &()) -> garde::Result {
    let hex = value
        .chars()
        .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c));
    if value.len() != 64 || !hex {
        return Err(garde::Error::new("is not a SHA-256"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_of_empty() {
        assert_eq!(
            hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
