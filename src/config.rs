use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::{env, fs, io};

#[derive(Deserialize, Serialize)]
pub struct Config {
    #[serde(default = "default_addr")]
    pub addr: IpAddr,
    #[serde(default = "default_port")]
    pub port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

impl Config {
    pub fn new() -> Self {
        Self {
            addr: default_addr(),
            port: default_port(),
        }
    }

    pub fn write(&self, file: &Path) -> io::Result<()> {
        if !file
            .parent()
            .ok_or(io::Error::new(io::ErrorKind::InvalidInput, "Path invalid"))?
            .exists()
        {
            eprintln!("Could not find parent dir, Creating...");

            // Why not `create_dir_all`? Because if $HOME/.config does not exist, there's something majorly wrong with the user I dont want to handle
            fs::create_dir(file.parent().unwrap())?;
        }

        let data = match toml::to_string_pretty(self) {
            Ok(d) => d,
            Err(err) => return Err(io::Error::new(ErrorKind::InvalidData, err.to_string()))
        };

        eprintln!(
            "Writing config file to `{}`",
            file.to_str()
                .ok_or(io::Error::new(io::ErrorKind::InvalidInput, "Path invalid"))?
        );
        fs::write(file, data)?;

        Ok(())
    }

    /// Loads the config file, if $MPD_HOST or $MPD_PORT is defined it will take its values instead of
    /// the ones specified in the config as per the MPD client specifications
    pub fn load_config(file: &Path) -> io::Result<Self> {
        let mut config = {
            if !file.exists() {
                eprintln!("Could not find config file. Using default values instead");
                Self::new()
            } else {
                let data = fs::read_to_string(file)?;

                match toml::from_str(&data) {
                    Ok(config) => config,
                    Err(err) => {
                        return Err(io::Error::new(io::ErrorKind::InvalidData, err.message()));
                    }
                }
            }
        };

        if let Ok(addr) = env::var("MPD_HOST") {
            config.addr =
                match addr.parse() {
                    Ok(a) => a,
                    Err(_) => return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Could not parse the $MPD_HOST environment variable into a host address.",
                    )),
                }
        }

        if let Ok(port) = env::var("MPD_PORT") {
            config.port = match port.parse() {
                Ok(p) => p,
                Err(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Could not parse the $MPD_PORT environment variable into an integer.",
                    ))
                }
            }
        }

        Ok(config)
    }
}

fn default_addr() -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))
}
fn default_port() -> u16 {
    6600
}
