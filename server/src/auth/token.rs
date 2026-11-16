//! Signed tokens: HS256 JWTs that name their signing key in `kid` and their
//! kind in `typ` (ADR-0025).

use std::collections::HashMap;
use std::fmt;
use std::fmt::Write as _;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::task;

use crate::auth::password;
use crate::store::{Store, installation};

/// The kinds of token the server signs. Each kind has its own `typ` and
/// lifetime, and a token of one kind never passes as the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Proves who the caller is on a management route.
    Access,
    /// Trades for a new access token at the refresh route.
    Refresh,
}

impl Kind {
    /// The `typ` header of this kind.
    fn typ(self) -> &'static str {
        match self {
            // The media type RFC 9068, 2.1 gives access token JWTs.
            Self::Access => "at+jwt",
            // A type of our own, because RFC 9068 defines only the access
            // token's, and the two kinds must differ (RFC 8725, 3.11).
            Self::Refresh => "tk-refresh+jwt",
        }
    }

    /// How long a token of this kind lives.
    #[must_use]
    pub fn lifetime(self) -> Duration {
        // ADR-0025 sets both lifetimes. The refresh route issues no new
        // refresh token, so 7 days is also how long a login lasts, and how
        // long an operator leaves the previous key set after a rotation.
        match self {
            Self::Access => Duration::from_hours(1),
            Self::Refresh => Duration::from_hours(7 * 24),
        }
    }
}

/// The claims of a token.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Claims {
    /// The user's id.
    pub sub: String,
    /// The user's tenant id.
    pub tid: String,
    /// The issue time in seconds since the Unix epoch, with a microsecond
    /// fraction, which a `NumericDate` allows (RFC 7519, 2). The fraction
    /// lets the check of a caller compare the issue time exactly with the
    /// user's `tokens_valid_after`, the cutoff that revokes earlier tokens.
    pub iat: f64,
    /// The expiry in whole seconds since the Unix epoch.
    pub exp: u64,
}

impl Claims {
    /// When the token was issued, as epoch microseconds.
    #[must_use]
    pub fn issued_at_micros(&self) -> i64 {
        // An f64 holds today's epoch microseconds exactly, so the rounding
        // undoes the division at issue.
        #[allow(clippy::cast_possible_truncation)]
        let micros = (self.iat * 1e6).round() as i64;
        micros
    }
}

/// Why a token was refused.
#[derive(Debug)]
pub enum Refused {
    /// The header names no key, or one the ring does not hold.
    UnknownKey,
    /// The `typ` is not the asked kind's.
    WrongKind,
    /// jsonwebtoken refused the token: a header or body that it cannot parse,
    /// a signature that does not verify, an algorithm other than HS256, or an
    /// expiry that has passed.
    Invalid(jsonwebtoken::errors::Error),
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownKey => f.write_str("the token names no key of this server"),
            Self::WrongKind => f.write_str("the token is of another kind"),
            Self::Invalid(error) => write!(f, "{error}"),
        }
    }
}

/// Why the keyring could not be built from the store.
#[derive(Debug)]
pub enum KeyringError {
    /// The installation has no salt, so migrate has not run on this database.
    NoSalt,
    /// The store could not be reached or read while the salt was fetched.
    Database(sqlx::Error),
}

impl fmt::Display for KeyringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSalt => {
                f.write_str("the database has no signing salt; run `trapperkeeper migrate`")
            }
            Self::Database(error) => write!(f, "{error}"),
        }
    }
}

impl From<sqlx::Error> for KeyringError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

/// The keys a running server signs and checks with: the signing key, and
/// the previous key when one is set.
pub struct Keyring {
    signing_kid: String,
    signing: EncodingKey,
    checking: HashMap<String, DecodingKey>,
}

impl Keyring {
    /// Derives the keys from the passphrases and the salt the store holds.
    ///
    /// # Errors
    ///
    /// Fails when the store has no salt or cannot be read.
    pub async fn from_store(
        store: &Store,
        signing: &str,
        previous: Option<&str>,
    ) -> Result<Self, KeyringError> {
        // `installation::salt` takes a transaction, so `from_store` opens one
        // for this read alone. The block drops it at its end, which rolls the
        // read back and changes nothing, so no connection is held while the
        // two derivations run.
        let salt = {
            let mut tx = store.begin().await?;
            installation::salt(&mut tx).await?
        }
        .ok_or(KeyringError::NoSalt)?;

        let (signing_kid, signing_key) = derive(signing.to_owned(), salt.clone()).await;
        let mut checking = HashMap::new();
        // The signing key also goes into the checking keys under its id,
        // because it checks the tokens that it signs.
        checking.insert(signing_kid.clone(), DecodingKey::from_secret(&signing_key));

        // The previous key goes into the checking keys alone, so the tokens
        // that it signed before a rotation keep passing `check` while it is
        // set.
        if let Some(previous) = previous {
            let (kid, key) = derive(previous.to_owned(), salt).await;
            checking.insert(kid, DecodingKey::from_secret(&key));
        }

        Ok(Self {
            signing_kid,
            // The bytes of the signing key make the encoding key here and its
            // decoding key above, because HS256 is symmetric.
            signing: EncodingKey::from_secret(&signing_key),
            checking,
        })
    }

