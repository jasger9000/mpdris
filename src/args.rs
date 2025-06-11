use argh::FromArgs;
use log::LevelFilter;
use std::{net::IpAddr, path::PathBuf};

use crate::util::get_config_path;

/// A client implementing the dbus MPRIS standard for mpd
#[derive(FromArgs)]
#[argh(help_triggers("-h", "--help"))]
pub struct Args {
    /// display version and exit
    #[argh(switch, short = 'v')]
    pub version: bool,
    /// the port over which to connect to mpd
    #[argh(option, short = 'p')]
    pub port: Option<u16>,
    /// the ip address over which to connect to mpd
    #[argh(option, short = 'a')]
    pub addr: Option<IpAddr>,
    /// number of times mpdris tries to reconnect to mpd before exiting. Set to -1 to retry infinite times
    #[argh(option, short = 'r')]
    pub retries: Option<isize>,
    /// the path to the config file to use instead of the computed default
    #[argh(option, default = "get_config_path()")]
    pub config: PathBuf,
    /// the logging level to use. May be one of: trace, debug, info, warn, error
    #[argh(option, default = "log::LevelFilter::Info")]
    pub level: LevelFilter,
    /// when set, will try to fork into a daemon upon launch
    #[argh(switch)]
    pub daemon: bool,
    /// set to act as a systemd service. Acts like a daemon without forking
    #[argh(switch)]
    pub service: bool,
}
