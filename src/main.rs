mod config;
mod connection;

use std::env;
use std::net::IpAddr;
use std::path::PathBuf;
use clap::{arg, Command, value_parser};

use crate::config::Config;
use crate::connection::MpdConnection;

#[rustfmt::skip]
const VERSION_STR: &str = concat!("v", env!("CARGO_PKG_VERSION"), " (", env!("GIT_HASH"), ") compiled using rustc v", env!("RUSTC_VERSION"));

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

    #[rustfmt::skip] // this gets really messy when formatted as multiline
    let matches = Command::new(env!("CARGO_BIN_NAME"))
        .version(VERSION_STR)
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(arg!(-p --port <PORT> "The port over which to connect to mpd").value_parser(value_parser!(u16)))
        .arg(arg!(-a --addr <ADDRESS> "the ip address over which to connect to mpd"))
        .arg(arg!(--"no-spawn-daemon" "When set does not try to fork into a daemon"))
        .get_matches();

    let mut config = {
        match Config::load_config(config_path.as_path()) {
            Ok(c) => c,
            Err(err) => {
                panic!("Error occurred while trying to read config file! {err}");
            }
        }
    };

    if let Some(port) = matches.get_one::<u16>("port") { config.port = *port; }
    if let Some(addr) = matches.get_one::<IpAddr>("addr") { config.addr = *addr; }
    let spawn_daemon = !matches.get_flag("no-spawn-daemon");

    let mut conn = match MpdConnection::init_connection(config.addr, config.port) {
        Ok(c) => c,
        Err(e) => {
            panic!("Could not connect to mpd server: {e}")
        }
    };
}