    /// A token of `kind` for the user `sub` of tenant `tid`, issued at `now`
    /// and signed by the signing key.
    ///
    /// # Panics
    ///
    /// `issue` does not panic in practice, because the claims hold only
    /// strings and numbers, which `serde_json` always serializes, and HS256
    /// signs any input.
    #[must_use]
    pub fn issue(&self, kind: Kind, sub: &str, tid: &str, now: SystemTime) -> String {
        // A clock before 1970 reads as the epoch itself.
        let since_epoch = now.duration_since(UNIX_EPOCH).unwrap_or_default();
        let claims = Claims {
            sub: sub.to_owned(),
            tid: tid.to_owned(),
            // The division gives the seconds with the microsecond fraction
            // that `Claims::iat` describes.
            #[allow(clippy::cast_precision_loss)]
            iat: since_epoch.as_micros() as f64 / 1e6,
            exp: (since_epoch + kind.lifetime()).as_secs(),
        };
        let header = Header {
            kid: Some(self.signing_kid.clone()),
            typ: Some(kind.typ().to_owned()),
            ..Header::new(Algorithm::HS256)
        };
        jsonwebtoken::encode(&header, &claims, &self.signing).expect("HS256 signs any claims")
    }

    /// The claims of `token` when it is a valid token of `kind`.
    ///
    /// # Errors
    ///
    /// Refuses a token that does not parse, whose `kid` is missing or names a
    /// key the ring does not hold, whose `typ` differs from the `typ` of
    /// `kind`, whose algorithm is not HS256, whose signature does not verify,
    /// or whose expiry passed more than 60 seconds ago.
    pub fn check(&self, kind: Kind, token: &str) -> Result<Claims, Refused> {
        let header = jsonwebtoken::decode_header(token).map_err(Refused::Invalid)?;
        // `check` verifies each token with the key that its `kid` names, so a
        // token signed by a key that has left the ring fails, and a token
        // signed by either key of the ring passes.
        let key = header
            .kid
            .as_deref()
            .and_then(|kid| self.checking.get(kid))
            .ok_or(Refused::UnknownKey)?;
        // jsonwebtoken does not check typ, so this line keeps a token of one
        // kind from passing as another (RFC 8725, 3.11).
        if header.typ.as_deref() != Some(kind.typ()) {
            return Err(Refused::WrongKind);
        }
        // The server fixes the algorithm and never takes it from the header.
        // The default leeway of 60 seconds absorbs clock skew between
        // instances.
        let validation = Validation::new(Algorithm::HS256);
        jsonwebtoken::decode(token, key, &validation)
            .map(|data| data.claims)
            .map_err(Refused::Invalid)
    }
}

/// Stretches `passphrase` under `salt` into a 32-byte key and its id, the
/// first 16 hex characters of the key's SHA-256.
async fn derive(passphrase: String, salt: String) -> (String, [u8; 32]) {
    // Each derivation runs one Argon2id hash with 64 MiB of memory, which
    // must not block the runtime, so it runs on the blocking pool (ADR-0014).
    // The derivations take no permit. The hashing limit guards against a
    // flood of logins (ADR-0022), and serve runs the derivations once, one
    // after the other, before it accepts a request.
    task::spawn_blocking(move || {
        let mut key = [0_u8; 32];
        // The derivation uses the parameters of the password hasher
        // (ADR-0022). The stretching makes each offline guess at a passphrase
        // cost one Argon2id run (ADR-0025).
        password::hasher()
            // The salt is the 32 hex characters that
            // `installation::ensure_salt` wrote, which is above argon2's
            // minimum of 8 bytes.
            .hash_password_into(passphrase.as_bytes(), salt.as_bytes(), &mut key)
            .expect("the salt and the output length are within argon2's bounds");
        // The id is a fingerprint of the derived key, so nobody types one
        // (ADR-0025). It is a hash, so the `kid` in the header of a token
        // shows no byte of the key.
        let fingerprint = Sha256::digest(key);
        // The length of the id is a choice that no measurement backs.
        let kid = fingerprint
            .iter()
            .take(8)
            .fold(String::new(), |mut hex, byte| {
                let _ = write!(hex, "{byte:02x}");
                hex
            });
        (kid, key)
    })
    .await
    .expect("deriving does not panic")
}
