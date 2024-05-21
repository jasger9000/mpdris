use core::net::IpAddr;
use std::io::{self, ErrorKind, Read, Write};
use std::net::TcpStream;

use const_format::concatcp;

/// How many bytes MPD sents at once
const SIZE_LIMIT: usize = 1024;

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
    pub fn request_data(&mut self, request: Option<&str>) -> io::Result<String> {
        let mut data = [0; SIZE_LIMIT];

        if let Some(req) = request {
            self.connection.write(format!("{req}\n").as_bytes())?;
        }

        self.connection.read(&mut data)?;
        let s = match std::str::from_utf8(&data) {
            Ok(s) => s,
            Err(err) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Could not read response into UTF-8: {err}"),
                ))
            }
        };

        Ok(String::from(s.trim_matches(|c| c == '\0').trim()))
    }

    pub fn play(&mut self) -> io::Result<()> {
        return match self.request_data(Some("play")) {
            Ok(s) => {
                if s == "OK" {
                    Ok(())
                } else {
                    Err(io::Error::new(ErrorKind::Other, "Could not play: {s}"))
                }
            }
            Err(err) => { Err(err) }
        }
    }

    pub fn pause(&mut self) -> io::Result<()> {
        return match self.request_data(Some("pause")) {
            Ok(s) => {
                if s == "OK" {
                    Ok(())
                } else {
                    Err(io::Error::new(ErrorKind::Other, "Could not pause: {s}"))
                }
            }
            Err(err) => { Err(err) }
        }
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
        let res = self.request_data(Some("status")).unwrap_or(String::new());

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

    pub fn init_connection(addr: IpAddr, port: u16) -> io::Result<Self> {
        println!("Connecting to server on ip-address: {addr} using port: {port}");
        let mut conn = Self {
            connection: TcpStream::connect(format!("{addr}:{port}"))?,
        };

        {
            println!("Validating connection");

            let res = conn.request_data(None)?;

            if !res.starts_with("OK MPD") {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Excepted `OK MPD {{VERSION}}` from server but got `{res}`"),
                ));
            }
        }
        {
            println!("Setting binary output limit to {SIZE_LIMIT} bytes");
            let res = conn.request_data(Some(concatcp!("binarylimit ", SIZE_LIMIT)))?;

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
