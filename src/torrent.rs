use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use librqbit::{AddTorrent, AddTorrentOptions, AddTorrentResponse, Session, SessionOptions};
use sha1::{Digest, Sha1};
use tracing::info;

#[derive(Clone, Debug)]
pub struct TorrentItemStats {
    pub id: usize,
    pub name: String,
    pub progress: f32,
    pub is_finished: bool,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub live_peers: usize,
}

pub struct TorrentEngine {
    session: Arc<Session>,
}

impl TorrentEngine {
    pub async fn new<P: AsRef<Path>>(download_dir: P) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let dir = download_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;

        let session = Session::new_with_opts(
            dir,
            SessionOptions {
                disable_dht: false,
                ..Default::default()
            },
        ).await?;

        Ok(Self {
            session,
        })
    }

    pub async fn add_magnet(&self, magnet: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let add_torrent = AddTorrent::from_url(magnet);
        let opts = Some(AddTorrentOptions {
            overwrite: false,
            ..Default::default()
        });

        match self.session.add_torrent(add_torrent, opts).await? {
            AddTorrentResponse::Added(id, _) => {
                info!(id, "New torrent joined the swarm (ง •̀_•́)ง");
                Ok(format!("Joined Swarm #{}", id))
            }
            AddTorrentResponse::AlreadyManaged(id, _) => {
                Ok(format!("Swarm #{} is already in queue", id))
            }
            AddTorrentResponse::ListOnly(_) => {
                Ok("Torrent metadata received".to_string())
            }
        }
    }

    pub async fn add_path_for_seeding<P: AsRef<Path>>(&self, path: P) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let path_ref = path.as_ref();
        let file_name = path_ref.file_name().and_then(|n| n.to_str()).unwrap_or("track.mp3");

        // Если это уже торрент-файл — читаем напрямую, иначе генерируем торрент-метаданные
        let torrent_bytes = if path_ref.extension().and_then(|e| e.to_str()) == Some("torrent") {
            std::fs::read(path_ref)?
        } else {
            let file_data = std::fs::read(path_ref)?;
            let file_len = file_data.len();
            let piece_len: usize = 262_144; // 256 KiB стандарт для торрентов

            let mut pieces_hashes = Vec::new();
            for chunk in file_data.chunks(piece_len) {
                let mut hasher = Sha1::new();
                hasher.update(chunk);
                pieces_hashes.extend_from_slice(&hasher.finalize());
            }

            let mut info_dict = Vec::new();
            write!(
                info_dict,
                "d6:lengthi{}e4:name{}:{}12:piece lengthi{}e6:pieces{}:",
                file_len,
                file_name.len(),
                file_name,
                piece_len,
                pieces_hashes.len()
            )?;
            info_dict.extend_from_slice(&pieces_hashes);
            info_dict.push(b'e');

            let mut torrent = Vec::new();
            write!(torrent, "d4:info")?;
            torrent.extend_from_slice(&info_dict);
            torrent.push(b'e');
            torrent
        };

        let add_torrent = AddTorrent::from_bytes(torrent_bytes);
        let opts = Some(AddTorrentOptions {
            overwrite: true,
            ..Default::default()
        });

        match self.session.add_torrent(add_torrent, opts).await? {
            AddTorrentResponse::Added(id, handle) => {
                let info_hash = handle.shared().info_hash;
                info!(id, ?path_ref, "Song successfully registered and seeding in P2P swarm! (★ω★)");
                let magnet = format!(
                    "magnet:?xt=urn:btih:{:?}&dn={}&tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337%2Fannounce&tr=udp%3A%2F%2Fopen.stealth.si%3A80%2Fannounce",
                    info_hash,
                    urlencoding_simple(file_name)
                );
                Ok(magnet)
            }
            AddTorrentResponse::AlreadyManaged(_id, handle) => {
                let info_hash = handle.shared().info_hash;
                let magnet = format!(
                    "magnet:?xt=urn:btih:{:?}&dn={}&tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337%2Fannounce&tr=udp%3A%2F%2Fopen.stealth.si%3A80%2Fannounce",
                    info_hash,
                    urlencoding_simple(file_name)
                );
                Ok(magnet)
            }
            AddTorrentResponse::ListOnly(_) => {
                Ok("Seeding metadata generated".to_string())
            }
        }
    }

    pub fn list_swarms(&self) -> Vec<TorrentItemStats> {
        self.session.with_torrents(|torrents| {
            let mut list = Vec::new();
            for (id, handle) in torrents {
                let stats = handle.stats();
                let total = stats.total_bytes;
                let progress = if total > 0 {
                    (stats.progress_bytes as f32 / total as f32).min(1.0)
                } else {
                    0.0
                };

                let name = format!("Swarm #{} ({:?})", id, handle.shared().info_hash);

                list.push(TorrentItemStats {
                    id,
                    name,
                    progress,
                    is_finished: stats.finished,
                    total_bytes: total,
                    downloaded_bytes: stats.progress_bytes,
                    live_peers: 0,
                });
            }
            list
        })
    }
}

fn urlencoding_simple(input: &str) -> String {
    let mut out = String::new();
    for b in input.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}