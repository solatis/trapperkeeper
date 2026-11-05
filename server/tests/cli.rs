//! Runs the binary and checks how it behaves on the command line: the
//! version, usage errors from bad flags, and the failures that stop the
//! process before a subcommand runs. The settings precedence is checked in
//! `settings.rs`.

mod common;

use std::fs;
use std::process::{Command, Output};

use common::Scratch;

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

/// A `--config` file that does not exist fails the process before the
/// subcommand runs, and the message names the path.
#[test]
fn missing_config_file_fails() {
    let out = run(&["--config", "/nonexistent/trapperkeeper.toml", "serve"]);
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(err.contains("/nonexistent/trapperkeeper.toml"));
}

/// `--listen` is parsed as a socket address by clap, so a bad value is a
/// usage error.
#[test]
fn listen_flag_is_parsed() {
    let out = run(&["--listen", "nope", "serve"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("--listen"));
}

/// `--log-level` is parsed as a tracing level by clap, so a bad value is a
/// usage error.
#[test]
fn log_level_flag_is_parsed() {
    let out = run(&["--log-level", "loud", "migrate"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("--log-level"));
}

/// An invalid `RUST_LOG` directive fails the process before the subcommand
/// runs: the message names the variable, and migrate creates no database.
#[test]
fn invalid_rust_log_fails() {
    // The scratch directory is the working directory, where migrate would
    // create the database of the default URL.
    let scratch = Scratch::create();
    let out = Command::new(env!("CARGO_BIN_EXE_trapperkeeper"))
        .current_dir(&scratch.path)
        .env("RUST_LOG", "not=a=directive")
        .arg("migrate")
        .output()
        .expect("the binary runs");
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("RUST_LOG"));
    let entries = fs::read_dir(&scratch.path).expect("the scratch directory is read");
    assert_eq!(entries.count(), 0);
}

/// serve without `auth.signing_key` fails before it binds, and the message
/// names the setting. The binary runs directly, with no key in its
/// environment.
#[test]
fn serve_refuses_without_a_signing_key() {
    let scratch = Scratch::create();
    let url = scratch.sqlite_url("store.db");
    assert!(common::run_migrate(&url).status.success());
    let out = Command::new(env!("CARGO_BIN_EXE_trapperkeeper"))
        .env_remove("TRAPPERKEEPER_AUTH__SIGNING_KEY")
        .args(["serve", "--listen", "127.0.0.1:0", "--database-url", &url])
        .output()
        .expect("the binary runs");
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("auth.signing_key"));
}

/// A signing key shorter than 32 characters is refused with the settings,
/// before any subcommand runs.
#[test]
fn a_short_signing_key_fails() {
    let out = Command::new(env!("CARGO_BIN_EXE_trapperkeeper"))
        .env("TRAPPERKEEPER_AUTH__SIGNING_KEY", "too short")
        .args(["serve", "--listen", "127.0.0.1:0"])
        .output()
        .expect("the binary runs");
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("32 characters"));
}
