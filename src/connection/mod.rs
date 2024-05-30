mod error;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use crate::config::Config;
use const_format::concatcp;

pub use self::error::MPDResult as Result;
pub use self::error::*;

/// How many bytes MPD sends at once
const SIZE_LIMIT: usize = 1024;
/// Maximum accepted data from one request_data() call
const MAX_DATA_SIZE: usize = 16_384;

#[derive(Debug)]
pub struct Status {
    pub playing: bool,
    pub volume: u8,
    pub repeat: Repeat,
    pub shuffle: bool,
    /// elapsed time of the current song in sceonds
    pub elapsed: usize,
}

impl Status {
    fn new() -> Self {
        Self {
            playing: false,
            volume: 100,
            repeat: Repeat::OFF,
            shuffle: false,
            elapsed: 0,
        }
    }
}

#[derive(PartialEq, Debug)]
pub enum Repeat {
    OFF = 0,
    ON = 1,
    SINGLE = 2,
}

pub struct MpdConnection {
    connection: TcpStream,
}

impl MpdConnection {
    pub fn request_data(&mut self, request: &str) -> Result<String> {
        let mut data = String::new();
        loop {
            let mut buf = [0; SIZE_LIMIT];

            self.connection.write(format!("{request}\n").as_bytes())?;

            self.connection.read(&mut buf)?;
            let s = std::str::from_utf8(&buf)?;

            data.push_str(s.trim_matches(|c| c == '\0').trim());

            if data.len() > MAX_DATA_SIZE {
                self.empty_connection();

                return Err(Error::new(
                    ErrorKind::DataLimitExceeded,
                    concatcp!(
                        "Data buffer has overflown (Max size ",
                        MAX_DATA_SIZE,
                        " bytes)"
                    ),
                ));
            }

            if buf[SIZE_LIMIT - 1] == 0 {
                // buffer not filled e.g. everything is read
                break;
            }
        }

        if data.ends_with("OK") {
            return Ok(data);
        }

        Err(Error::try_from_mpd(data)?)
    }

    /// Empty out all bytes remaining in the input of the connection
    fn empty_connection(&mut self) {
        let mut buf = [0; SIZE_LIMIT];

        while let Ok(read_amount) = self.connection.read(&mut buf) {
            if read_amount == 0 {
                break;
            }
        }
    }

    pub fn play(&mut self) -> Result<()> {
        let _ = self.request_data("pause 0")?;

        Ok(())
    }

    /// Seek to a position in the current song with offset in seconds
    /// To seek relative to the current position use [Self::seek_relative]
    pub fn seek(&mut self, offset: usize) -> Result<()> {
        let _ = self.request_data(&format!("seekcur {offset}"))?;

        Ok(())
    }

    /// Seek to a position in the current song relative to the current position with offset in
    /// seconds
    /// To seek from the songs begin (absolute) use [Self::seek]
    pub fn seek_relative(&mut self, offset: isize) -> Result<()> {
        let offset: String = if offset > 0 {
            format!("+{offset}")
        } else {
            offset.to_string()
        };

        let _ = self.request_data(&format!("seekcur {offset}"))?;

        Ok(())
    }

    /// Pause playback
    pub fn pause(&mut self) -> Result<()> {
        let _ = self.request_data("pause 1")?;

        Ok(())
    }

    pub fn toggle_play(&mut self) -> Result<()> {
        let is_playing = self.get_status()?.playing;

        if is_playing {
            return self.pause();
        } else {
            return self.play();
        }
    }

    pub fn get_status(&mut self) -> Result<Status> {
        let res = self.request_data("status")?;

        let mut status = Status::new();

        for line in res.lines() {
            let mut parts = line.split(": ");

            if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                match k {
                    "state" => status.playing = v.contains("play"),
                    "single" => {
                        if v.parse().unwrap_or(0) > 0 {
                            status.repeat = Repeat::SINGLE;
                        }
                    }
                    "repeat" => {
                        if v.parse().unwrap_or(0) > 0 && status.repeat == Repeat::OFF {
                            status.repeat = Repeat::ON;
                        }
                    }
                    "volume" => status.volume = v.parse().unwrap_or(0),
                    "random" => status.shuffle = v.parse().unwrap_or(0) > 0,
                    "elapsed" => status.elapsed = v.parse().unwrap_or(0),
                    &_ => {}
                }
            } else if line == "OK" {
                break;
            } else {
                eprintln!("Expected {{k}}: {{v}} but got `{line}`.");
                eprintln!("Could not split line into key-value pair");
            }
        }

        Ok(status)
    }

    pub fn init_connection(config: &Config) -> Result<Self> {
        println!(
            "Connecting to server on ip-address: {} using port: {}",
            config.addr, config.port
        );

        let stream = {
            let mut attempts = 0;
            let timeout = if config.timeout > 0 {
                Some(Duration::from_secs(config.timeout as u64))
            } else {
                None
            };
            let addr = &SocketAddr::new(config.addr, config.port);

            loop {
                let stream = if let Some(t) = timeout {
                    TcpStream::connect_timeout(addr, t)
                } else {
                    TcpStream::connect(addr)
                };

                match stream {
                    Ok(stream) => {
                        stream.set_read_timeout(timeout).unwrap(); // Cannot error out because Duration cannot be zero
                        stream.set_write_timeout(timeout).unwrap();
                        break stream;
                    }
                    Err(err) => {
                        if config.retries > 0 {
                            eprintln!(
                                "Could not connect (tries left {}): {err}",
                                config.retries - attempts
                            );

                            attempts += 1;
                            if attempts > config.retries {
                                return Err(err.into());
                            }
                        } else {
                            eprintln!("Could not connect: {err}");
                        }
                    }
                }
            }
        };

        let mut conn = Self { connection: stream };

        {
            println!("Validating connection");
            let mut buf = [0; 1024];

            conn.connection.read(&mut buf)?;
            let res = std::str::from_utf8(&buf)?;

            if !res.starts_with("OK MPD") {
                return Err(Error::new_string(ErrorKind::InvalidConnection,
                    format!("Could not validate connection. Excepted `OK MPD {{VERSION}}` from server but got `{res}`"),
                ));
            }
        }
        {
            println!("Setting binary output limit to {SIZE_LIMIT} bytes");
            let res = conn.request_data(concatcp!("binarylimit ", SIZE_LIMIT))?;

            if res != "OK" {
                return Err(Error::new_string(
                    ErrorKind::InvalidConnection,
                    format!(
                        "Could not validate connection. Excepted `OK` from server but got `{res}`"
                    ),
                ));
            }
        }

        Ok(conn)
    }
}
