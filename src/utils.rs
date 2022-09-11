use rand::distributions;
use rand::prelude::*;

/// Returns a secure random token of length `n`
pub fn random_token(n: usize) -> String {
    let rng = rand::thread_rng();

    rng.sample_iter(distributions::Alphanumeric)
        .take(n)
        .map(char::from)
        .collect()
}
