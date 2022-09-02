use rand::prelude::*;

/// Returns a random token of length 16
pub fn random_token() -> String {
    let mut rng = rand::thread_rng();
    let xs: [u8; 16] = rng.gen();

    return hex::encode(&xs);
}
