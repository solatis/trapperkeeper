use std::env;

use lazy_static::lazy_static;
use serde::Deserialize;

use config;

lazy_static! {
    pub static ref CONFIG: Config = Config::new().expect("Unable to read configuration");
}

#[derive(Clone, Debug, Deserialize)]
pub struct Database {
    pub url: String,
    pub pool_size: u32,
}

impl Default for Database {
    fn default() -> Self {
        Self {
            url: String::from("sqlite://./tk.sqlite"),
            pool_size: 8,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Api {
    pub addr: String,
    pub port: u16,
}

impl Default for Api {
    fn default() -> Self {
        Self {
            addr: String::from("127.0.0.1"),
            port: 8080,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub debug: bool,
    pub database: Database,
    pub api: Api,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            debug: false,
            database: Database::default(),
            api: Api::default(),
        }
    }
}

impl Config {
    pub fn new() -> Result<Self, config::ConfigError> {
        let cfg_file: String = match env::var("TK_CONFIG_FILE") {
            Ok(fname) => fname,
            _ => String::from("default.yaml"),
        };

        let s = config::Config::builder()
            .set_default("debug", false)?
            .add_source(config::File::with_name(cfg_file.as_str()))
            .add_source(config::Environment::with_prefix("tk"))
            .build()?;

        s.try_deserialize()
    }
}
