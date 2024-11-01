use audiotags::AudioTag;
use once_cell::sync::Lazy;
use rhai::{CustomType, TypeBuilder};

static NULL_CHAR: once_cell::sync::Lazy<String> = Lazy::new(|| String::from_utf8(vec![0]).unwrap());

#[derive(Debug, Clone, sqlx::Type, Default)]
#[repr(i64)]
pub enum FileState {
    #[default]
    Copied,
    Copying,
    Unknown,
}

impl From<i64> for FileState {
    fn from(value: i64) -> Self {
        match value {
            0 => Self::Copied,
            1 => Self::Copying,
            _ => Self::Unknown,
        }
    }
}

pub struct RawTrack {
    pub tags: Box<dyn AudioTag + Send + Sync>,
    pub path: String,
}

#[derive(Debug, Clone, Default, rhai::CustomType)]
pub struct BaseTrack {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub number: i64,
    pub file_path: String,
    pub disc_number: i64,
    pub disc_total: i64,
    pub extension: String,
}

impl From<Track> for BaseTrack {
    fn from(value: Track) -> Self {
        Self {
            title: value.title,
            artist: value.artist,
            album: value.album,
            number: value.number,
            file_path: value.file_path,
            disc_number: value.disc_number,
            disc_total: value.disc_total,
            extension: value.extension,
        }
    }
}

#[derive(Debug, Clone, sqlx::Type, Default)]
pub struct Track {
    pub id: i64,
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub number: i64,
    pub file_path: String,
    pub disc_number: i64,
    pub disc_total: i64,
    pub file_state: FileState,
    pub extension: String,
}

impl std::fmt::Display for Track {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} - {},  {}", self.title, self.album, self.artist)
    }
}

impl Track {
    pub fn storage_path(&self, base: &str) -> String {
        let mut p = std::path::PathBuf::new();

        let extension = std::path::Path::new(&self.file_path)
            .extension()
            .unwrap_or_default()
            .to_str()
            .unwrap();

        let filename = format!("{}.{}", self.title, extension);

        p.push(base);
        p.push(clean(self.artist.clone(), false));
        p.push(clean(self.album.clone(), false));
        p.push(clean(self.disc_number.to_string(), false));
        p.push(clean(filename, true));

        p.to_str().unwrap().to_string()
    }
}

impl From<RawTrack> for Track {
    fn from(track: RawTrack) -> Self {
        let disc = track.tags.disc();

        // If no album artist has been found, use the artist tag.
        // If that's missing too, we have an Unknown album.
        let artist = match track.tags.album_artist() {
            Some(aa) => aa,
            None => track.tags.artist().unwrap_or("Unknown Album"),
        };

        let mut t = Self {
            id: 0,
            track_id: Default::default(),
            title: track.tags.title().unwrap_or("Unknown Title").to_owned(),
            artist: artist.to_owned(),
            album: track
                .tags
                .album_title()
                .unwrap_or("Unknown Album")
                .to_owned(),
            number: track.tags.track_number().unwrap_or_default() as i64,
            file_path: track.path,
            disc_number: disc.0.unwrap_or_default() as i64,
            disc_total: disc.1.unwrap_or_default() as i64,
            file_state: FileState::Unknown,
            extension: String::new(),
        };

        t.track_id = track_hash(&t);

        let extension = std::path::Path::new(&t.file_path)
            .extension()
            .unwrap_or(std::ffi::OsStr::new("NONE"))
            .to_str()
            .unwrap()
            .to_string();

        t.extension = extension;

        t
    }
}

fn track_hash(track: &Track) -> String {
    let mut sb = string_builder::Builder::default();

    sb.append(track.artist.clone());
    sb.append(track.album.clone());
    sb.append(track.title.clone());
    sb.append(track.extension.clone());

    sha256::digest(sb.string().unwrap())
}

fn clean(s: String, is_file: bool) -> String {
    let mut s = s.clone();

    for c in [
        r#"""#,
        std::path::MAIN_SEPARATOR_STR,
        "*",
        "/",
        ":",
        "<",
        ">",
        "?",
        r#"\"#,
        "|",
        "+",
        ",",
        {
            if !is_file {
                "."
            } else {
                ""
            }
        },
        ";",
        "=",
        "[",
        "]",
        &NULL_CHAR,
    ] {
        if c != "" {
            s = s.replace(c, "_")
        }
    }

    s
}

pub struct Album {
    pub title: String,
    pub artist: String,
    pub format: String,
}
