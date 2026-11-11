//! Password hashing with Argon2id, and generated passwords
//! (ADR-0022).

use std::num::NonZero;
use std::sync::OnceLock;
use std::thread;

use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::distr::{Alphanumeric, SampleString};
use tokio::sync::Semaphore;
use tokio::task;

/// Argon2id with 64 MiB of memory, 3 passes and 4 lanes.
pub fn hasher() -> Argon2<'static> {
    // The second recommended option of RFC 9106, 4.
    let params =
        Params::new(64 * 1024, 3, 4, None).expect("the parameters are within argon2's bounds");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

/// Limits how many hashes run at once to the number of CPUs.
static HASHING: OnceLock<Semaphore> = OnceLock::new();

fn hashing() -> &'static Semaphore {
    // Login is unauthenticated, so a flood of attempts queues here instead of
    // allocating 64 MiB each.
    HASHING.get_or_init(|| Semaphore::new(thread::available_parallelism().map_or(1, NonZero::get)))
}

/// The PHC string of `password`, hashed with a fresh random salt.
pub async fn hash(password: String) -> String {
    let _permit = hashing()
        .acquire()
        .await
        .expect("the semaphore is never closed");
    // A hash takes tens of milliseconds of CPU or more, which must not block
    // the runtime (ADR-0014).
    task::spawn_blocking(move || {
        hasher()
            .hash_password(password.as_bytes())
            .expect("argon2 hashes any password")
            .to_string()
    })
    .await
    .expect("hashing does not panic")
}

/// Whether `password` hashes to `phc` under the salt and parameters that `phc`
/// carries. A string that does not parse matches nothing.
pub async fn verify(password: String, phc: String) -> bool {
    let _permit = hashing()
        .acquire()
        .await
        .expect("the semaphore is never closed");
    // argon2 compares the two outputs in constant time.
    task::spawn_blocking(move || {
        hasher()
            .verify_password(password.as_bytes(), phc.as_str())
            .is_ok()
    })
    .await
    .expect("verifying does not panic")
}

/// A new password of 16 random ASCII letters and digits.
pub fn generate() -> String {
    // 16 characters from 62 symbols are about 95 bits, enough for a password
    // its holder replaces at the first login.
    Alphanumeric.sample_string(&mut rand::rng(), 16)
}
