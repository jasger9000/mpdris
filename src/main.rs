use libc::{EXIT_FAILURE, EXIT_SUCCESS, SIGHUP, SIGQUIT};
use once_cell::sync::Lazy;
use std::sync::{atomic::AtomicBool, Arc};
use std::{env, io, process::exit};

use signal_hook::{consts::TERM_SIGNALS, flag, iterator::Signals, low_level::emulate_default_handler};

use crate::args::Args;
use crate::client::MPDClient;
use crate::config::{config, Config, CONFIG};
use util::get_config_path;
use util::notify::{monotonic_time, Systemd};

mod args;
mod client;
mod config;
mod dbus;
mod util;

#[rustfmt::skip]
const VERSION_STR: &str = concat!("Running ", env!("CARGO_BIN_NAME"), " v", env!("CARGO_PKG_VERSION"), " (", env!("GIT_HASH"), ") compiled using rustc v", env!("RUSTC_VERSION"));
static HOME_DIR: Lazy<String> = Lazy::new(|| env::var("HOME").expect("$HOME must be set"));

#[cfg(target_os = "linux")]
#[async_std::main]
async fn main() {
    let args: Args = argh::from_env();

    let config_path = match &args.config {
        Some(c) => c,
        None => &get_config_path(),
    };
    if args.version {
        println!("{}", VERSION_STR);
        exit(EXIT_SUCCESS);
    }

    // subscribe to signals
    let mut signals = {
        get_signals(args.service).unwrap_or_else(|err| {
            eprintln!("Could not subscribe to signals: {err}");
            exit(EXIT_FAILURE);
        })
    };

    {
        let config = Config::load_config(config_path, &args).await.unwrap_or_else(|err| {
            eprintln!("Error occurred while trying to load the config: {err}");
            exit(EXIT_FAILURE);
        });

        if !config_path.is_file() {
            config.write(config_path).await.unwrap_or_else(|err| {
                eprintln!("Could not write config file: {err}");
            });
        }
        CONFIG.set(config.into()).expect("CONFIG should not have been written to");
    }

    // Main app here
    let (conn, recv) = MPDClient::new()
        .await
        .unwrap_or_else(|e| panic!("Could not connect to mpd server: {e}"));
    let conn = Arc::new(conn);

    let _interface = dbus::serve(conn.clone(), recv)
        .await
        .unwrap_or_else(|err| panic!("Could not serve the dbus interface: {err}"));

    let libsystemd = if args.service {
        Some(Systemd::new().expect("failed to load libsystemd"))
    } else {
        None
    };

    if let Some(libsystemd) = &libsystemd {
        libsystemd.notify("READY=1");
    }

    let handle = signals.handle();
    for signal in &mut signals {
        match signal {
            SIGHUP => {
                println!("Received SIGHUP, reloading config");
                if let Some(libsystemd) = &libsystemd {
                    let time = monotonic_time().as_micros();
                    libsystemd.notify(&format!("RELOADING=1\nMONOTONIC_USEC={time}"));
                }

                match Config::load_config(config_path, &args).await {
                    Ok(c) => {
                        *config().write().await = c;

                        conn.reconnect().await.unwrap_or_else(|err| {
                            eprintln!("Could not reconnect to mpd, quitting: {err}");
                            handle.close();
                        });

                        if let Some(libsystemd) = &libsystemd {
                            libsystemd.notify("READY=1");
                        }
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
                eprintln!("Received exit signal, quitting...");
                handle.close();
            }
        }
    }

    if let Some(libsystemd) = &libsystemd {
        libsystemd.notify("STOPPING=1")
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

    Signals::new(sigs)
}
