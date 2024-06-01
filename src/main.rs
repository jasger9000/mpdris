mod config;
mod connection;

use async_std::sync::Mutex;
use clap::{arg, value_parser, Command};
use libc::{
    EXIT_FAILURE, EXIT_SUCCESS, SIGHUP, SIGQUIT, STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO,
};
use std::cmp::Ordering;
use std::env;
use std::ffi::CString;
use std::io;
use std::path::PathBuf;
use std::process::exit;
use std::sync::{atomic::AtomicBool, Arc};

use signal_hook::consts::TERM_SIGNALS;
use signal_hook::flag;
use signal_hook::iterator::Signals;
use signal_hook::low_level::emulate_default_handler;

use crate::config::Config;
use crate::connection::MpdConnection;

#[rustfmt::skip]
const VERSION_STR: &str = concat!("v", env!("CARGO_PKG_VERSION"), " (", env!("GIT_HASH"), ") compiled using rustc v", env!("RUSTC_VERSION"));

#[cfg(target_os = "linux")]
#[async_std::main]
async fn main() {
    let config_path = get_config_path();

    #[rustfmt::skip] // this gets really messy when formatted as multiline
    let matches = Command::new(env!("CARGO_BIN_NAME"))
        .version(VERSION_STR)
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(arg!(-p --port <PORT> "The port over which to connect to mpd").value_parser(value_parser!(u16)))
        .arg(arg!(-a --addr <ADDRESS> "the ip address over which to connect to mpd"))
        .arg(arg!(--retries <AMOUNT> "Amount of times mpDris retries to connect to mpd before exiting. Set to -1 to retry infinite times").value_parser(value_parser!(isize)))
        .arg(arg!(--"no-spawn-daemon" "When set does not try to fork into a daemon"))
        .arg(arg!(--systemd "When set acts as a daemon without forking the process"))
        .get_matches();

    // subscribe to signals
    let mut signals = {
        // decide wether or not we should fork
        let is_daemon = !matches.get_flag("no-spawn-daemon") || matches.get_flag("systemd");
        let should_fork = !matches.get_flag("no-spawn-daemon") && !matches.get_flag("systemd");

        if should_fork {
            daemonize();
        }

        get_signals(is_daemon).unwrap_or_else(|err| {
            eprintln!("Could not subscribe to signals: {err}");
            exit(EXIT_FAILURE);
        })
    };

    let config = {
        let config = Config::load_config(config_path.as_path(), &matches)
            .await
            .unwrap_or_else(|err| {
                eprintln!("Error occurred while trying to load the config: {err}");
                exit(EXIT_FAILURE);
            });

        if !config_path.is_file() {
            config.write(&config_path).await.unwrap_or_else(|err| {
                eprintln!("Could not write config file: {err}");
            });
        }
        Arc::new(Mutex::new(config))
    };

    // Main app here

    let conn = Arc::new(Mutex::new(
        MpdConnection::new(config.clone())
            .await
            .unwrap_or_else(|e| panic!("Could not connect to mpd server: {e}")),
    ));

    let handle = signals.handle();
    for signal in &mut signals {
        match signal {
            SIGHUP => {
                todo!("Implement config reloading");
            }
            SIGQUIT => {
                eprintln!("Received SIGQUIT, dumping core...");
                handle.close();
                emulate_default_handler(SIGQUIT).unwrap_or_else(|err| {
                    eprintln!("Failed to dump core: {err}");
                    exit(EXIT_FAILURE);
                });
            }
            _ => {
                eprintln!("Received signal, quitting...");
                handle.close();
            }
        }
    }
}

/// Forks the currently running process, kills the parent, closes all filedescriptors and sets the working directory to /
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

/// Subscribes to exit signals
/// If is_daemon is true will add SIGHUP signal to returned Signals
fn get_signals(is_daemon: bool) -> io::Result<Signals> {
    let kill_now = Arc::new(AtomicBool::new(false));

    for sig in TERM_SIGNALS {
        // kill application when flag is set & signal is received
        flag::register_conditional_shutdown(*sig, EXIT_FAILURE, Arc::clone(&kill_now))?;
        // Sets signal after it is already handled by line above -> will instantly kill when singal is received twice
        flag::register(*sig, Arc::clone(&kill_now))?;
    }

    let mut sigs: Vec<_> = TERM_SIGNALS.iter().collect();

    // subscribe to extra signals
    if is_daemon {
        sigs.push(&SIGHUP);
    }

    Ok(Signals::new(sigs)?)
}

#[cfg(not(debug_assertions))]
fn get_config_path() -> PathBuf {
    let path: PathBuf = match env::var("XDG_CONFIG_HOME") {
        Ok(p) => p,
        Err(_) => env::var("HOME").expect("$HOME must always be set"),
    }
    .parse()
    .expect("Could not parse path to config directory");

    path.join(["mpd", "mpDris.conf"].iter().collect::<PathBuf>())
}
#[cfg(debug_assertions)]
fn get_config_path() -> PathBuf {
    [
        env::var("PWD").expect("$PWD must always be set").as_str(),
        "mpDris.conf",
    ]
    .iter()
    .collect()
}
