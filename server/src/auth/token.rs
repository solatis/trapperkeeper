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
    /// When the token was issued: seconds with a microsecond fraction, which
    /// `NumericDate` allows (RFC 7519, 2), so a comparison with the user's
    /// cutoff is exact.
    pub iat: f64,
    /// When the token expires, in seconds.
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
    /// The signature, the algorithm or the expiry failed.
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
        // 1. read the salt
        // 2. derive the signing key
        // 3. derive the previous key, when one is set
        // 4. assemble the ring

        // 1. A transaction of its own, because store functions take one and
        //    nothing else is read. A missing salt means migrate never ran.
        let salt = {
            let mut tx = store.begin().await?;
            installation::salt(&mut tx).await?
        }
        .ok_or(KeyringError::NoSalt)?;

        // 2. The signing key checks as well as signs, so it goes into the
        //    checking keys under its id.
        let (signing_kid, signing_key) = derive(signing.to_owned(), salt.clone()).await;
        let mut checking = HashMap::new();
        checking.insert(signing_kid.clone(), DecodingKey::from_secret(&signing_key));

        // 3. The previous key only checks, so the tokens it signed stay valid
        //    until they expire.
        if let Some(previous) = previous {
            let (kid, key) = derive(previous.to_owned(), salt).await;
            checking.insert(kid, DecodingKey::from_secret(&key));
        }

        // 4. The same bytes serve as the encoding key and as a decoding key,
        //    because HS256 is symmetric.
        Ok(Self {
            signing_kid,
            signing: EncodingKey::from_secret(&signing_key),
            checking,
        })
    }

    /// A token of `kind` for the user `sub` of tenant `tid`, issued at `now`
    /// and signed by the signing key.
    ///
    /// # Panics
    ///
    /// Never in practice: the claims are plain strings and numbers, which
    /// always serialize, and HS256 signs any input.
    #[must_use]
    pub fn issue(&self, kind: Kind, sub: &str, tid: &str, now: SystemTime) -> String {
        // A clock before 1970 reads as the epoch itself.
        let since_epoch = now.duration_since(UNIX_EPOCH).unwrap_or_default();
        let claims = Claims {
            sub: sub.to_owned(),
            tid: tid.to_owned(),
            // Microseconds divided out, so the fraction survives the float.
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
    /// Refuses a token whose `kid` is missing or not in the ring, whose `typ`
    /// is not the kind's, whose algorithm is not HS256, or whose expiry has
    /// passed.
    pub fn check(&self, kind: Kind, token: &str) -> Result<Claims, Refused> {
        let header = jsonwebtoken::decode_header(token).map_err(Refused::Invalid)?;
        // A token is checked with the key it names, so a removed key's tokens
        // fail and a new key's pass.
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
    // Each derivation takes a 64 MiB hash, once per key at startup
    // (ADR-0014).
    task::spawn_blocking(move || {
        let mut key = [0_u8; 32];
        // The same parameters as passwords, so each offline guess at a
        // passphrase costs one Argon2id run (ADR-0022).
        password::hasher()
            .hash_password_into(passphrase.as_bytes(), salt.as_bytes(), &mut key)
            .expect("the salt and the output length are within argon2's bounds");
        // The id changes only with the key, and nobody types one.
        let fingerprint = Sha256::digest(key);
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
