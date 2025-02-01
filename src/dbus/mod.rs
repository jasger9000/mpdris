use async_std::channel::Receiver;
use async_std::task::{spawn, JoinHandle};
use std::sync::Arc;
use zbus::zvariant::ObjectPath;
use zbus::Connection;
use zbus::{connection::Builder, InterfaceRef};

use base::BaseInterface;
use player::PlayerInterface;

use crate::connection::{MpdClient, StateChanged};

mod base;
mod player;

const NAME: &str = "org.mpris.MediaPlayer2.mpd";
const PATH: &str = "/org/mpris/MediaPlayer2";
const TRACKID_PATH_BASE: &str = "/org/musicpd/mpris/";

pub async fn serve(
    connection: Arc<MpdClient>,
    recv: Receiver<StateChanged>,
) -> Result<(Connection, JoinHandle<()>), zbus::Error> {
    let base = BaseInterface::new();
    let player = PlayerInterface::new(connection).await;

    let connection = Builder::session()?
        .name(NAME)?
        .serve_at(PATH, base)?
        .serve_at(PATH, player)?
        .build()
        .await?;

    let signal_connection = connection.clone();

    let task = spawn(async move {
        loop {
            if let Err(err) = send_signals(&signal_connection, &recv).await {
                eprintln!("D-Bus Change Signal Sender died, restarting: {err}");
            }
        }
    });

    Ok((connection, task))
}

fn id_to_path<'a>(id: u32) -> ObjectPath<'a> {
    ObjectPath::try_from(format!("{TRACKID_PATH_BASE}{id}")).expect("should always create a valid path")
}

fn path_to_id(path: &ObjectPath<'_>) -> Option<u32> {
    path.strip_prefix(TRACKID_PATH_BASE)?.parse().ok()
}

async fn send_signals(connection: &Connection, recv: &Receiver<StateChanged>) -> zbus::Result<()> {
    let object_server = connection.object_server();
    let player_iface_ref: InterfaceRef<PlayerInterface> = object_server.interface(PATH).await.unwrap();

    loop {
        use StateChanged::*;

        let change = recv.recv().await.expect("Channel must always be open");

        let player_iface = player_iface_ref.get_mut().await;
        let player_ctxt = player_iface_ref.signal_context();

        match change {
            Position(ms) => {
                player_iface.seeked(player_ctxt, ms).await?;
            }
            Song(prev, next) => {
                player_iface.metadata_changed(player_ctxt).await?;
                if prev {
                    player_iface.can_go_previous_changed(player_ctxt).await?;
                }
                if next {
                    player_iface.can_go_next_changed(player_ctxt).await?;
                }
            }
            Playlist => {
                // TODO implement tracklist interface
            }
            PlayState => {
                player_iface.playback_status_changed(player_ctxt).await?;
            }
            Volume => {
                player_iface.volume_changed(player_ctxt).await?;
            }
            Repeat => {
                player_iface.loop_status_changed(player_ctxt).await?;
            }
            Shuffle => {
                player_iface.shuffle_changed(player_ctxt).await?;
            }
        }
    }
}
