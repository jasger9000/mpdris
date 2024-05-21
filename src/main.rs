mod config;
mod connection;

use clap::{arg, value_parser, Command};
use libc::{
    sighandler_t, signal, EXIT_SUCCESS, SIGHUP, SIGTERM,
    SIG_ERR, STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO,
};
use std::cmp::Ordering;
use std::env;
use std::ffi::CString;
use std::net::IpAddr;
use std::path::PathBuf;
use std::process::exit;

use crate::config::Config;
use crate::connection::MpdConnection;

#[rustfmt::skip]
const VERSION_STR: &str = concat!("v", env!("CARGO_PKG_VERSION"), " (", env!("GIT_HASH"), ") compiled using rustc v", env!("RUSTC_VERSION"));

#[cfg(target_os = "linux")]
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
        .arg(arg!(--retries <AMOUNT> "Amount of times mpDris retries to connect to mpd before exiting. Set to -1 to retry infinite times").value_parser(value_parser!(isize)))
        .arg(arg!(--timeout <SECONDS> "Amount of seconds mpDris waits for MPD to respond. Set to -1 to wait indefinetly").value_parser(value_parser!(isize)))
        .arg(arg!(--"no-spawn-daemon" "When set does not try to fork into a daemon"))
        .arg(arg!(--systemd "When set acts as a daemon without forking the process"))
        .get_matches();

    {
        let is_daemon = !matches.get_flag("no-spawn-daemon") || matches.get_flag("systemd");
        let should_fork = !matches.get_flag("no-spawn-daemon");

        if should_fork {
            daemonize();
        }
    }

    let mut config = {
        match Config::load_config(config_path.as_path()) {
            Ok(c) => c,
            Err(err) => {
                panic!("Error occurred while trying to read config file! {err}");
            }
        }
    };

    if !config_path.is_file() {
        match config.write(&config_path) {
            Ok(_) => {},
            Err(err) => eprintln!("Could not write config file: {err}"),
        }
    }

    if let Some(port) = matches.get_one::<u16>("port") { config.port = *port; }
    if let Some(addr) = matches.get_one::<IpAddr>("addr") { config.addr = *addr; }
    if let Some(retries) = matches.get_one::<isize>("retries") { config.retries = *retries; }
    if let Some(timeout) = matches.get_one::<isize>("timeout") { config.timeout = *timeout; }

    let mut conn = MpdConnection::init_connection(&config)
        .unwrap_or_else(|e| panic!("Could not connect to mpd server: {e}"));
}

fn daemonize() {
    let pid = unsafe { libc::fork() };
    match pid.cmp(&0) {
        Ordering::Less => {
            panic!("Could not fork the process")
        }
        Ordering::Equal => {} // child process
        Ordering::Greater => exit(EXIT_SUCCESS),
    }

    let sid = unsafe { libc::setsid() };
    if sid < 0 {
        panic!("Could not create a new session for the daemon");
    }

    let root_dir = CString::new("/").expect("Root path descriptor creation failed");
    if unsafe { libc::chdir(root_dir.as_ptr()) } < 0 {
        panic!("Could not change to root directory");
    }

    unsafe {
        if libc::close(STDIN_FILENO) < 0
            || libc::close(STDOUT_FILENO) < 0
            || libc::close(STDERR_FILENO) < 0
        {
            panic!("Could not filedescriptors stdin, stdout, stderr");
        }
    }
}
