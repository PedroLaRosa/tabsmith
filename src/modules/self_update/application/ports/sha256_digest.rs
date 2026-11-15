/// SHA-256, as the lowercase hex `sha256sum` prints.
pub(crate) trait Sha256Digest {
    fn hex_digest(&self, bytes: &[u8]) -> String;
}

/// Hex-encodes the bytes instead of hashing them, so a test's `SHA256SUMS` stays readable: the "digest" of `b"ok"`
/// is `6f6b`.
#[cfg(test)]
pub(crate) struct HexEncodingDigest;

#[cfg(test)]
impl Sha256Digest for HexEncodingDigest {
    fn hex_digest(&self, bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
