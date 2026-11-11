use id3::TagLike;
use std::io::Read;
use std::path::Path;

pub struct LoadedSong {
    pub audio_data: Vec<u8>,
    pub title: String,
    pub artist: String,
    pub cover_data: Option<Vec<u8>>,
    pub lyrics: String,
}

pub fn parse_cassette_file(path: &Path) -> Option<LoadedSong> {
    let mut mp3_bytes = Vec::new();
    let mut lyrics = String::new();
    let default_title = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown").to_string();

    if path.extension().and_then(|e| e.to_str()) == Some("zip") {
        let file = std::fs::File::open(path).ok()?;
        let mut archive = zip::ZipArchive::new(file).ok()?;

        for i in 0..archive.len() {
            if let Ok(mut item) = archive.by_index(i) {
                let name = item.name().to_lowercase();
                if name.ends_with(".mp3") {
                    let mut buf = Vec::new();
                    let _ = item.read_to_end(&mut buf);
                    mp3_bytes = buf;
                } else if name.ends_with(".txt") {
                    let mut txt = String::new();
                    let _ = item.read_to_string(&mut txt);
                    lyrics = txt;
                }
            }
        }
    } else if path.extension().and_then(|e| e.to_str()) == Some("mp3") {
        mp3_bytes = std::fs::read(path).ok()?;
    }

    if mp3_bytes.is_empty() {
        return None;
    }

    // Извлекаем ID3 теги и картинку альбома
    let mut title = default_title;
    let mut artist = "NetCassette".to_string();
    let mut cover_data = None;

    if let Ok(tag) = id3::Tag::read_from2(std::io::Cursor::new(&mp3_bytes)) {
        if let Some(t) = tag.title() {
            title = t.to_string();
        }
        if let Some(a) = tag.artist() {
            artist = a.to_string();
        }
        if let Some(picture) = tag.pictures().next() {
            cover_data = Some(picture.data.to_vec());
        }
    }

    Some(LoadedSong {
        audio_data: mp3_bytes,
        title,
        artist,
        cover_data,
        lyrics,
    })
}