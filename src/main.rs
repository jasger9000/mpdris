mod config;
mod connection;

use std::{env, fs, io};
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::connection::MpdConnection;

#[rustfmt::skip]
const VERSION_STR: &str = concat!(env!("CARGO_BIN_NAME"), " v", env!("CARGO_PKG_VERSION"), " (", env!("GIT_HASH"), ") compiled using rustc v", env!("RUSTC_VERSION"));

fn main() {
    #[cfg(not(debug_assertions))]
    let config_path: PathBuf = {
        let mut path: PathBuf = match env::var("XDG_CONFIG_HOME") {
            Ok(c) => c,
            Err(_) => env::var("HOME").expect("$HOME must always be set"),
        }
        .parse()
        .expect("Could not parse path to config directory");

        path.join(["mpd", "mpDris.conf"].iter().collect())
    };
    #[cfg(debug_assertions)]
    let config_path: PathBuf = [
        env::var("PWD").expect("$PWD must always be set").as_str(),
        "mpDris.conf",
    ]
    .iter()
    .collect();

    let config = {
        match load_config(config_path.as_path()) {
            Ok(c) => c,
            Err(err) => {
                panic!("Error occurred while trying to read config file! {err}");
            }
        }
    };

    let mut conn = match MpdConnection::init_connection(config.addr, config.port) {
        Ok(c) => c,
        Err(e) => {
            panic!("Could not connect to mpd server: {e}")
        }
    };
}

/// Loads the config file, if $MPD_HOST or $MPD_PORT is defined it will take its values instead of
/// the ones specified in the config as per the MPD client specifications
fn load_config(file: &Path) -> io::Result<Config> {
    let mut config = {
        if !file.exists() {
            Config::new()
        } else {
            let data = fs::read_to_string(file)?;

            match toml::from_str(&data) {
                Ok(config) => config,
                Err(err) => {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, err.message()));
                }
            }
        }
    };

    if let Ok(addr) = env::var("MPD_HOST") {
        config.addr = match addr.parse() {
            Ok(a) => a,
            Err(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Could not parse the $MPD_HOST environment variable into a host address.",
                ))
            }
        }
    }

    if let Ok(port) = env::var("MPD_PORT") {
        config.port = match port.parse() {
            Ok(p) => p,
            Err(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Could not parse the $MPD_PORT environment variable into an integer.",
                ))
            }
        }
    }

    Ok(config)
}
