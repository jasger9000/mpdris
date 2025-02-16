use async_std::channel::Sender;
use log::debug;
use std::mem::replace;
use std::sync::Arc;
use std::time::Duration;

use crate::config::config;

use super::MPDConnection;
use super::MPDResult;

const IMG_EXTS: [&str; 10] = ["jpg", "jpeg", "png", "webp", "avif", "jxl", "bmp", "gif", "heif", "heic"];

#[derive(Debug, Clone)]
pub struct Status {
    /// The play state of MPD. See: [PlayState]
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

#[derive(Debug, Clone)]
pub struct Song {
    pub uri: Arc<str>,
    pub cover: Option<Arc<str>>,
    pub artists: Vec<Arc<str>>,
    pub album: Option<Arc<str>>,
    pub album_artists: Vec<Arc<str>>,
    pub title: Option<Arc<str>>,
    pub track: Option<u8>,
    pub genres: Vec<Arc<str>>,
    pub date: Option<u32>,
    pub composers: Vec<Arc<str>>,
    pub comments: Vec<Arc<str>>,
    pub disc: Option<u8>,
    pub id: u32,
}

impl Eq for Song {}
impl PartialEq for Song {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Song {
    /// Creates a new empty song
    pub fn new() -> Self {
        Self {
            uri: "".into(),
            cover: None,
            artists: Vec::new(),
            album: None,
            album_artists: Vec::new(),
            title: None,
            track: None,
            genres: Vec::new(),
            date: None,
            composers: Vec::new(),
            comments: Vec::new(),
            disc: None,
            id: 0,
        }
    }

    async fn try_set_cover_url(&mut self) {
        let base = &config().read().await.music_directory;
        debug!("searching cover for '{}'", self.uri);

        let paths = {
            let covers_dir = base.join("covers").join(&*self.uri);
            let same_dir = base.join(&*self.uri);
            let cover_file = same_dir.with_file_name("cover");

            [covers_dir, same_dir, cover_file]
        };

        for mut path in paths {
            debug!("searching path '{}' for cover", path.display());

            for ext in IMG_EXTS {
                path.set_extension(ext);
                if !path.is_file() {
                    continue;
                }

                let path = path.display();
                debug!("found cover '{path}'");
                self.cover = Some(format!("file://{path}").into());
                return;
            }
        }

        debug!("no cover found");
    }

    async fn from_response(value: Vec<(String, String)>) -> Self {
        let mut song = Self::new();

        for (k, v) in value {
            match k.as_str() {
                "file" => song.uri = v.into(),
                "Artist" => song.artists.push(v.into()),
                "Album" => song.album = Some(v.into()),
                "AlbumArtist" => song.album_artists.push(v.into()),
                "Title" => song.title = Some(v.into()),
                "Track" => song.track = v.parse().ok(),
                "Genre" => song.genres.push(v.into()),
                "Date" => song.date = v.parse().ok(),
                "Composer" => song.composers.push(v.into()),
                "Comment" => song.comments.push(v.into()),
                "Disc" => song.disc = v.parse().ok(),
                "Id" => song.id = v.parse().unwrap_or(0),
                &_ => {}
            }
        }
        song.try_set_cover_url().await;

        song
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
pub async fn update_status(conn: &mut MPDConnection, status: &mut Status, sender: &Sender<StateChanged>) -> MPDResult<bool> {
    let res = conn.request_data("status").await?;
    let mut old_status = replace(status, Status::new());

    let mut is_single = false;
    let mut song_changed = false;

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
                    status.current_song = Some(Song::from_response(conn.request_data("currentsong").await?).await);
                    song_changed = true;
                } else {
                    status.current_song = old_status.current_song.take();
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
    if song_changed {
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
