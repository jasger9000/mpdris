use std::time::Duration;

use async_std::channel::{Receiver, Sender, bounded, unbounded};
use async_std::sync::{Arc, Mutex, RwLock};
use async_std::task::{JoinHandle, sleep, spawn};

use futures_util::future::{Either, join, select};
use futures_util::pin_mut;
use log::{info, warn};

use self::connection::MPDConnection;
pub use self::error::MPDResult as Result;
pub use self::error::*;
pub use self::status::{PlayState, Repeat, StateChanged, Status};
use crate::config::config;

mod connection;
mod error;
mod status;

/// Request that gets send when the connection waits for something to happen
const IDLE_REQUEST: &str = "idle stored_playlist playlist player mixer options";

pub struct MPDClient {
    connection: Arc<Mutex<MPDConnection>>,
    idle_connection: Arc<Mutex<MPDConnection>>,
    drop_idle_lock: Sender<()>,
    /// Cached status
    status: Arc<RwLock<Status>>,
    sender: Sender<StateChanged>,
    #[allow(unused)]
    ping_task: JoinHandle<()>,
    #[allow(unused)]
    idle_task: JoinHandle<()>,
}

impl MPDClient {
    pub async fn request_data(&self, request: &str) -> Result<Vec<(String, String)>> {
        let mut c = self.connection.lock().await;

        c.request_data(request).await
    }

    pub async fn reconnect(&self) -> Result<()> {
        let _ = self.drop_idle_lock.send(()).await;
        let (mut c, mut ic) = join(self.connection.lock(), self.idle_connection.lock()).await;

        c.reconnect().await?;
        ic.reconnect().await?;
        let _ = self.drop_idle_lock.send(()).await;
        Ok(())
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

    pub fn get_status(&self) -> Arc<RwLock<Status>> {
        Arc::clone(&self.status)
    }

    pub async fn update_status(&self) -> Result<()> {
        let mut s = self.status.write().await;
        let mut conn = self.connection.lock().await;
        let sender = &self.sender;

        status::update_status(&mut conn, &mut s, sender).await?;
        Ok(())
    }

    pub async fn new() -> Result<(Self, Receiver<StateChanged>)> {
        let c = config().read().await;

        info!("Connecting to server on ip-address: {} using port: {}", c.addr, c.port);

        let (sender, recv) = unbounded();
        let status = Arc::new(RwLock::new(Status::new()));
        let connection = Arc::new(Mutex::new(MPDConnection::new(&c).await?));

        info!("Connecting second stream to ask for updates");
        let idle_connection = Arc::new(Mutex::new(MPDConnection::new(&c).await?));
        let (drop_idle_lock, drop_lock) = bounded(1);

        let idle_conn = Arc::clone(&idle_connection);
        let idle_sender = Sender::clone(&sender);
        let idle_status = Arc::clone(&status);
        let ping_conn = Arc::clone(&connection);

        let idle_task = spawn(idle_task(idle_conn, idle_status, idle_sender, drop_lock));
        let ping_task = spawn(ping_task(ping_conn));

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

async fn idle_task(
    connection: Arc<Mutex<MPDConnection>>,
    status: Arc<RwLock<Status>>,
    sender: Sender<StateChanged>,
    drop_lock: Receiver<()>,
) {
    loop {
        let mut conn = connection.lock().await;

        let result = {
            // we need assign result using coroutine because it is impossible to drop request and therefore the lock on conn
            let result = {
                let request = conn.request_data(IDLE_REQUEST);
                let drp = drop_lock.recv();

                pin_mut!(request, drp);
                match select(request, drp).await {
                    Either::Left((res, _)) => Some(res),
                    Either::Right((_, _)) => None,
                }
            };

            if result.is_none() {
                drop(conn);
                let _ = drop_lock.recv().await;
                continue;
            }
            result.unwrap()
        };

        match result {
            Ok(response) => {
                let mut s = status.write().await;

                match status::update_status(&mut conn, &mut s, &sender).await {
                    Ok(could_be_seeking) => {
                        if response[0].1 == "player" && could_be_seeking {
                            let elapsed = s.elapsed.unwrap().as_micros() as i64;
                            drop(s);

                            sender.send(StateChanged::Position(elapsed)).await.unwrap();
                        }
                    }
                    Err(err) => {
                        log::error!("Could not update status: {err}");
                    }
                }
            }
            Err(err) => {
                warn!("Error while awaiting change in MPD: {err}");
                continue;
            }
        }
    }
}

async fn ping_task(connection: Arc<Mutex<MPDConnection>>) {
    loop {
        let mut conn = connection.lock().await;

        match conn.request_data("ping").await {
            Ok(_) => {}
            Err(err) => {
                warn!("Could not ping MPD: {err}");
            }
        };
        drop(conn);
        sleep(Duration::from_secs(15)).await;
    }
}
