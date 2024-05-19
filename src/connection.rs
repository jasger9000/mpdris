use core::net::IpAddr;
use std::io::{self, Read, Write};
use std::net::TcpStream;

use const_format::concatcp;

/// How many bytes MPD sents at once
const SIZE_LIMIT: usize = 1024;

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

    pub fn init_connection(addr: IpAddr, port: u16) -> io::Result<Self> {
        eprintln!("Connecting to server on ip-address: {addr} using port: {port}");
        let mut conn = Self {
            connection: TcpStream::connect(format!("{addr}:{port}"))?,
        };

        {
            eprintln!("Validating connection");

            let res = conn.request_data(None)?;

            if !res.starts_with("OK MPD") {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Excepted `OK MPD {{VERSION}}` from server but got `{res}`"),
                ));
            }
        }
        {
            eprintln!("Setting binary output limit to {SIZE_LIMIT} bytes");
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
