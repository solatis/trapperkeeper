//! Runs the binary and checks how it behaves on the command line.

use std::process::{Command, Output};

/// Runs the binary with `args` and returns its output.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_trapperkeeper"))
        .args(args)
        .output()
        .expect("the binary runs")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// `--version` prints the binary name and the package version and exits zero.
#[test]
fn version_line() {
    let out = run(&["--version"]);
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        concat!("trapperkeeper ", env!("CARGO_PKG_VERSION"), "\n")
    );
}

/// No subcommand is a usage error: clap exits with status 2 and prints the
/// usage on stderr.
#[test]
fn no_subcommand_is_a_usage_error() {
    let out = run(&[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("Usage:"));
}

/// `serve` is a placeholder: it fails with a status other than clap's 2 and
/// says so on stderr.
#[test]
fn serve_is_not_implemented() {
    let out = run(&["serve"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("not implemented yet"));
}

/// `migrate` is a placeholder in the same way.
#[test]
fn migrate_is_not_implemented() {
    let out = run(&["migrate"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("not implemented yet"));
}

/// A `--config` file that does not exist fails the process before the
/// subcommand runs, and the message names the path.
#[test]
fn missing_config_file_fails() {
    let out = run(&["--config", "/nonexistent/trapperkeeper.toml", "serve"]);
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(err.contains("/nonexistent/trapperkeeper.toml"));
    assert!(!err.contains("not implemented yet"));
}

/// `--listen` is parsed as a socket address by clap, so a bad value is a
/// usage error.
#[test]
fn listen_flag_is_parsed() {
    let out = run(&["--listen", "nope", "serve"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("--listen"));
}
