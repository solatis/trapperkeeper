//! Rebuilds the binary when a migration file changes. `migrate!` embeds the
//! files at compile time, and it sees a new file only through this rerun
//! directive, because the build does not set the cfg `sqlx_macros_unstable`
//! with which sqlx tracks the files itself.

fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
