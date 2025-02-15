use std::{env, io, path::PathBuf};

use crate::HOME_DIR;

pub mod expand;
pub mod notify;

/// Gets the default config path from the environment.
/// Defined as: $XDG_CONFIG_PATH/mpd/mpDris.conf or $HOME/.config/mpd/mpDris.conf
pub fn get_config_path() -> PathBuf {
    let base = env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{}/.config", *HOME_DIR));
    [base.as_str(), "mpd", "mpDris.conf"].iter().collect()
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
