use async_std::{fs, io, sync::RwLock};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

use std::net::{IpAddr, Ipv4Addr};
use std::{env, path::Path};

use crate::args::Args;
use crate::expand::serde_expand_path;
use crate::HOME_DIR;

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
    pub music_directory: Box<str>,
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

const DEFAULT_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
const DEFAULT_PORT: u16 = 6600;
const DEFAULT_RETRIES: isize = 3;

pub static CONFIG: OnceCell<RwLock<Config>> = OnceCell::new();

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
        println!("Writing config file to `{}`", file.to_string_lossy());
        if !file
            .parent()
            .ok_or(io::Error::new(io::ErrorKind::InvalidInput, "Path invalid"))?
            .exists()
        {
            eprintln!("Could not find parent dir, Creating...");

            // Why not `create_dir_all`? Because if $HOME/.config does not exist, there's something majorly wrong with the user I don't want to handle
            fs::create_dir(file.parent().unwrap()).await?;
        }

        let data = match toml::to_string_pretty(self) {
            Ok(d) => d,
            Err(err) => return Err(io::Error::new(io::ErrorKind::InvalidData, err.to_string())),
        };

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
            eprintln!("Could not find config file, using default values instead");
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
            self.addr = match addr.parse() {
                Ok(a) => a,
                Err(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Could not parse the $MPD_HOST environment variable into a host address.",
                    ))
                }
            }
        }

        if let Ok(port) = env::var("MPD_PORT") {
            self.port = match port.parse() {
                Ok(p) => p,
                Err(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Could not parse the $MPD_PORT environment variable into an integer.",
                    ))
                }
            }
        }

        Ok(())
    }

    /// Loads config from file
    async fn load_from_file(file: &Path) -> io::Result<Self> {
        let data = fs::read_to_string(file).await?;

        match toml::from_str(&data) {
            Ok(config) => Ok(config),
            Err(err) => {
                return Err(io::Error::new(io::ErrorKind::InvalidData, err.message()));
            }
        }
    }
}

fn default_music_dir() -> Box<str> {
    let dir = format!("{}/Music", *HOME_DIR).into_boxed_str();
    eprintln!("Missing value `music_directory` in config, using default: {dir}");
    dir
}
fn default_addr() -> IpAddr {
    eprintln!("Missing value `addr` in config, using default: {DEFAULT_ADDR}");
    DEFAULT_ADDR
}
fn default_port() -> u16 {
    eprintln!("Missing value `port` in config, using default: {DEFAULT_PORT}");
    DEFAULT_PORT
}
fn default_retries() -> isize {
    eprintln!("Missing value `retries` in config, using default: {DEFAULT_RETRIES}");
    DEFAULT_RETRIES
}
