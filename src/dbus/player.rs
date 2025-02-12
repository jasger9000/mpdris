use async_std::sync::RwLock;
use std::{collections::HashMap, ops::Add, sync::Arc, time::Duration};
use zbus::{
    fdo, interface,
    object_server::SignalEmitter,
    zvariant::{ObjectPath, Value},
};

use crate::client::{MPDClient, PlayState, Repeat, Status};
use crate::config::config;

use super::{id_to_path, path_to_id};

pub struct PlayerInterface {
    mpd: Arc<MPDClient>,
    status: Arc<RwLock<Status>>,
}

impl PlayerInterface {
    pub async fn new(connection: Arc<MPDClient>) -> Self {
        let status = connection.get_status();
        Self { mpd: connection, status }
    }
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl PlayerInterface {
    async fn next(&mut self) -> fdo::Result<()> {
        let s = self.status.read().await;

        if let Some(next_id) = s.next_song {
            self.mpd.play_song(next_id).await.map_err(|err| {
                eprintln!("Failed to switch to next song: {err}");
                err.into()
            })
        } else if s.repeat == Repeat::Off {
            self.mpd.pause().await.map_err(|err| {
                eprintln!("Failed to pause playback because of empty playlist after next: {err}");
                err.into()
            })
        } else {
            Ok(())
        }
    }

    async fn previous(&mut self) -> fdo::Result<()> {
        let s = self.status.read().await;

        if s.playlist_length >= 1 {
            let cmd = if s.state != PlayState::Playing {
                "command_list_begin\nprevious\npause\ncommand_list_end"
            } else {
                "previous"
            };

            match self.mpd.request_data(cmd).await {
                Ok(_) => Ok(()),
                Err(err) => {
                    eprintln!("Failed to switch to previous song: {err}");
                    Err(err.into())
                }
            }
        } else if s.playlist_length <= 1 && s.repeat == Repeat::Off {
            self.mpd.stop().await.map_err(|err| {
                eprintln!("Failed to pause playback because of empty playlist after previous: {err}");
                err.into()
            })
        } else {
            Ok(())
        }
    }

    async fn pause(&mut self) -> fdo::Result<()> {
        self.mpd.pause().await.map_err(|err| {
            eprintln!("Failed to pause playback: {err}");
            err.into()
        })
    }

    async fn play_pause(&mut self) -> fdo::Result<()> {
        if !self.can_pause().await {
            return Err(fdo::Error::Failed(String::from(
                "Attempted to toggle playback while CanPause is false",
            )));
        }

        self.mpd.toggle_play().await.map_err(|err| {
            eprintln!("Failed to toggle playback: {err}");
            err.into()
        })
    }

    async fn stop(&mut self) -> fdo::Result<()> {
        self.mpd.stop().await.map_err(|err| {
            eprintln!("Failed to stop playback: {err}");
            err.into()
        })
    }

    async fn play(&mut self) -> fdo::Result<()> {
        self.mpd.play().await.map_err(|err| {
            eprintln!("Failed to start playback: {err}");
            err.into()
        })
    }

    async fn seek(&mut self, ms: i64, #[zbus(signal_emitter)] ctxt: SignalEmitter<'_>) -> fdo::Result<()> {
        let s = self.status.read().await;
        let is_positive = ms > 0;
        let ms = Duration::from_micros(ms.unsigned_abs());

        if s.elapsed.unwrap_or(Duration::ZERO) + ms > s.duration.unwrap_or(Duration::MAX) {
            drop(s);
            self.next().await?;
            return Ok(());
        }

        match self.mpd.seek_relative(is_positive, ms).await {
            Ok(_) => {}
            Err(err) => {
                eprintln!("Failed to seek: {err}");
                return Err(err.into());
            }
        }

        Self::seeked(&ctxt, s.elapsed.unwrap_or(Duration::ZERO).add(ms).as_micros() as i64).await?;

        Ok(())
    }

    async fn set_position(
        &mut self,
        track_path: ObjectPath<'_>,
        ms: i64,
        #[zbus(signal_emitter)] ctxt: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        if ms < 0 {
            return Ok(());
        }

        let pos = Duration::from_micros(ms.unsigned_abs());
        let s = self.status.read().await;
        let Some(track_id) = path_to_id(&track_path) else {
            return Ok(());
        };

        if pos > s.duration.unwrap_or(Duration::MAX)
            || s.current_song.is_none()
            || s.current_song.as_ref().unwrap().id != track_id
        {
            return Ok(());
        }

        match self.mpd.seek(pos).await {
            Ok(_) => {}
            Err(err) => {
                eprintln!("Failed to set position: {err}");
                return Err(err.into());
            }
        }

        Self::seeked(&ctxt, ms).await?;

        Ok(())
    }

