use async_std::sync::Mutex;
use std::{collections::HashMap, ops::Add, sync::Arc, time::Duration};
use zbus::{
    fdo, interface,
    zvariant::{ObjectPath, Value},
    SignalContext,
};

use crate::config::Config;
use crate::connection::{MpdClient, PlayState, Repeat, Status};

use super::{id_to_path, path_to_id};

pub struct PlayerInterface {
    mpd: Arc<MpdClient>,
    status: Arc<Mutex<Status>>,
    config: Arc<Mutex<Config>>,
}

impl PlayerInterface {
    pub async fn new(connection: Arc<MpdClient>, config: Arc<Mutex<Config>>) -> Self {
        let status = connection.get_status();
        Self {
            mpd: connection,
            status,
            config,
        }
    }
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl PlayerInterface {
    async fn next(&mut self) -> fdo::Result<()> {
        let s = self.status.lock().await;

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
        let s = self.status.lock().await;

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

    async fn seek(&mut self, ms: i64, #[zbus(signal_context)] ctxt: SignalContext<'_>) -> fdo::Result<()> {
        let s = self.status.lock().await;
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

        self.seeked(&ctxt, s.elapsed.unwrap_or(Duration::ZERO).add(ms).as_micros() as u64)
            .await?;

        Ok(())
    }

    async fn set_position(
        &mut self,
        track_path: ObjectPath<'_>,
        ms: i64,
        #[zbus(signal_context)] ctxt: SignalContext<'_>,
    ) -> fdo::Result<()> {
        if ms < 0 {
            return Ok(());
        }

        let pos = Duration::from_micros(ms.unsigned_abs());
        let s = self.status.lock().await;
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

        self.seeked(&ctxt, ms.unsigned_abs()).await?;

        Ok(())
    }

    #[zbus(signal)]
    pub async fn seeked(&self, ctxt: &SignalContext<'_>, ms: u64) -> zbus::Result<()>;

    #[zbus(property)]
    async fn playback_status(&self) -> &str {
        match self.status.lock().await.state {
            PlayState::Playing => "Playing",
            PlayState::Paused => "Paused",
            PlayState::Stopped => "Stopped",
        }
    }

    #[zbus(property)]
    async fn loop_status(&self) -> &str {
        match self.status.lock().await.repeat {
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

        self.status.lock().await.repeat = if single == 1 {
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
        self.status.lock().await.shuffle
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

        self.status.lock().await.shuffle = shuffle;
        Ok(())
    }

    #[zbus(property)]
    async fn metadata(&self) -> HashMap<&str, Value> {
        let s = self.status.lock().await;
        let music_dir  = &self.config.lock().await.music_directory;
        let mut map = HashMap::new();

        if let Some(song) = &s.current_song {
            map.insert("mpris:trackid", id_to_path(song.id).into());
            map.insert("xesam:artist", song.artists.clone().into());
            map.insert("xesam:url", format!("file://{}/{}", music_dir, song.uri).into());

            if let Some(duration) = s.duration {
                map.insert("mpris:length", (duration.as_micros() as u64).into());
            }
            if let Some(cover) = &song.find_cover_url(music_dir).await {
                map.insert("mpris:artUrl", cover.clone().into());
            }
            if let Some(title) = &song.title {
                map.insert("xesam:title", title.clone().into());
            }
            if let Some(album) = &song.album {
                map.insert("xesam:album", album.clone().into());
            }
            if let Some(album_artist) = &song.album_artist {
                map.insert("xesam:albumArtist", album_artist.clone().into());
            }
            // TODO date
            if let Some(track) = song.track {
                map.insert("xesam:trackNumber", track.into());
            }
        }

        return map;
    }

    #[zbus(property)]
    async fn volume(&self) -> u8 {
        self.status.lock().await.volume
    }

    #[zbus(property)]
    async fn set_volume(&self, volume: i16) -> zbus::Result<()> {
        if volume > 100 {
            return Err(fdo::Error::InvalidArgs(String::from("Volume cannot be greater than 100")).into());
        }

        match self.mpd.request_data(&format!("setvol {volume}")).await {
            Ok(_) => {}
            Err(err) => {
                eprintln!("Could not set volume: {err}");
                return Err(Into::<fdo::Error>::into(err).into());
            }
        }

        self.status.lock().await.volume = volume as u8;
        Ok(())
    }

    #[zbus(property)]
    async fn position(&self) -> fdo::Result<u64> {
        self.mpd.update_status().await?;
        return Ok(self.status.lock().await.elapsed.unwrap_or(Duration::ZERO).as_micros() as u64);
    }

    #[zbus(property)]
    async fn minimum_rate(&self) -> f32 {
        1.0
    }

    #[zbus(property)]
    async fn maximum_rate(&self) -> f32 {
        1.0
    }

    #[zbus(property)]
    async fn can_go_next(&self) -> bool {
        self.status.lock().await.next_song.is_some()
    }

    #[zbus(property)]
    async fn can_go_previous(&self) -> bool {
        self.status.lock().await.playlist_length > 1
    }

    #[zbus(property)]
    async fn can_play(&self) -> bool {
        self.status.lock().await.current_song.is_some()
    }

    #[zbus(property)]
    async fn can_pause(&self) -> bool {
        self.status.lock().await.current_song.is_some()
    }

    #[zbus(property)]
    async fn can_seek(&self) -> bool {
        self.status.lock().await.current_song.is_some()
    }

    #[zbus(property)]
    async fn can_control(&self) -> bool {
        true
    }
}
