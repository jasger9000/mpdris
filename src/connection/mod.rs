mod error;

use async_std::io::{BufReader, BufWriter};
use async_std::net::TcpStream;
use async_std::task::sleep;
use std::net::SocketAddr;
use std::time::Duration;

use crate::config::Config;
use const_format::concatcp;
use futures_util::{
    io::{ReadHalf, WriteHalf},
    AsyncReadExt, AsyncWriteExt,
};

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
    /// elapsed time of the current song in seconds
    pub elapsed: usize,
}

impl Status {
    fn new() -> Self {
        Self {
            playing: false,
            volume: 100,
            repeat: Repeat::Off,
            shuffle: false,
            elapsed: 0,
        }
    }
}

#[derive(PartialEq, Debug)]
pub enum Repeat {
    Off = 0,
    On = 1,
    Single = 2,
}

pub struct MpdConnection {
    reader: BufReader<ReadHalf<TcpStream>>,
    writer: BufWriter<WriteHalf<TcpStream>>,
}

impl MpdConnection {
    pub async fn request_data(&mut self, request: &str) -> Result<String> {
        let mut data = String::new();
        loop {
            let mut buf = [0; SIZE_LIMIT];

            self.writer
                .write_all(format!("{request}\n").as_bytes())
                .await?;
            self.writer.flush().await?; // wait until the request is definitely sent to mpd

            let _ = self.reader.read(&mut buf).await?; // non-full buffers are intended
            let s = std::str::from_utf8(&buf)?;

            data.push_str(s.trim_matches(|c| c == '\0').trim());

            if data.len() > MAX_DATA_SIZE {
                self.empty_connection().await;

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
    async fn empty_connection(&mut self) {
        let mut buf = [0; SIZE_LIMIT];

        while let Ok(read_amount) = self.reader.read(&mut buf).await {
            if read_amount == 0 {
                break;
            }
        }
    }

    /// Start playback from current song position
    pub async fn play(&mut self) -> Result<()> {
        let _ = self.request_data("pause 0").await?;

        Ok(())
    }

    /// Seek to a position in the current song with offset in seconds.
    /// To seek relative to the current position use [Self::seek_relative]
    pub async fn seek(&mut self, time: Duration) -> Result<()> {
        let _ = self
            .request_data(&format!(
                "seekcur {}.{}",
                time.as_secs(),
                time.subsec_millis()
            ))
            .await?;

        Ok(())
    }

    /// Seek to a position in the current song relative to the current position with offset in
    /// milliseconds.
    /// To seek from the songs begin (absolute) use [Self::seek]
    pub async fn seek_relative(&mut self, offset: i64) -> Result<()> {
        let prefix = if offset > 0 { '+' } else { '-' };
        let dur = Duration::from_micros(offset.unsigned_abs());

        let _ = self
            .request_data(&format!(
                "seekcur {}{}.{}",
                prefix,
                dur.as_secs(),
                dur.subsec_millis()
            ))
            .await?;

        Ok(())
    }

    /// Pause playback
    pub async fn pause(&mut self) -> Result<()> {
        let _ = self.request_data("pause 1").await?;

        Ok(())
    }

    /// Toggle playback e.g. pause when playing and play when paused
    pub async fn toggle_play(&mut self) -> Result<()> {
        let _ = self.request_data("pause").await?;

        Ok(())
    }

    pub async fn get_status(&mut self) -> Result<Status> {
        let res = self.request_data("status").await?;

        let mut status = Status::new();

        for line in res.lines() {
            let mut parts = line.split(": ");

            if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                match k {
                    "state" => status.playing = v.contains("play"),
                    "single" => {
                        if v.parse().unwrap_or(0) > 0 {
                            status.repeat = Repeat::Single;
                        }
                    }
                    "repeat" => {
                        if v.parse().unwrap_or(0) > 0 && status.repeat == Repeat::Off {
                            status.repeat = Repeat::On;
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

    pub async fn init_connection(config: &Config) -> Result<Self> {
        println!(
            "Connecting to server on ip-address: {} using port: {}",
            config.addr, config.port
        );

        let (r, w) = {
            let mut attempts = 0;
            let addr = &SocketAddr::new(config.addr, config.port);

            loop {
                match TcpStream::connect(addr).await {
                    Ok(stream) => {
                        break stream.split();
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

                        eprintln!("Retrying in 3 seconds");
                        sleep(Duration::from_secs(3)).await;
                    }
                }
            }
        };

        let mut conn = Self {
            reader: BufReader::new(r),
            writer: BufWriter::new(w),
        };

        {
            println!("Validating connection");
            let mut buf = [0; 1024];

            let _ = conn.reader.read(&mut buf).await?; // non-full buffers are intended
            let res = std::str::from_utf8(&buf)?;

            if !res.starts_with("OK MPD") {
                return Err(Error::new_string(ErrorKind::InvalidConnection,
                    format!("Could not validate connection. Excepted `OK MPD {{VERSION}}` from server but got `{res}`"),
                ));
            }
        }
        {
            println!("Setting binary output limit to {SIZE_LIMIT} bytes");
            let res = conn
                .request_data(concatcp!("binarylimit ", SIZE_LIMIT))
                .await?;

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
