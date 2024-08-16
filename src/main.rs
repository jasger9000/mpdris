mod args;
mod config;
mod connection;
mod dbus;

use async_std::sync::Mutex;
use libc::{EXIT_FAILURE, EXIT_SUCCESS, SIGHUP, SIGQUIT};
use once_cell::sync::Lazy;
use std::sync::{atomic::AtomicBool, Arc};
use std::{env, io, path::PathBuf, process::exit};

use signal_hook::{consts::TERM_SIGNALS, flag, iterator::Signals, low_level::emulate_default_handler};

use crate::args::Args;
use crate::config::Config;
use crate::connection::MpdClient;

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

    let config = {
        let config = Config::load_config(config_path, &args).await.unwrap_or_else(|err| {
            eprintln!("Error occurred while trying to load the config: {err}");
            exit(EXIT_FAILURE);
        });

        if !config_path.is_file() {
            config.write(config_path).await.unwrap_or_else(|err| {
                eprintln!("Could not write config file: {err}");
            });
        }
        Arc::new(Mutex::new(config))
    };

    // Main app here
    let (conn, recv) = MpdClient::new(config.clone())
        .await
        .unwrap_or_else(|e| panic!("Could not connect to mpd server: {e}"));
    let conn = Arc::new(conn);

    let _interface = dbus::serve(conn.clone(), recv, config.clone())
        .await
        .unwrap_or_else(|err| panic!("Could not serve the dbus interface: {err}"));

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

/// Gets the default config path from the enviroment.
/// Defined as: $XDG_CONFIG_PATH/mpd/mpDris.conf or $HOME/.config/mpd/mpDris.conf
fn get_config_path() -> PathBuf {
    let base = env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{}/.config", *HOME_DIR));
    [base.as_str(), "mpd", "mpDris.conf"].iter().collect()
}