    #[zbus(signal)]
    pub async fn seeked(ctxt: &SignalEmitter<'_>, ms: i64) -> zbus::Result<()>;

    #[zbus(property)]
    async fn playback_status(&self) -> &str {
        match self.status.read().await.state {
            PlayState::Playing => "Playing",
            PlayState::Paused => "Paused",
            PlayState::Stopped => "Stopped",
        }
    }

    #[zbus(property)]
    async fn loop_status(&self) -> &str {
        match self.status.read().await.repeat {
            Repeat::Off => "None",
            Repeat::On => "Playlist",
            Repeat::Single => "Track",
        }
    }

    #[zbus(property)]
    async fn set_loop_status(&mut self, loop_status: String) -> fdo::Result<()> {
        let (repeat, single) = match loop_status.as_str() {
            "None" => (0u8, 0u8),
            "Playlist" => (1, 0),
            "Track" => (1, 1),
            _ => return Err(fdo::Error::InvalidArgs(format!("`{loop_status}` is not a valid loop status"))),
        };

        let cmd = format!("command_list_begin\nrepeat {repeat}\nsingle {single}\ncommand_list_end");
        match self.mpd.request_data(&cmd).await {
            Ok(_) => {}
            Err(err) => {
                eprintln!("Failed to set loop status: {err}");
                return Err(err.into());
            }
        };

        self.status.write().await.repeat = if single == 1 {
            Repeat::Single
        } else if repeat == 1 {
            Repeat::On
        } else {
            Repeat::Off
        };

        Ok(())
    }

    #[zbus(property)]
    async fn shuffle(&self) -> bool {
        self.status.read().await.shuffle
    }

    #[zbus(property)]
    async fn set_shuffle(&self, shuffle: bool) -> zbus::Result<()> {
        let cmd = if shuffle { "random 1" } else { "random 0" };

        match self.mpd.request_data(cmd).await {
            Ok(_) => {}
            Err(err) => {
                eprintln!("Could not set shuffleing: {err}");
                return Err(Into::<fdo::Error>::into(err).into());
            }
        }

        self.status.write().await.shuffle = shuffle;
        Ok(())
    }

    #[zbus(property)]
    async fn metadata(&self) -> HashMap<&str, Value> {
        let s = self.status.read().await;
        let c = config().read().await;

        let music_dir: &str = &c.music_directory;
        let mut map = HashMap::new();

        if let Some(song) = &s.current_song {
            map.insert("mpris:trackid", id_to_path(song.id).into());
            map.insert("xesam:url", format!("file://{}/{}", music_dir, song.uri).into());

            if let Some(duration) = s.duration {
                map.insert("mpris:length", (duration.as_micros() as i64).into());
            }
            if let Some(cover) = &song.cover {
                map.insert("mpris:artUrl", Arc::clone(cover).into());
            }
            if let Some(album) = &song.album {
                map.insert("xesam:album", Arc::clone(album).into());
            }
            if !song.album_artists.is_empty() {
                map.insert("xesam:albumArtist", song.album_artists.into_value());
            }
            if !song.artists.is_empty() {
                map.insert("xesam:artist", song.artists.into_value());
            }
            if !song.comments.is_empty() {
                map.insert("xesam:comment", song.comments.into_value());
            }
            if !song.composers.is_empty() {
                map.insert("xesam:composer", song.composers.into_value());
            }
            if let Some(date) = song.date {
                map.insert("xesam:contentCreated", format!("{date}-01-01T00:00+0000").into());
            }
            if let Some(disc) = song.disc {
                map.insert("xesam:discNumber", disc.into());
            }
            if !song.genres.is_empty() {
                map.insert("xesam:genre", song.genres.into_value());
            }
            if let Some(title) = &song.title {
                map.insert("xesam:title", Arc::clone(title).into());
            }
            if let Some(track) = song.track {
                map.insert("xesam:trackNumber", track.into());
            }
        }

        map
    }

    #[zbus(property)]
    async fn volume(&self) -> f64 {
        self.status.read().await.volume as f64
    }

    #[zbus(property)]
    async fn set_volume(&self, volume: f64) -> zbus::Result<()> {
        if volume < 0.0 || volume > 100.0 {
            return Err(fdo::Error::InvalidArgs(String::from("Volume must be between 0 and 100")).into());
        }

        match self.mpd.request_data(&format!("setvol {}", volume as u8)).await {
            Ok(_) => {}
            Err(err) => {
                eprintln!("Could not set volume: {err}");
                return Err(Into::<fdo::Error>::into(err).into());
            }
        }

        self.status.write().await.volume = volume as u8;
        Ok(())
    }

    #[zbus(property)]
    async fn position(&self) -> fdo::Result<i64> {
        self.mpd.update_status().await?;
        Ok(self.status.read().await.elapsed.unwrap_or(Duration::ZERO).as_micros() as i64)
    }

    #[zbus(property)]
    async fn rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    async fn set_rate(&mut self, rate: f64) -> fdo::Result<()> {
        if rate == 0.0 {
            self.pause().await?;
        }

        Ok(())
    }

    #[zbus(property)]
    async fn minimum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    async fn maximum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    async fn can_go_next(&self) -> bool {
        self.status.read().await.next_song.is_some()
    }

    #[zbus(property)]
    async fn can_go_previous(&self) -> bool {
        self.status.read().await.playlist_length > 1
    }

    #[zbus(property)]
    async fn can_play(&self) -> bool {
        self.status.read().await.current_song.is_some()
    }

    #[zbus(property)]
    async fn can_pause(&self) -> bool {
        self.status.read().await.current_song.is_some()
    }

    #[zbus(property)]
    async fn can_seek(&self) -> bool {
        self.status.read().await.current_song.is_some()
    }

    #[zbus(property)]
    async fn can_control(&self) -> bool {
        true
    }
}

trait SliceToValue {
    fn into_value<'a, 'v>(&'a self) -> Value<'v>
    where
        Self: Clone;
}

/// Why don't we just .clone() the vec? Because then we'd actually create two new vecs, the one we
/// cloned and the one that the From implementation will create. With this we reduce to one clone
impl SliceToValue for Vec<Arc<str>> {
    fn into_value<'a, 'v>(&'a self) -> Value<'v> {
        Value::Array(self.into())
    }
}
