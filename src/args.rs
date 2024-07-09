use argh::FromArgs;
use std::net::IpAddr;

/// A client implementing the dbus MPRIS standard for mpd
#[derive(FromArgs)]
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
    /// amount of times mpDris tries to reconnect to mpd before exiting. Set to -1 to retry inifinite times
    #[argh(option, short = 'r')]
    pub retries: Option<isize>,
    /// when set, does not try to fork into a daemon
    #[argh(switch)]
    pub no_spawn_daemon: bool,
    /// when set, acts as a daemon without forking the process
    #[argh(switch)]
    pub service: bool,
}
