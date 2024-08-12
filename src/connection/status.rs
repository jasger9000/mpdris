use std::sync::mpsc::Sender;
use std::time::Duration;

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
    pub uri: String,
    pub cover: Option<String>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub title: Option<String>,
    pub album: Option<String>,
    pub track: Option<u8>,
    pub date: Option<u32>,
    pub id: u32,
}

impl Song {
    /// Creates a new empty song
    pub fn new() -> Self {
        Self {
            uri: String::new(),
            cover: None, // TODO cover
            artist: None,
            album_artist: None,
            title: None,
            album: None,
            track: None,
            date: None,
            id: 0,
        }
    }
}

impl From<Vec<(String, String)>> for Song {
    fn from(value: Vec<(String, String)>) -> Self {
        let mut song = Self::new();

        for (k, v) in value {
            match k.as_str() {
                "file" => song.uri = v,
                "Artist" => song.artist = Some(v),
                "AlbumArtist" => song.artist = Some(v),
                "Title" => song.title = Some(v),
                "Album" => song.album = Some(v),
                "Track" => song.track = v.parse().ok(),
                "Date" => song.date = v.parse().ok(),
                "Id" => song.id = v.parse().unwrap_or(0),
                &_ => {}
            }
        }

        return song;
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum StateChanged {
    Position(u64),
    Song,
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
pub async fn update_status(
    conn: &mut MpdConnection,
    status: &mut Status,
    sender: &Sender<StateChanged>,
) -> MPDResult<bool> {
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
                let old_id = old_status
                    .current_song
                    .as_ref()
                    .map_or_else(|| u32::MIN, |s| s.id);

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
    
    // TODO check if current, prev & next id are still valid
    if old_status.state != PlayState::Playing && status.state != PlayState::Playing &&
        old_status.elapsed != status.elapsed
    {
        sender.send(StateChanged::Position(status.elapsed.unwrap().as_micros() as u64)).unwrap();
    } else if old_status.state != status.state {
        sender.send(StateChanged::PlayState).unwrap();
    } else if old_status.volume != status.volume {
        sender.send(StateChanged::Volume).unwrap();
    } else if old_status.repeat != status.repeat {
        sender.send(StateChanged::Repeat).unwrap();
    } else if old_status.shuffle != status.shuffle {
        sender.send(StateChanged::Shuffle).unwrap();
    } else if old_status.current_song != status.current_song {
        sender.send(StateChanged::Song).unwrap();
    } else if old_status.next_song.is_some() != status.next_song.is_some()
        || old_status.playlist_length != status.playlist_length
    {
        sender.send(StateChanged::Playlist).unwrap();
    }
   
    let could_be_seeking = old_status.current_song == status.current_song && old_status.state == status.state && status.state == PlayState::Playing;
    Ok(could_be_seeking)
}
