use sha2::{Digest, Sha256};

pub fn sha256_hex(input: impl AsRef<[u8]>) -> String {
    Sha256::digest(input.as_ref())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
