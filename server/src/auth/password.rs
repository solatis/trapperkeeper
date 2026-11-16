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
    // The parameters are the second recommended option in section 4 of
    // RFC 9106.
    let params =
        Params::new(64 * 1024, 3, 4, None).expect("the parameters are within argon2's bounds");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

/// Limits how many hashes run at once to the number of CPUs. Login is
/// unauthenticated and each hash holds 64 MiB while it runs, so under a flood
/// of attempts the attempts wait for a permit instead of each allocating
/// 64 MiB.
static HASHING: OnceLock<Semaphore> = OnceLock::new();

fn hashing() -> &'static Semaphore {
    // When the host does not report its CPU count, the limit is one permit,
    // the smallest limit that still lets a hash run. The value is a choice
    // that no measurement backs.
    HASHING.get_or_init(|| Semaphore::new(thread::available_parallelism().map_or(1, NonZero::get)))
}

/// The PHC string of `password`, hashed with a fresh random salt. The call
/// waits for a free slot while as many hashes run as the limit allows.
pub async fn hash(password: String) -> String {
    let permit = hashing()
        .acquire()
        .await
        .expect("the semaphore is never closed");
    // One Argon2id hash is CPU-bound work over 64 MiB and 3 passes, which
    // must not block the runtime (ADR-0014).
    task::spawn_blocking(move || {
        // The blocking task holds the permit, so a caller that drops this
        // future does not free it while the hash still runs.
        let _permit = permit;
        hasher()
            .hash_password(password.as_bytes())
            .expect("argon2 hashes any password")
            .to_string()
    })
    .await
    .expect("hashing does not panic")
}

/// Whether `password` hashes to `phc` under the salt and parameters that `phc`
/// carries. A string that does not parse matches nothing. The call waits for
/// a free slot while as many hashes run as the limit allows.
pub async fn verify(password: String, phc: String) -> bool {
    let permit = hashing()
        .acquire()
        .await
        .expect("the semaphore is never closed");
    task::spawn_blocking(move || {
        // The blocking task holds the permit, so a caller that drops this
        // future does not free it while the hash still runs.
        let _permit = permit;
        // `verify_password` takes the algorithm, the parameters and the salt
        // from `phc`, so the parameters of `hasher` do not count here. argon2
        // compares the two outputs in constant time.
        hasher()
            .verify_password(password.as_bytes(), phc.as_str())
            .is_ok()
    })
    .await
    .expect("verifying does not panic")
}

/// A new password of 16 random ASCII letters and digits.
pub fn generate() -> String {
    // The length 16 is a choice. 16 characters drawn from 62 symbols give
    // about 95 bits, which is enough, because the holder is asked to change
    // the password at the first login.
    Alphanumeric.sample_string(&mut rand::rng(), 16)
}

#[cfg(test)]
mod tests {
    use super::{hash, hashing};

    #[tokio::test]
    async fn a_dropped_hash_keeps_its_permit() {
        // The test drops a caller of `hash` while its hash still runs, and
        // checks that the hash keeps its permit.
        //
        // 1. We start a hash in a task of its own and wait until it has taken
        //    a permit.
        // 2. We abort the task and wait for it to end, which drops the future
        //    of `hash`. The hash goes on in its blocking thread, because
        //    tokio does not stop a blocking task that has started.
        // 3. We check that the permit is still taken.

        // The limit is at least one permit, and no other test of this binary
        // hashes, so `free` is at least 1 and only this test moves the
        // permits.
        let free = hashing().available_permits();
        // 1. start a hash
        let caller = tokio::spawn(hash(String::new()));
        while hashing().available_permits() == free {
            tokio::task::yield_now().await;
        }
        // 2. abort the caller
        caller.abort();
        // The task ends with the error of a cancelled task, which is the
        // outcome that the test asked for.
        let _ = caller.await;
        // 3. check the permit
        // One Argon2id hash takes far longer than the abort, so the hash is
        // still running here.
        assert!(hashing().available_permits() < free);
    }
}
