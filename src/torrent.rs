use std::path::{Path, PathBuf};
use std::sync::Arc;
use librqbit::{AddTorrent, AddTorrentOptions, AddTorrentResponse, Session, SessionOptions};
use tracing::{info, warn};

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
    pub download_dir: PathBuf,
}

impl TorrentEngine {
    pub async fn new<P: AsRef<Path>>(download_dir: P) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let dir = download_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;

        let session = Session::new_with_opts(
            dir.clone(),
            SessionOptions {
                disable_dht: false,
                ..Default::default()
            },
        ).await?;

        Ok(Self {
            session,
            download_dir: dir,
        })
    }

    pub async fn add_magnet(&self, magnet: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let add_torrent = AddTorrent::from_url(magnet)?;
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
        }
    }

    pub fn list_swarms(&self) -> Vec<TorrentItemStats> {
        let mut list = Vec::new();
        self.session.with_torrents(|torrents| {
            for (id, handle) in torrents {
                let stats = handle.stats();
                let total = stats.total_bytes;
                let progress = if total > 0 {
                    (stats.progress_bytes as f32 / total as f32).min(1.0)
                } else {
                    0.0
                };

                let name = handle.with_state(|state| {
                    state.info().and_then(|i| i.name().map(|n| n.to_string()))
                }).unwrap_or_else(|| format!("Swarm #{}", id));

                list.push(TorrentItemStats {
                    id,
                    name,
                    progress,
                    is_finished: stats.finished,
                    total_bytes: total,
                    downloaded_bytes: stats.progress_bytes,
                    live_peers: stats.live_peers,
                });
            }
        });
        list
    }
}