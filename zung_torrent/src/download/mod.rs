//! For torrent download management.
//!
//! It provides a unified interface for constructing sources from metadata, allowing a torrent client
//! to efficiently pull data from either or both types of sources based on the information contained
//! in the [`MetaInfo`] file.
//!
//! [`MetaInfo`]: crate::meta_info::MetaInfo

use std::sync::{atomic::AtomicUsize, Arc};

mod http_seeder_downloader;
mod sources;
mod tracker_downloader;

pub use http_seeder_downloader::HttpSeederDownloader;
pub use sources::DownloadSources;
// pub use tracker_downloader::TrackerDownloader;
pub use tracker_downloader::TrackerDownloader;
use tracker_downloader::TrackerDownloaderState;

use crate::meta_info::InfoHashEncoded;

/// Represents a download with its sources, info hash, and progress tracking.
///
/// This struct encapsulates all the information needed to manage a torrent download,
/// including the download sources (trackers, HTTP seeders, or both), the torrent's
/// info hash, and counters for tracking download progress.
#[derive(Debug)]
pub struct Download {
    sources: Arc<DownloadSources>,
    info_hash: InfoHashEncoded,
    left: Arc<AtomicUsize>,
    downloaded: Arc<AtomicUsize>,
}

impl Download {
    /// Creates a new `Download` instance.
    pub(crate) fn new(
        sources: Arc<DownloadSources>,
        info_hash: InfoHashEncoded,
        left: usize,
        downloaded: usize,
    ) -> Self {
        Self {
            sources,
            info_hash,
            left: Arc::new(AtomicUsize::new(left)),
            downloaded: Arc::new(AtomicUsize::new(downloaded)),
        }
    }

    /// Creates a new downloader for this download.
    ///
    /// Returns a `Downloader` in the `Uninitiated` state, configured based on
    /// the available download sources.
    pub fn new_downloader(&self) -> Downloader<Uninitiated> {
        match self.sources() {
            DownloadSources::Trackers { tracker_list } => {
                Downloader::TrackerDownloader(TrackerDownloader::new(
                    Arc::clone(tracker_list),
                    self.info_hash,
                    Arc::clone(&self.left),
                    Arc::clone(&self.downloaded),
                ))
            }
            // TODO: Rest of the source types
            DownloadSources::Hybrid { tracker_list, .. } => {
                tracing::error!("Httpseeders downloader not yet implemented");
                Downloader::TrackerDownloader(TrackerDownloader::new(
                    Arc::clone(tracker_list),
                    self.info_hash,
                    Arc::clone(&self.left),
                    Arc::clone(&self.downloaded),
                ))
            }
            DownloadSources::HttpSeeders { http_seeder_list } => {
                tracing::error!("Httpseeders downloader not yet implemented");
                Downloader::HttpSeederDownloader(HttpSeederDownloader {
                    _state: Uninitiated,
                    _list: Arc::clone(http_seeder_list),
                })
            }
        }
    }

    /// Returns a reference to the download sources.
    pub fn sources(&self) -> &DownloadSources {
        &self.sources
    }
}

/// Trait for representing the state of a downloader.
///
/// This trait is used as a marker for the different states a downloader can be in,
/// allowing for type-safe state transitions.
pub trait DownloaderState {}

/// Represents the initial state of a downloader before any operations have been performed.
pub struct Uninitiated;

impl DownloaderState for Uninitiated {}

/// Enum representing the different types of downloaders available.
///
/// This enum allows for polymorphic handling of different downloader types,
/// each parameterized by its current state.
pub enum Downloader<S>
where
    S: DownloaderState,
{
    /// A downloader that uses tracker-based peer discovery
    TrackerDownloader(TrackerDownloader<S>),
    /// A downloader that uses HTTP seeders
    HttpSeederDownloader(HttpSeederDownloader<S>),
}

impl Downloader<Uninitiated> {
    /// Initiates a tracker-based download process.
    ///
    /// This method performs the complete download flow for tracker-based downloads:
    /// 1. Initiates the connection
    /// 2. Announces to all trackers
    /// 3. Establishes handshakes with peers
    /// 4. Downloads the torrent data
    ///
    /// # Returns
    /// A `TrackerDownloader` in its final state after the download process,
    /// or an error if the torrent doesn't contain any trackers.
    pub async fn tracker_download(
        &self,
    ) -> anyhow::Result<TrackerDownloader<impl TrackerDownloaderState>> {
        match self {
            Downloader::TrackerDownloader(tracker_downloader) => Ok(tracker_downloader
                .initiate()
                .announce_all()
                .download_all()
                .await),

            Downloader::HttpSeederDownloader(..) => {
                Err(anyhow::anyhow!("Torrent doesnot contain any trackers"))
            }
        }
    }
}
