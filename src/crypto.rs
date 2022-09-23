use derive_more::{Display, Error};
use hmac::{Hmac, Mac};
use jwt::{Header, SignWithKey, Token, VerifyWithKey};
use lazy_static::lazy_static;
use rand::distributions;
use rand::prelude::*;
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::Sha256;

use crate::config;

pub type HmacType = Hmac<Sha256>;
lazy_static! {
    static ref HMAC_SECRET: String = random_token(32);
}

#[derive(Debug, Display, Error)]
pub enum Error {
    #[display(fmt = "JWT verification failed")]
    JwtVerificationFailed(jwt::Error),
}

impl From<jwt::Error> for Error {
    fn from(e: jwt::Error) -> Self {
        Error::JwtVerificationFailed(e)
    }
}

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
    let tok: String = match config::CONFIG.debug {
        true => {
            log::warn!("debug mode enabled, using hard-coded HMAC secret");
            String::from("trapperkeeper")
        }
        false => HMAC_SECRET.clone(),
    };

    Hmac::new_from_slice(tok.as_bytes()).expect("Unable to generate random hmac")
}

/// Encodes a claim using JWT
pub fn jwt_encode<C>(claim: C, key: &Hmac<Sha256>) -> String
where
    C: Serialize,
{
    claim.sign_with_key(key).expect("unable to sign JWT key")
}

/// Encodes a claim using JWT
pub fn jwt_decode<C>(token_str: &String, key: &Hmac<Sha256>) -> Result<C, Error>
where
    C: DeserializeOwned,
    C: Clone,
{
    let token: Token<Header, C, _> = VerifyWithKey::verify_with_key(token_str.as_str(), key)?;

    Ok(token.claims().clone())
}
