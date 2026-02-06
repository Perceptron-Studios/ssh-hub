use md5::{Digest, Md5};

/// Calculate MD5 checksum of content
pub fn md5_hash(content: &[u8]) -> String {
    let mut hasher = Md5::new();
    hasher.update(content);
    let result = hasher.finalize();
    format!("{:x}", result)
}