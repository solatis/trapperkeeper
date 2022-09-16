use crate::crypto;

/// Returns a secure random token of length `n`
pub fn random_token(n: usize) -> String {
    crypto::random_token(n)
}
