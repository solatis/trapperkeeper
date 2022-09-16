use rand::distributions;
use rand::prelude::*;

use hmac::{Hmac, Mac};
use jwt::{Header, SignWithKey, Token, VerifyWithKey};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::Sha256;

pub type HmacType = Hmac<Sha256>;

/// Returns a secure random token of length `n`
pub fn random_token(n: usize) -> String {
    let rng = rand::thread_rng();

    rng.sample_iter(distributions::Alphanumeric)
        .take(n)
        .map(char::from)
        .collect()
}

/// Generates a random HMAC key.
pub fn random_hmac() -> HmacType {
    Hmac::new_from_slice(random_token(32).as_bytes()).unwrap()
}

/// Encodes a claim using JWT
pub fn jwt_encode<C>(claim: C, key: &Hmac<Sha256>) -> String
where
    C: Serialize,
{
    claim.sign_with_key(key).expect("unable to sign JWT key")
}

/// Encodes a claim using JWT
pub fn jwt_decode<C>(token_str: String, key: &Hmac<Sha256>) -> C
where
    C: DeserializeOwned,
    C: Copy,
{
    let token: Token<Header, C, _> =
        VerifyWithKey::verify_with_key(token_str.as_str(), key).unwrap();

    *token.claims()
}
