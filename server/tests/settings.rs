//! Checks the precedence of the settings layers through the binary. Three
//! settings are covered: the listen address, the log level and the database
//! URL. For each, the file, the environment and the flags each give a random
//! value or stay silent. The binary logs the settings it resolved, and each
//! must be the value of the highest layer that gave one, or the default when
//! none did. One property over generated layers instead of a case per layer,
//! as `doc/adr/0018` asks. The test runs `migrate`, because it resolves and
//! logs the settings without needing a port.

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use proptest::prelude::*;
use serde_json::Value;

/// The level names the settings accept, as a file would spell them.
const LEVELS: [&str; 6] = ["trace", "debug", "info", "warn", "error", "off"];

/// What one layer says about each setting; `None` leaves the setting to the
/// layers below.
#[derive(Debug, Clone)]
struct Layer {
    listen: Option<u16>,
    level: Option<&'static str>,
    database: Option<u8>,
}

fn layer() -> impl Strategy<Value = Layer> {
    (
        proptest::option::of(any::<u16>()),
        proptest::option::of(proptest::sample::select(&LEVELS[..])),
        proptest::option::of(any::<u8>()),
    )
        .prop_map(|(listen, level, database)| Layer {
            listen,
            level,
            database,
        })
}

fn address(port: u16) -> String {
    format!("127.0.0.1:{port}")
}

fn database_url(n: u8) -> String {
    format!("sqlite://db-{n}.db")
}

/// Gives each run its own configuration file, because the runs of one test
/// execute on parallel threads and each writes a file.
static RUN: AtomicU32 = AtomicU32::new(0);

/// Runs `migrate` with the three layers and returns the settings it logged.
fn resolved(file: &Layer, env: &Layer, flags: &Layer) -> Value {
    // 1. Write the file layer as TOML, even when empty, so the file path is
    //    always exercised.
    let path: PathBuf = std::env::temp_dir().join(format!(
        "trapperkeeper-settings-{}-{}.toml",
        std::process::id(),
        RUN.fetch_add(1, Ordering::Relaxed)
    ));
    let mut toml = String::new();
    if let Some(port) = file.listen {
        writeln!(toml, "listen = \"{}\"", address(port)).expect("a string grows");
    }
    if let Some(level) = file.level {
        writeln!(toml, "[log]\nlevel = \"{level}\"").expect("a string grows");
    }
    if let Some(n) = file.database {
        writeln!(toml, "[database]\nurl = \"{}\"", database_url(n)).expect("a string grows");
    }
    fs::write(&path, toml).expect("the file is written");

    // 2. Run the binary with the environment and flag layers. The outer
    //    environment's own variables are removed first, so a developer's shell
    //    cannot leak into the property. RUST_LOG at debug makes the settings
    //    line appear whatever level the layers chose, because the directive
    //    replaces the setting for the filter and leaves the resolved value as
    //    it is.
    let mut command = Command::new(env!("CARGO_BIN_EXE_trapperkeeper"));
    command
        .env_remove("TRAPPERKEEPER_LISTEN")
        .env_remove("TRAPPERKEEPER_LOG__LEVEL")
        .env_remove("TRAPPERKEEPER_DATABASE__URL")
        .env("RUST_LOG", "debug")
        .arg("--config")
        .arg(&path);
    if let Some(port) = env.listen {
        command.env("TRAPPERKEEPER_LISTEN", address(port));
    }
    if let Some(level) = env.level {
        command.env("TRAPPERKEEPER_LOG__LEVEL", level);
    }
    if let Some(n) = env.database {
        command.env("TRAPPERKEEPER_DATABASE__URL", database_url(n));
    }
    if let Some(port) = flags.listen {
        command.arg("--listen").arg(address(port));
    }
    if let Some(level) = flags.level {
        command.arg("--log-level").arg(level);
    }
    if let Some(n) = flags.database {
        command.arg("--database-url").arg(database_url(n));
    }
    command.arg("migrate");
    let output = command.output().expect("the binary runs");
    fs::remove_file(&path).expect("the file is removed");

    // 3. Find the settings line among the JSON lines on stdout and parse the
    //    settings it carries.
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 stdout");
    let line = stdout
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("one JSON object per line"))
        .find(|line| line["fields"]["message"] == "settings resolved")
        .unwrap_or_else(|| panic!("no settings line in:\n{stdout}"));
    let settings = line["fields"]["settings"]
        .as_str()
        .expect("the settings are a JSON string");
    serde_json::from_str(settings).expect("the settings parse")
}

proptest! {
    // 32 runs rather than proptest's 256: each run starts a process, and the
    // property has few shapes to cover. No regression file, so a failure
    // leaves nothing untracked in the tree; the failing seed is in the output.
    #![proptest_config(ProptestConfig {
        cases: 32,
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// For every setting, the resolved value is the highest layer's, flags
    /// over environment over file, and the default when every layer is silent.
    #[test]
    fn the_highest_layer_wins(file in layer(), env in layer(), flags in layer()) {
        let settings = resolved(&file, &env, &flags);

        let listen = flags.listen.or(env.listen).or(file.listen)
            .map_or_else(|| address(8080), address);
        prop_assert_eq!(settings["listen"].as_str(), Some(listen.as_str()));

        let level = flags.level.or(env.level).or(file.level).unwrap_or("info");
        prop_assert_eq!(settings["log"]["level"].as_str(), Some(level));

        let database = flags.database.or(env.database).or(file.database)
            .map_or_else(|| "sqlite://trapperkeeper.db".to_owned(), database_url);
        prop_assert_eq!(settings["database"]["url"].as_str(), Some(database.as_str()));
    }
}
