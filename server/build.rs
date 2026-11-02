//! Rebuilds the binary when a migration file changes. `migrate!` embeds the
//! files at compile time, and on stable Rust it sees a new file only through
//! this rerun directive.

fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
