/// The release signing key's check of a release's `SHA256SUMS.sig`.
pub(crate) trait ChecksumSignature {
    /// `signature` is the release signing key's signature over `sums`.
    fn is_authentic(&self, sums: &[u8], signature: &[u8]) -> bool;
}

/// Takes `sig:` followed by the sums for their valid signature, so a test can sign by hand.
#[cfg(test)]
pub(crate) struct PrefixedChecksumSignature;

#[cfg(test)]
impl ChecksumSignature for PrefixedChecksumSignature {
    fn is_authentic(&self, sums: &[u8], signature: &[u8]) -> bool {
        signature.strip_prefix(b"sig:") == Some(sums)
    }
}
