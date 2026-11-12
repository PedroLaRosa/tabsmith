use crate::modules::self_update::application::ports::sha256_digest::Sha256Digest;

/// SHA-256 from ring, already built in through rustls.
pub(crate) struct RingSha256Digest;

impl Sha256Digest for RingSha256Digest {
    fn hex_digest(&self, bytes: &[u8]) -> String {
        let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
        digest.as_ref().iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FIPS 180-2, appendix B.1.
    #[test]
    fn should_give_the_sha256_of_abc() {
        let digest = RingSha256Digest.hex_digest(b"abc");

        assert_eq!(digest, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}
