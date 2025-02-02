use async_std::{channel::Sender, fs::metadata};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::config::config;

use super::MPDResult;
use super::MpdConnection;

#[derive(Debug, Clone)]
pub struct Status {
    /// The play state of MPD. See: [State]
    pub state: PlayState,
    /// The Volume MPD outputs in percent
    pub volume: u8,
    /// Repeat behaviour of MPD. See: [Repeat]
    pub repeat: Repeat,
    /// If shuffling is turned on
    pub shuffle: bool,
    /// elapsed time of the current song, or None if no song selected
    pub elapsed: Option<Duration>,
    /// Duration of the current song, or None if no song selected
    pub duration: Option<Duration>,
    /// The currently playing song
    pub current_song: Option<Song>,
    /// The id of the song that is going to be played after [Self::current_song]
    pub next_song: Option<u32>,
    /// The length of the current playlist/tracklist
    pub playlist_length: u32,
}

impl Status {
    pub fn new() -> Self {
        Self {
            state: PlayState::Paused,
            volume: 100,
            repeat: Repeat::Off,
            shuffle: false,
            elapsed: None,
            duration: None,
            current_song: None,
            next_song: None,
            playlist_length: 0,
        }
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum PlayState {
    Playing,
    Paused,
    Stopped,
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum Repeat {
    Off = 0,
    On = 1,
    Single = 2,
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Song {
    pub uri: Arc<str>,
    pub cover: Option<Arc<str>>,
    pub artists: Option<Vec<Arc<str>>>,
    pub album: Option<Arc<str>>,
    pub album_artists: Option<Vec<Arc<str>>>,
    pub title: Option<Arc<str>>,
    pub track: Option<u8>,
    pub genre: Option<Vec<Arc<str>>>,
    pub date: Option<u32>,
    pub composer: Option<Vec<Arc<str>>>,
    pub comment: Option<Vec<Arc<str>>>,
    pub disc: Option<u8>,
    pub id: u32,
}

impl Song {
    /// Creates a new empty song
    pub fn new() -> Self {
        Self {
            uri: "".into(),
            cover: None,
            artists: None,
            album: None,
            album_artists: None,
            title: None,
            track: None,
            genre: None,
            date: None,
            composer: None,
            comment: None,
            disc: None,
            id: 0,
        }
    }

    pub async fn try_set_cover_url(&mut self) {
        let base: &str = &config().read().await.music_directory;

        let covers: PathBuf = [base, "covers", &self.uri].iter().collect();
        let uri: PathBuf = [base, &self.uri].iter().collect();

        async fn check_path(path: &Path) -> Option<Arc<str>> {
            let img_exts = ["jpg", "jpeg", "png", "webp", "avif", "jxl", "bmp", "gif", "heif", "heic"];

            for ext in img_exts {
                let path = path.with_extension(ext);
                if metadata(&path).await.is_ok() {
                    return Some(format!("file://{}", path.to_str()?).into());
                }
            }
            None
        }

        self.cover = if let Some(path) = check_path(&covers).await {
            Some(path)
        } else if let Some(path) = check_path(&uri).await {
            Some(path)
        } else {
            check_path(&uri.with_file_name("cover")).await
        };
    }
}

impl From<Vec<(String, String)>> for Song {
    fn from(value: Vec<(String, String)>) -> Self {
        let mut song = Self::new();

        for (k, v) in value {
            match k.as_str() {
                "file" => song.uri = v.into(),
                "Artist" => add_to_vec(&mut song.artists, v),
                "Album" => song.album = Some(v.into()),
                "AlbumArtist" => add_to_vec(&mut song.album_artists, v),
                "Title" => song.title = Some(v.into()),
                "Track" => song.track = v.parse().ok(),
                "Genre" => add_to_vec(&mut song.genre, v),
                "Date" => song.date = v.parse().ok(),
                "Composer" => add_to_vec(&mut song.composer, v),
                "Comment" => add_to_vec(&mut song.comment, v),
                "Disc" => song.disc = v.parse().ok(),
                "Id" => song.id = v.parse().unwrap_or(0),
                &_ => {}
            }
        }
        song.try_set_cover_url();

        song
    }
}

/// Pushes value to the vec if it is some or creates a new Vec with value
fn add_to_vec<T, V: Into<T>>(vec: &mut Option<Vec<T>>, value: V) {
    if let Some(vec) = vec {
        vec.push(value.into());
    } else {
        *vec = Some(vec![value.into()]);
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum StateChanged {
    Position(i64),
    Song(bool, bool),
    Playlist,
    PlayState,
    Volume,
    Repeat,
    Shuffle,
}

/// Updates the given status with new information from MPD gathered from the given connection.
/// Returns a boolean if the query was successful, or the Error variant
/// if there was an error with the communication with MPD.
/// Boolean is true when MPD was previously playing and still is, and the current song hasn't changed
pub async fn update_status(conn: &mut MpdConnection, status: &mut Status, sender: &Sender<StateChanged>) -> MPDResult<bool> {
    let res = conn.request_data("status").await?;

    let old_status = status.clone();
    *status = Status::new();

    let mut is_single = false;

    for (k, v) in res {
        match k.as_str() {
            "state" => match v.as_str() {
                "play" => status.state = PlayState::Playing,
                "pause" => status.state = PlayState::Paused,
                "stop" => status.state = PlayState::Stopped,
                _ => {}
            },
            "single" => {
                if v.parse().unwrap_or(0) > 0 {
                    is_single = true;
                }
            }
            "repeat" => {
                if v.parse().unwrap_or(0) > 0 {
                    status.repeat = Repeat::On;
                } else {
                    status.repeat = Repeat::Off;
                }
            }
            "duration" => {
                if let Ok(secs) = v.parse::<f64>() {
                    status.duration = Some(Duration::from_secs_f64(secs));
                } else {
                    status.duration = None;
                }
            }
            "elapsed" => {
                if let Ok(secs) = v.parse::<f64>() {
                    status.elapsed = Some(Duration::from_secs_f64(secs));
                } else {
                    status.elapsed = None;
                }
            }
            "songid" => {
                let id = v.parse().unwrap_or(u32::MAX);
                let old_id = old_status.current_song.as_ref().map_or_else(|| u32::MIN, |s| s.id);

                if id != old_id {
                    status.current_song = Some(conn.request_data("currentsong").await?.into());
                } else {
                    status.current_song = old_status.current_song.clone();
                }
            }
            "volume" => status.volume = v.parse().unwrap_or(0),
            "random" => status.shuffle = v.parse().unwrap_or(0) > 0,
            "nextsongid" => status.next_song = v.parse().ok(),
            "playlistlength" => status.playlist_length = v.parse().unwrap_or(0),
            _ => {}
        }
    }

    if is_single {
        status.repeat = Repeat::Single;
    }

    if old_status.state != PlayState::Playing && status.state != PlayState::Playing && old_status.elapsed != status.elapsed {
        #[rustfmt::skip]
        sender.send(StateChanged::Position(status.elapsed.unwrap().as_micros() as i64)).await.unwrap();
    }
    if old_status.state != status.state {
        sender.send(StateChanged::PlayState).await.unwrap();
    }
    if old_status.volume != status.volume {
        sender.send(StateChanged::Volume).await.unwrap();
    }
    if old_status.repeat != status.repeat {
        sender.send(StateChanged::Repeat).await.unwrap();
    }
    if old_status.shuffle != status.shuffle {
        sender.send(StateChanged::Shuffle).await.unwrap();
    }
    if old_status.current_song != status.current_song {
        let prev = old_status.playlist_length != status.playlist_length
            && ((status.playlist_length < 1) != (old_status.playlist_length < 1));
        let next = old_status.next_song != status.next_song;
        sender.send(StateChanged::Song(prev, next)).await.unwrap();
    }
    if old_status.next_song.is_some() != status.next_song.is_some() || old_status.playlist_length != status.playlist_length {
        sender.send(StateChanged::Playlist).await.unwrap();
    }

    let could_be_seeking = old_status.current_song == status.current_song
        && old_status.state == status.state
        && status.state == PlayState::Playing;
    Ok(could_be_seeking)
}
