use std::process::Command;

/// The binary prints "<name> <version>", the name and version of its package,
/// and exits zero.
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
