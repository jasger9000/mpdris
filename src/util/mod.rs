use std::{env, io, path::PathBuf, process::exit};

use crate::HOME_DIR;

use libc::EXIT_SUCCESS;
use log::{debug, warn};

pub mod expand;
pub mod notify;

/// Gets the default config path from the environment.
/// Defined as: $XDG_CONFIG_PATH/mpdris/mpdris.conf or $HOME/.config/mpdris/mpdris.conf
/// Deprecated path: $XDG_CONFIG_PATH/mpd/mpDris.conf or $HOME/.config/mpd/mpDris.conf
pub fn get_config_path() -> PathBuf {
    let base = env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{}/.config", *HOME_DIR));
    let paths: [PathBuf; 2] = [
        [base.as_str(), "mpdris", "mpdris.conf"].iter().collect(),
        [base.as_str(), "mpd", "mpDris.conf"].iter().collect(),
    ];
    let idx = paths.iter().position(|p| p.is_file()).unwrap_or(0);
    if idx == 1 {
        warn!("Using deprecated configuration path `{}`", paths[idx].display());
    }
    paths.into_iter().nth(idx).unwrap()
}

pub fn init_logger(level: log::LevelFilter) {
    use simplelog::format_description;

    let logconf = simplelog::ConfigBuilder::new()
        .set_target_level(log::LevelFilter::Error)
        .set_time_format_custom(format_description!("[year]-[month]-[day] [hour]:[minute]:[second]"))
        .set_time_offset_to_local()
        .expect("failed to get UTC offset")
        .build();

    simplelog::TermLogger::init(level, logconf, simplelog::TerminalMode::Mixed, simplelog::ColorChoice::Auto)
        .expect("failed to set logger");
}

/// Sends a signal to the specified PID, uses libc::kill as the underlying implementation
///
/// For more information see the libc documentation for kill
///
/// # Errors:
/// [InvalidInput](io::ErrorKind::InvalidInput): An invalid signal was specified.<br />
/// [PermissionDenied](io::ErrorKind::PermissionDenied): The calling process does not have
/// permission to send the signal to any of the target processes.<br />
/// [Uncategorized](io::ErrorKind::Uncategorized): The target process or process group does not exist.
pub fn send_sig(pid: u32, signal: i32) -> io::Result<()> {
    unsafe {
        if libc::kill(pid as i32, signal) != 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

/// Forks the currently running process, kills the parent,
/// closes all file descriptors and sets the working directory to /
pub fn daemonize() {
    use libc::{STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO};
    use std::cmp::Ordering;
    debug!("daemonizing");

    unsafe {
        let pid = libc::fork();
        match pid.cmp(&0) {
            Ordering::Less => panic!("Failed to fork the process"),
            Ordering::Equal => {} // child process
            Ordering::Greater => exit(EXIT_SUCCESS),
        }

        if libc::setsid() < 0 {
            panic!("Failed to create a new session for the daemon");
        }

        if libc::chdir(c"/".as_ptr()) < 0 {
            panic!("Failed to change path to root directory");
        }

        if libc::close(STDIN_FILENO) < 0 || libc::close(STDOUT_FILENO) < 0 || libc::close(STDERR_FILENO) < 0 {
            panic!("Failed to close one of the file descriptors stdin, stdout, stderr");
        }
    }
}
