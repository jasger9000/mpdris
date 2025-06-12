use async_std::{fs, io, sync::RwLock};
use log::{info, warn};
use serde::{Deserialize, Serialize};

use std::net::{IpAddr, Ipv4Addr};
use std::{env, path::Path, path::PathBuf, sync::OnceLock};

use crate::HOME_DIR;
use crate::args::Args;
use crate::util::expand::serde_expand_path;
use dns_lookup::lookup_host;

#[derive(Deserialize, Serialize, Debug)]
pub struct Config {
    #[serde(default = "default_addr")]
    /// The IP address of MPD to connect to
    pub addr: IpAddr,
    #[serde(default = "default_port")]
    /// The port of MPD to connect to
    pub port: u16,
    #[serde(default = "default_retries")]
    /// Amount of time to retry to connect
    pub retries: isize,
    #[serde(default = "default_music_dir")]
    #[serde(deserialize_with = "serde_expand_path")]
    /// The root directory MPD uses to play music
    pub music_directory: PathBuf,
    #[serde(default = "default_cover_dir")]
    #[serde(deserialize_with = "serde_expand_path")]
    /// The dedicated root directory mpdris uses to search for covers
    pub cover_directory: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

const DEFAULT_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
const DEFAULT_PORT: u16 = 6600;
const DEFAULT_RETRIES: isize = 3;

pub static CONFIG: OnceLock<RwLock<Config>> = OnceLock::new();

/// Returns a refrence to the global CONFIG value.
///
/// Panics when the config was not assigned
pub fn config() -> &'static RwLock<Config> {
    CONFIG.get().expect("Config should always be assigned")
}

impl Config {
    pub fn new() -> Self {
        Self {
            addr: DEFAULT_ADDR,
            port: DEFAULT_PORT,
            retries: DEFAULT_RETRIES,
            music_directory: default_music_dir(),
            cover_directory: default_cover_dir(),
        }
    }

    /// Writes the loaded config to the specified path. Returns a future that completes when all data is written.
    /// This function will create the parent directory of the file if it does not exist
    ///
    /// # Errors
    /// The function will return the error variant in the following situations:
    /// - InvalidInput when an invalid path is passed in
    /// - InvalidData when the config could not be serialized (should never occur)
    /// - NotFound if the parent of the parent dir does not exist
    /// - PermissionDenied if the process lacks the permission to write to the directory/file
    /// - Some other I/O error further specified in [fs::create_dir] or [fs::write]
    pub async fn write(&self, file: &Path) -> io::Result<()> {
        info!("Writing config file to `{}`", file.to_string_lossy());
        if !file
            .parent()
            .ok_or(io::Error::new(io::ErrorKind::InvalidInput, "Path invalid"))?
            .exists()
        {
            warn!("Could not find parent dir, Creating...");

            // Why not `create_dir_all`? Because if $HOME/.config does not exist, there's something majorly wrong with the user I don't want to handle
            fs::create_dir(file.parent().unwrap()).await?;
        }

        let data = toml::to_string_pretty(self).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        fs::write(file, data).await?;

        Ok(())
    }

    /// Loads the config file.
    ///
    /// ## Behaviour
    /// - If the file does not exist, it will use the standard config instead.
    /// - If a value is missing from the config, it will warn the user and use the default value.
    /// - If the `$MPD_HOST` or `$MPD_PORT` environment variable is defined,
    ///   it will take its values instead of the ones specified in the config as per
    ///   the [MPD client specifications](https://mpd.readthedocs.io/en/stable/client.html#connecting-to-mpd)
    /// - If an argument is specified it will use the value from the argument
    ///
    /// ## Errors
    /// - PermissionDenied if the process lacks the permissions to read the file
    /// - InvalidData if the file read contains invalid UTF-8
    /// - InvalidData if the file cannot be deserialized into a config
    /// - Some other I/O error further specified in [fs::read_to_string]
    pub async fn load_config(file: &Path, args: &Args) -> io::Result<Self> {
        let mut config = if file.exists() {
            Self::load_from_file(file).await?
        } else {
            warn!("Could not find config file, using default values instead");
            Self::new()
        };

        config.load_from_env_vars()?;

        config.load_from_args(args);

        Ok(config)
    }

    fn load_from_args(&mut self, args: &Args) {
        if let Some(port) = args.port {
            self.port = port;
        }
        if let Some(addr) = args.addr {
            self.addr = addr;
        }
        if let Some(retries) = args.retries {
            self.retries = retries;
        }
    }

    /// Loads values $MPD_HOST and $MPD_PORT from environment
    fn load_from_env_vars(&mut self) -> io::Result<()> {
        if let Ok(addr) = env::var("MPD_HOST") {
            self.addr = lookup_host(addr.as_str())
                .map_err(|_e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Could not resolve the $MPD_HOST environment variable into an IP address.",
                    )
                })?
                .pop()
                .ok_or(io::Error::new(io::ErrorKind::InvalidData, "Could not resolve $MPD_HOST"))?;
        }

        if let Ok(port) = env::var("MPD_PORT") {
            self.port = port.parse().map_err(|_e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Could not parse the $MPD_PORT environment variable into an integer.",
                )
            })?;
        }

        Ok(())
    }

    /// Loads config from file
    async fn load_from_file(file: &Path) -> io::Result<Self> {
        let data = fs::read_to_string(file).await?;

        toml::from_str::<Config>(&data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.message()))
    }
}

fn default_music_dir() -> PathBuf {
    [&HOME_DIR, "Music"].iter().collect()
}
fn default_cover_dir() -> PathBuf {
    [&HOME_DIR, "Music", "covers"].iter().collect()
}
fn default_addr() -> IpAddr {
    DEFAULT_ADDR
}
fn default_port() -> u16 {
    DEFAULT_PORT
}
fn default_retries() -> isize {
    DEFAULT_RETRIES
}
