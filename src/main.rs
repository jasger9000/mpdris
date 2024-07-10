mod args;
mod config;
mod connection;

use async_std::sync::Mutex;
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

use crate::args::Args;
use crate::config::Config;
use crate::connection::MpdClient;

#[rustfmt::skip]
const VERSION_STR: &str = concat!("Running ", env!("CARGO_BIN_NAME"), " v", env!("CARGO_PKG_VERSION"), " (", env!("GIT_HASH"), ") compiled using rustc v", env!("RUSTC_VERSION"));

#[cfg(target_os = "linux")]
#[async_std::main]
async fn main() {
    let config_path = get_config_path();
    let args: Args = argh::from_env();
    if args.version {
        println!("{}", VERSION_STR);
        exit(EXIT_SUCCESS);
    }

    // subscribe to signals
    let mut signals = {
        // decide whether, we should fork
        let is_daemon = !args.no_spawn_daemon || args.service;
        let should_fork = !args.no_spawn_daemon && !args.service;

        if should_fork {
            daemonize();
        }

        get_signals(is_daemon).unwrap_or_else(|err| {
            eprintln!("Could not subscribe to signals: {err}");
            exit(EXIT_FAILURE);
        })
    };

    let config = {
        let config = Config::load_config(config_path.as_path(), &args)
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
    let mut conn = Arc::new(
        MpdClient::new(config.clone())
            .await
            .unwrap_or_else(|e| panic!("Could not connect to mpd server: {e}")),
    );

    let handle = signals.handle();
    for signal in &mut signals {
        match signal {
            SIGHUP => {
                println!("Received SIGHUP, reloading config");
                match Config::load_config(&config_path, &args).await {
                    Ok(c) => {
                        *config.lock().await = c;

                        conn.reconnect().await.unwrap_or_else(|err| {
                            eprintln!("Could not reconnect to mpd: {err}");
                            eprintln!("Exiting...");
                            exit(EXIT_FAILURE);
                        });

                        println!("Reload complete!");
                    }
                    Err(err) => {
                        eprintln!("Could not load config file, continuing with old one: {err}");
                    }
                }
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

/// Forks the currently running process, kills the parent,
/// closes all file descriptors and sets the working directory to /
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
            panic!("Could not file descriptors stdin, stdout, stderr");
        }
    }
}

/// Subscribes to exit signals
/// If is_daemon is true will add SIGHUP signal to returned Signals
fn get_signals(is_daemon: bool) -> io::Result<Signals> {
    let kill_now = Arc::new(AtomicBool::new(false));

    for sig in TERM_SIGNALS {
        // kill application when a flag is set & signal is received
        flag::register_conditional_shutdown(*sig, EXIT_FAILURE, Arc::clone(&kill_now))?;
        // Set signal after it is already handled by the line above -> will instantly kill
        // when signal is received twice
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
