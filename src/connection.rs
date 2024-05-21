use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use crate::config::Config;
use const_format::concatcp;

/// How many bytes MPD sends at once
const SIZE_LIMIT: usize = 1024;
/// Maximum accepted data from one request_data() call
const MAX_DATA_SIZE: usize = 16_384;

pub struct Status {
    pub playing: bool,
    pub volume: u8,
    pub repeat: Repeat,
    pub shuffle: bool,
}

impl Status {
    fn new() -> Self {
        Self {
            playing: false,
            volume: 100,
            repeat: Repeat::OFF,
            shuffle: false,
        }
    }
}

#[derive(PartialEq)]
pub enum Repeat {
    OFF = 0,
    ON = 1,
    SINGLE = 2,
}

pub struct MpdConnection {
    connection: TcpStream,
}

impl MpdConnection {
    pub fn request_data(&mut self, request: &str) -> io::Result<String> {
        let mut data = String::new();
        loop {
            let mut buf = [0; SIZE_LIMIT];

            self.empty_connection();
            self.connection.write(format!("{request}\n").as_bytes())?;

            self.connection.read(&mut buf)?;
            let s = match std::str::from_utf8(&buf) {
                Ok(s) => s,
                Err(err) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Could not read response into UTF-8: {err}"),
                    ))
                }
            };

            data.push_str(s.trim_matches(|c| c == '\0').trim());

            if data.len() > MAX_DATA_SIZE {
                self.empty_connection();

                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Data size limit exceeded",
                ));
            }

            if buf[SIZE_LIMIT - 1] == 0 {
                // buffer not filled e.g. everything is read
                break;
            }
        }

        Ok(data)
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

    pub fn play(&mut self) -> io::Result<()> {
        return match self.request_data("play") {
            Ok(s) => {
                if s == "OK" {
                    Ok(())
                } else {
                    Err(io::Error::new(io::ErrorKind::Other, "Could not play: {s}"))
                }
            }
            Err(err) => Err(err),
        };
    }

    pub fn pause(&mut self) -> io::Result<()> {
        return match self.request_data("pause") {
            Ok(s) => {
                if s == "OK" {
                    Ok(())
                } else {
                    Err(io::Error::new(io::ErrorKind::Other, "Could not pause: {s}"))
                }
            }
            Err(err) => Err(err),
        };
    }

    pub fn toggle_play(&mut self) -> io::Result<()> {
        let is_playing = self.get_status()?.playing;

        if is_playing {
            return self.pause();
        } else {
            return self.play();
        }
    }

    pub fn get_status(&mut self) -> io::Result<Status> {
        let res = self.request_data("status").unwrap_or(String::new());

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

    pub fn init_connection(config: &Config) -> io::Result<Self> {
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
                            println!(
                                "Could not connect (tries left {}): {err}",
                                config.retries - attempts
                            );

                            attempts += 1;
                            if attempts > config.retries {
                                return Err(err);
                            }
                        } else {
                            println!("Could not connect: {err}");
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
            let res = match std::str::from_utf8(&buf) {
                Ok(s) => s,
                Err(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "MPD sent invalid UTF-8",
                    ))
                }
            };

            if !res.starts_with("OK MPD") {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Excepted `OK MPD {{VERSION}}` from server but got `{res}`"),
                ));
            }
        }
        {
            println!("Setting binary output limit to {SIZE_LIMIT} bytes");
            let res = conn.request_data(concatcp!("binarylimit ", SIZE_LIMIT))?;

            if res != "OK" {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Excepted `OK` from server but got `{res}`"),
                ));
            }
        }

        Ok(conn)
    }
}
