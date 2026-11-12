use crate::self_update::application::ports::checksum_signature::ChecksumSignature;
use ring::signature::{UnparsedPublicKey, ED25519};

/// The public half of the release signing key, compiled in: every installed binary carries it forward, so only
/// releases signed with its private half (the `release` environment's `RELEASE_SIGNING_KEY`) update tabsmith.
pub(crate) const RELEASE_SIGNING_KEY_HEX: &str = include_str!("../../../../scripts/release-signing-key.pub");

/// Checks `SHA256SUMS.sig` as an ed25519 signature, with ring, against the release signing key.
pub(crate) struct RingEd25519ChecksumSignature {
    public_key: Vec<u8>,
}

impl RingEd25519ChecksumSignature {
    /// `public_key_hex` is the raw 32-byte key in hex, as `scripts/release-signing-key.pub` holds it. A key that is
    /// not hex verifies nothing, so a broken key file fails closed.
    pub(crate) fn new(public_key_hex: &str) -> Self {
        RingEd25519ChecksumSignature {
            public_key: decode_hex(public_key_hex).unwrap_or_default(),
        }
    }
}

impl ChecksumSignature for RingEd25519ChecksumSignature {
    fn is_authentic(&self, sums: &[u8], signature: &[u8]) -> bool {
        UnparsedPublicKey::new(&ED25519, &self.public_key)
            .verify(sums, signature)
            .is_ok()
    }
}

/// The bytes `text` spells in hex, surrounding whitespace ignored (the key file ends in a newline).
fn decode_hex(text: &str) -> Option<Vec<u8>> {
    let text = text.trim();
    if text.len() % 2 != 0 {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|start| u8::from_str_radix(text.get(start..start + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 8032 §7.1, TEST 1: the key pair's public key and its signature of the empty message.
    const TEST_1_PUBLIC_KEY: &str = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
    const TEST_1_SIGNATURE: &str = "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b";

    fn bytes(hex: &str) -> Vec<u8> {
        decode_hex(hex).unwrap()
    }

    #[test]
    fn should_accept_the_rfc_8032_test_1_signature_of_the_empty_message() {
        let checksum_signature = RingEd25519ChecksumSignature::new(TEST_1_PUBLIC_KEY);

        let authentic = checksum_signature.is_authentic(b"", &bytes(TEST_1_SIGNATURE));

        assert!(authentic);
    }

    #[test]
    fn should_refuse_the_rfc_8032_test_1_signature_with_one_byte_flipped() {
        let checksum_signature = RingEd25519ChecksumSignature::new(TEST_1_PUBLIC_KEY);
        let mut tampered = bytes(TEST_1_SIGNATURE);
        tampered[0] ^= 0x01;

        let authentic = checksum_signature.is_authentic(b"", &tampered);

        assert!(!authentic);
    }

    #[test]
    fn should_refuse_every_signature_when_the_key_is_not_hex() {
        let checksum_signature = RingEd25519ChecksumSignature::new("not a key");

        let authentic = checksum_signature.is_authentic(b"", &bytes(TEST_1_SIGNATURE));

        assert!(!authentic);
    }

    /// A malformed key file fails CI here instead of failing every update in the field.
    #[test]
    fn should_ship_a_release_signing_key_of_32_bytes() {
        let key = decode_hex(RELEASE_SIGNING_KEY_HEX);

        assert_eq!(key.map(|key| key.len()), Some(32));
    }
}
