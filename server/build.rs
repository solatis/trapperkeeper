//! Rebuilds the binary when a migration file changes. `migrate!` embeds the
//! files at compile time, and it sees a new file only through this rerun
//! directive. sqlx tracks the files itself only under the cfg
//! `sqlx_macros_unstable`, which turns on the unstable features
//! `proc_macro_tracked_path` and `proc_macro_tracked_env`. The project turns
//! on an unstable feature only where a decision record names it, and no record
//! names these (ADR-0009).

fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
