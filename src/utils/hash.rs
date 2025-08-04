use sha1::{Digest, Sha1};

/// Calculate SHA-1 hash using the sha1 crate
pub fn sha1_hash(data: &[u8]) -> [u8; 20] {
    let mut hasher = Sha1::new();
    hasher.update(data);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {}
