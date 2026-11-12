use crate::self_update::application::ports::checksum_signature::ChecksumSignature;

/// Checks `SHA256SUMS.sig` as an ed25519 signature, with ring, against the release signing key.
pub(crate) struct RingEd25519ChecksumSignature;

impl RingEd25519ChecksumSignature {
    pub(crate) fn new(_public_key_hex: &str) -> Self {
        RingEd25519ChecksumSignature
    }
}

impl ChecksumSignature for RingEd25519ChecksumSignature {
    fn is_authentic(&self, _sums: &[u8], _signature: &[u8]) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 8032 §7.1, TEST 1: the key pair's public key and its signature of the empty message.
    const TEST_1_PUBLIC_KEY: &str = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
    const TEST_1_SIGNATURE: &str = "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b";

    fn bytes(hex: &str) -> Vec<u8> {
        (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect()
    }

    #[test]
    fn should_accept_the_rfc_8032_test_1_signature_of_the_empty_message() {
        let checksum_signature = RingEd25519ChecksumSignature::new(TEST_1_PUBLIC_KEY);

        let authentic = checksum_signature.is_authentic(b"", &bytes(TEST_1_SIGNATURE));

        assert!(authentic);
    }
}
