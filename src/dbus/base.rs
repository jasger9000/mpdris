pub struct BaseInterface;

impl BaseInterface {
    pub fn new() -> Self {
        Self {}
    }
}

#[zbus::interface(name = "org.mpris.MediaPlayer2")]
impl BaseInterface {
    async fn raise(&self) {
        // do nothing
    }

    async fn quit(&self) {
        // do nothing
    }

    #[zbus(property)]
    async fn can_quit(&self) -> bool {
        false
    }

    #[zbus(property)]
    async fn fullscreen(&self) -> bool {
        false
    }

    #[zbus(property)]
    async fn can_set_fullscreen(&self) -> bool {
        false
    }

    #[zbus(property)]
    async fn can_raise(&self) -> bool {
        false
    }

    #[zbus(property, name = "HasTrackList")]
    async fn has_tracklist(&self) -> bool {
        false // todo implement tracklist interface
    }

    #[zbus(property)]
    async fn identity(&self) -> &str {
        "Music Player Daemon"
    }

    // todo add desktop entry

    #[zbus(property)]
    async fn supported_uri_schemes(&self) -> &[&str] {
        // &["file"] todo add tracklist interface
        &[]
    }

    #[zbus(property)]
    async fn supported_mime_types(&self) -> &[&str] {
        // todo add tracklist interface
        &[]
    }
}
