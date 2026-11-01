// Temporary. This test exists so that `cargo test` executes real code on every
// CI platform before the server has any. The issue that adds the subcommands
// removes it (owner, 2026-11-01).

use std::process::Command;

/// The binary prints "<name> <version>", its own binary name and the version
/// of its package, and exits zero.
#[test]
fn prints_name_and_version() {
    let out = Command::new(env!("CARGO_BIN_EXE_trapperkeeper"))
        .output()
        .expect("the binary runs");
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        concat!("trapperkeeper ", env!("CARGO_PKG_VERSION"), "\n")
    );
}
