use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use async_std::io::{self, BufReader, BufWriter};
use async_std::net::TcpStream;
use async_std::task::sleep;

use const_format::concatcp;
use futures_util::io::{ReadHalf, WriteHalf};
use futures_util::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use libc::SIGTERM;

use super::error::MPDResult as Result;
use super::error::{Error, ErrorKind};
use crate::config::{config, Config};
use crate::send_sig;

/// How many bytes MPD sends at once
const SIZE_LIMIT: usize = 1024;

pub struct MPDConnection {
    reader: BufReader<ReadHalf<TcpStream>>,
    writer: BufWriter<WriteHalf<TcpStream>>,
}

impl MPDConnection {
    pub async fn new(c: &Config) -> Result<Self> {
        let (r, w) = Self::connect(c.addr, c.port, c.retries).await?;

        let mut conn = Self { reader: r, writer: w };

        conn.after_connect().await?;
        Ok(conn)
    }

    pub async fn request_data(&mut self, request: &str) -> Result<Vec<(String, String)>> {
        match self.request_data_in(request).await {
            Ok(ok) => Ok(ok),
            Err(err) => {
                eprintln!("Failed to read from MPD connection, reconnecting: {err}");
                self.reconnect().await?;
                self.request_data_in(request).await
            }
        }
    }

    async fn request_data_in(&mut self, request: &str) -> Result<Vec<(String, String)>> {
        let request = format!("{request}\n");

        self.writer.write_all(request.as_bytes()).await?;
        self.writer.flush().await?; // wait until the request is definitely sent to mpd

        self.read_data().await
    }

    async fn read_data(&mut self) -> Result<Vec<(String, String)>> {
        let mut data: Vec<(String, String)> = Vec::new();
        let mut buf = String::new();
        let mut failed_parses: u8 = 0;

        loop {
            self.reader.read_line(&mut buf).await?;

            if buf.starts_with("OK") {
                // lines starting with OK indicate the end of response
                break;
            } else if buf.starts_with("ACK") {
                return Err(Error::try_from_mpd(buf)?);
            }

            let mut parts = buf.split(": ");

            if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                data.push((k.to_string(), v.trim().to_string()));
            } else {
                failed_parses += 1;
                eprintln!("Could not split response line into key-value pair (failed parses {failed_parses})");
                if failed_parses >= 3 {
                    return Err(Error::new_string(
                        ErrorKind::KeyValueError,
                        format!("Failed to parse {failed_parses} lines into key-value pairs"),
                    ));
                }
            }

            buf.clear();
        }

        Ok(data)
    }

    async fn after_connect(&mut self) -> Result<()> {
        self.read_data().await?;
        println!("Setting binary output limit to {SIZE_LIMIT} bytes");
        self.request_data_in(concatcp!("binarylimit ", SIZE_LIMIT)).await?;

        Ok(())
    }

    async fn connect(
        addr: IpAddr,
        port: u16,
        retries: isize,
    ) -> io::Result<(BufReader<ReadHalf<TcpStream>>, BufWriter<WriteHalf<TcpStream>>)> {
        let mut attempts = 0;
        let addr = &SocketAddr::new(addr, port);

        loop {
            match TcpStream::connect(addr).await {
                Ok(stream) => {
                    let (r, w) = stream.split();

                    println!("Connection established");
                    return Ok((BufReader::new(r), BufWriter::new(w)));
                }
                Err(err) => {
                    if retries > 0 {
                        eprintln!("Could not connect (tries left {}): {err}", retries - attempts);

                        attempts += 1;
                        if attempts > retries {
                            return Err(err);
                        }
                    } else {
                        eprintln!("Could not connect: {err}");
                    }

                    eprintln!("Retrying in 3 seconds");
                    sleep(Duration::from_secs(3)).await;
                }
            }
        }
    }

    pub async fn reconnect(&mut self) -> Result<()> {
        {
            let c = config().read().await;

            println!("Reconnecting to server on ip-address: {} using port: {}", c.addr, c.port);
            let (r, w) = Self::connect(c.addr, c.port, c.retries).await.unwrap_or_else(|e| {
                eprintln!("Failed to reconnect to MPD, exiting: {e}");
                send_sig(std::process::id(), SIGTERM).expect("should always be able to send signal");
                loop {
                    // wait for the signal handler to gracefully shut down
                    std::thread::sleep(Duration::from_secs(u64::MAX));
                }
            });

            self.reader = r;
            self.writer = w;
        }

        self.after_connect().await
    }
}
