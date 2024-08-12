use async_std::sync::Mutex;
use std::collections::HashMap;
use std::ops::Add;
use std::time::Duration;
use std::{sync::Arc, usize};
use zbus::{
    fdo, interface,
    zvariant::{ObjectPath, Value},
    SignalContext,
};

use crate::connection::{MpdClient, PlayState, Repeat, Status};

pub struct PlayerInterface {
    mpd: Arc<MpdClient>,
    status: Arc<Mutex<Status>>,
}

impl PlayerInterface {
    pub async fn new(connection: Arc<MpdClient>) -> Self {
        let status = connection.get_status();
        Self {
            mpd: connection,
            status,
        }
    }
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl PlayerInterface {
    async fn next(&mut self) {
        let s = self.status.lock().await;

        if let Some(next_id) = s.next_song {
            match self
                .mpd
                .request_data(format!("seekid {next_id} 0").as_str())
                .await
            {
                Ok(_) => {}
                Err(err) => eprintln!("Failed to switch to next song: {err}"),
            }
        } else if s.repeat == Repeat::Off {
            self.mpd.pause().await.unwrap_or_else(|err| {
                eprintln!("Failed to pause playback because of empty playlist after next: {err}")
            });
        }
    }

    async fn previous(&mut self) {
        let s = self.status.lock().await;

        if s.playlist_length >= 1 {
            let cmd = if s.state != PlayState::Playing {
                "command_list_begin\nprevious\npause\ncommand_list_end"
            } else {
                "previous"
            };

            match self.mpd.request_data(cmd).await {
                Ok(_) => {}
                Err(err) => eprintln!("Failed to switch to previous song: {err}"),
            }
        } else if s.playlist_length <= 1 && s.repeat == Repeat::Off {
            self.mpd.pause().await.unwrap_or_else(|err| {
                eprintln!(
                    "Failed to pause playback because of empty playlist after previous: {err}"
                )
            })
        }
    }

    async fn pause(&mut self) {
        self.mpd.pause().await.unwrap_or_else(|err| {
            eprintln!("Failed to pause playback: {err}");
        });
    }

    async fn play_pause(&mut self) {
        self.mpd
            .toggle_play()
            .await
            .unwrap_or_else(|err| eprintln!("Failed to toggle playback: {err}"));
    }

    async fn stop(&mut self) {
        self.mpd
            .stop()
            .await
            .unwrap_or_else(|err| eprintln!("Failed to stop playback: {err}"));
    }

    async fn play(&mut self) {
        self.mpd
            .play()
            .await
            .unwrap_or_else(|err| eprintln!("Failed to start playback: {err}"));
    }

    async fn seek(
        &mut self,
        ms: i64,
        #[zbus(signal_context)] ctxt: SignalContext<'_>,
    ) -> fdo::Result<()> {
        let s = self.status.lock().await;
        let is_positive = ms > 0;
        let ms = Duration::from_micros(ms.unsigned_abs());

        if s.elapsed.unwrap_or(Duration::ZERO) + ms > s.duration.unwrap_or(Duration::MAX) {
            drop(s);
            self.next().await;
            return Ok(());
        }

        self.mpd
            .seek_relative(is_positive, ms)
            .await
            .unwrap_or_else(|err| eprintln!("Failed to seek: {err}"));

        self.seeked(
            &ctxt,
            s.elapsed.unwrap_or(Duration::ZERO).add(ms).as_micros() as u64,
        )
        .await?;

        Ok(())
    }

    async fn set_position(
        &mut self,
        track_id: ObjectPath<'_>,
        ms: i64,
        #[zbus(signal_context)] ctxt: SignalContext<'_>,
    ) -> fdo::Result<()> {
        if ms < 0 {
            return Ok(());
        }

        let pos = Duration::from_micros(ms.unsigned_abs());
        let s = self.status.lock().await;
        if pos > s.duration.unwrap_or(Duration::MAX) || s.current_song.is_none()
        //            || s.current_song.unwrap() != track_id // TODO fix current_song != ObjectPath
        {
            return Ok(());
        }

        self.mpd
            .seek(pos)
            .await
            .unwrap_or_else(|err| eprintln!("Failed to set position: {err}"));

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
    async fn set_loop_status(&mut self, loop_status: String) {
        let (repeat, single) = match loop_status.as_str() {
            "None" => (0u8, 0u8),
            "Playlist" => (1, 0),
            "Track" => (1, 1),
            _ => (0, 0),
        };

        let cmd = format!("command_list_begin\nrepeat {repeat}\nsingle {single}\ncommand_list_end");
        match self.mpd.request_data(cmd.as_str()).await {
            Ok(_) => {}
            Err(err) => eprintln!("Could not set loop status: {err}"),
        }

        self.status.lock().await.repeat = if single == 1 {
            Repeat::Single
        } else if repeat == 1 {
            Repeat::On
        } else {
            Repeat::Off
        };
    }

    #[zbus(property)]
    async fn shuffle(&self) -> bool {
        self.status.lock().await.shuffle
    }

    #[zbus(property)]
    async fn set_shuffle(&self, shuffle: bool) {
        let cmd = if shuffle { "random 1" } else { "random 0" };

        match self.mpd.request_data(cmd).await {
            Ok(_) => {}
            Err(err) => eprintln!("Could not set shuffleing: {err}"),
        }

        self.status.lock().await.shuffle = shuffle;
    }

    #[zbus(property)]
    async fn metadata(&self) -> HashMap<&str, Value> {
        let mut map = HashMap::new();
        let s = self.status.lock().await;

        if let Some(song) = &s.current_song {
            //            map.insert("mpris:trackid", );
            if let Some(duration) = s.duration {
                map.insert("mpris:length", (duration.as_micros() as u64).into());
            }
            if let Some(cover) = &song.cover {
                map.insert("xesam:artUrl", cover.clone().into());
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
            if let Some(artist) = &song.artist {
                map.insert("xesam:artist", artist.clone().into());
            }
            // TODO date
            if let Some(track) = song.track {
                map.insert("xesam:trackNumber", track.into());
            }
            map.insert("xesam:url", format!("file://{}", song.uri).into());
        }

        return map;
    }

    #[zbus(property)]
    async fn volume(&self) -> u8 {
        self.status.lock().await.volume
    }

    #[zbus(property)]
    async fn set_volume(&self, volume: u8) {
        if volume > 100 {
            return;
        }

        match self
            .mpd
            .request_data(format!("setvol {volume}").as_str())
            .await
        {
            Ok(_) => {}
            Err(err) => eprintln!("Could not set volume: {err}"),
        }

        self.status.lock().await.volume = volume;
    }

    #[zbus(property)]
    async fn position(&self) -> u64 {
        let pos = self
            .status
            .lock()
            .await
            .elapsed
            .unwrap_or(Duration::ZERO)
            .as_micros() as u64;
        return pos;
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
