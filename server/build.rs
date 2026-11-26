//! Rebuilds the binary when a migration file changes. `migrate!` embeds the
//! files at compile time, and it sees a new file only through this rerun
//! directive. sqlx tracks the files itself only under the cfg
//! `sqlx_macros_unstable`, which turns on the unstable feature
//! `proc_macro_tracked_path`. The project turns on an unstable feature only
//! where a decision record names it, and no record names this one (ADR-0009).

fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
