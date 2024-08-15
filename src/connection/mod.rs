mod error;
mod status;

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use async_std::channel::{bounded, Receiver, Sender};
use async_std::io::{self, BufReader, BufWriter};
use async_std::net::TcpStream;
use async_std::sync::{Arc, Mutex};
use async_std::task::{sleep, spawn, JoinHandle};

use const_format::concatcp;
use futures_util::{
    future::{join, select, Either},
    io::{ReadHalf, WriteHalf},
    pin_mut, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt,
};

use crate::config::Config;

pub use self::error::MPDResult as Result;
pub use self::error::*;
pub use self::status::{PlayState, Repeat, StateChanged, Status};

/// How many bytes MPD sends at once
const SIZE_LIMIT: usize = 1024;
/// Request that gets send when the connection waits for something to happen
const IDLE_REQUEST: &str = "idle stored_playlist playlist player mixer options";

pub struct MpdClient {
    connection: Arc<Mutex<MpdConnection>>,
    idle_connection: Arc<Mutex<MpdConnection>>,
    drop_idle_lock: Sender<()>,
    /// Cached status
    status: Arc<Mutex<Status>>,
    sender: Sender<StateChanged>,
    #[allow(unused)]
    ping_task: JoinHandle<()>,
    #[allow(unused)]
    idle_task: JoinHandle<()>,
}

struct MpdConnection {
    reader: BufReader<ReadHalf<TcpStream>>,
    writer: BufWriter<WriteHalf<TcpStream>>,
    config: Arc<Mutex<Config>>,
}

impl MpdConnection {
    pub async fn new(config: Arc<Mutex<Config>>) -> Result<Self> {
        let (r, w) = {
            let c = config.lock().await;
            Self::connect(c.addr, c.port, c.retries).await?
        };

        let mut conn = Self {
            reader: r,
            writer: w,
            config,
        };

        conn.after_connect().await?;
        Ok(conn)
    }

    async fn request_data(&mut self, request: &str) -> Result<Vec<(String, String)>> {
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
    }

    pub async fn reconnect(&mut self) -> Result<()> {
        {
            let c = self.config.lock().await;

            println!("Reconnecting to server on ip-address: {} using port: {}", c.addr, c.port);
            let (r, w) = Self::connect(c.addr, c.port, c.retries).await?;
            self.reader = r;
            self.writer = w;
        }

        self.after_connect().await
    }
}

impl MpdClient {
    pub async fn request_data(&self, request: &str) -> Result<Vec<(String, String)>> {
        let mut c = self.connection.lock().await;

        c.request_data(request).await
    }

    pub async fn reconnect(&self) -> Result<()> {
        self.drop_idle_lock.send(()).await.expect("Channel must always be open");
        let (mut c, mut ic) = join(self.connection.lock(), self.idle_connection.lock()).await;

        c.reconnect().await?;
        ic.reconnect().await
    }

    /// Play the song with the given id, returns error if the id is invalid
    pub async fn play_song(&self, id: u32) -> Result<()> {
        let _ = self.request_data(&format!("seekid {id} 0")).await?;

        Ok(())
    }

    /// Start playback from current song position
    pub async fn play(&self) -> Result<()> {
        let _ = self.request_data("play").await?;

        Ok(())
    }

    /// Seek to time in the current song
    /// To seek relative to the current position use [Self::seek_relative]
    pub async fn seek(&self, time: Duration) -> Result<()> {
        let _ = self
            .request_data(&format!("seekcur {}.{}", time.as_secs(), time.subsec_millis()))
            .await?;

        Ok(())
    }

    /// Seek to a position in the current song relative to the current position with offset in
    /// To seek from the songs begin (absolute) use [Self::seek]
    pub async fn seek_relative(&self, is_positive: bool, offset: Duration) -> Result<()> {
        let prefix = if is_positive { '+' } else { '-' };

        let _ = self
            .request_data(&format!("seekcur {}{}.{}", prefix, offset.as_secs(), offset.subsec_millis()))
            .await?;

        Ok(())
    }

    /// Pause playback
    pub async fn pause(&self) -> Result<()> {
        let _ = self.request_data("pause 1").await?;

        Ok(())
    }

    /// Stop playback
    pub async fn stop(&self) -> Result<()> {
        let _ = self.request_data("stop").await?;

        Ok(())
    }

    /// Toggle playback, e.g. pauses when playing and play when paused
    pub async fn toggle_play(&self) -> Result<()> {
        let _ = self.request_data("pause").await?;

        Ok(())
    }

    pub fn get_status(&self) -> Arc<Mutex<Status>> {
        self.status.clone()
    }

    pub async fn update_status(&self) -> Result<()> {
        let mut s = self.status.lock().await;
        let mut conn = self.connection.lock().await;
        let sender = &self.sender;

        status::update_status(&mut conn, &mut s, sender).await?;
        Ok(())
    }

    pub async fn new(config: Arc<Mutex<Config>>) -> Result<(Self, Receiver<StateChanged>)> {
        let c = config.lock().await;

        println!("Connecting to server on ip-address: {} using port: {}", c.addr, c.port);

        drop(c);
        let (sender, recv) = bounded(1);
        let status = Arc::new(Mutex::new(Status::new()));
        let connection = Arc::new(Mutex::new(MpdConnection::new(config.clone()).await?));
        println!("Connecting second stream to ask for updates");
        let idle_connection = Arc::new(Mutex::new(MpdConnection::new(config.clone()).await?));
        let (drop_idle_lock, drop_lock) = bounded(1);

        let idle_conn = Arc::clone(&idle_connection);
        let idle_sender = Sender::clone(&sender);
        let idle_status = Arc::clone(&status);
        let ping_conn = Arc::clone(&connection);

        let idle_task = spawn(async move {
            loop {
                sleep(Duration::from_nanos(1)).await; // necessary to acquire lock in reconnect fn
                let mut conn = idle_conn.lock().await;
                let result = {
                    let request = conn.request_data(IDLE_REQUEST);
                    let drop_lock = async {
                        drop_lock.recv().await.expect("Channel must always be open");
                    };

                    pin_mut!(request, drop_lock);
                    match select(drop_lock, request).await {
                        Either::Left((_, _)) => continue,
                        Either::Right((res, _)) => res,
                    }
                };

                match result {
                    Ok(response) => {
                        let mut s = idle_status.lock().await;

                        match status::update_status(&mut conn, &mut s, &idle_sender).await {
                            Ok(could_be_seeking) => {
                                if response[0].1 == "player" && could_be_seeking {
                                    let elapsed = s.elapsed.unwrap().as_micros() as u64;
                                    drop(s);

                                    idle_sender.send(StateChanged::Position(elapsed)).await.unwrap();
                                }
                            }
                            Err(err) => {
                                eprintln!("Could not update status: {err}");
                            }
                        }
                    }
                    Err(err) => {
                        eprintln!("Error while awaiting change in MPD: {err}");
                        continue;
                    }
                }
            }
        });
        let ping_task = spawn(async move {
            loop {
                let mut conn = ping_conn.lock().await;

                match conn.request_data("ping").await {
                    Ok(_) => {}
                    Err(err) => {
                        eprintln!("Could not ping MPD: {err}");
                    }
                };
                drop(conn);
                sleep(Duration::from_secs(15)).await;
            }
        });

        let client = Self {
            connection,
            idle_connection,
            drop_idle_lock,
            sender,
            ping_task,
            idle_task,
            status,
        };

        client.update_status().await?;

        Ok((client, recv))
    }
}
