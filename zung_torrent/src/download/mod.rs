//! For handling torrent data sources.
//!
//! This module provides the [`DownloadSources`] enum, which categorizes sources into tracker
//! requests, HTTP seeders, or both (hybrid). It provides a unified interface for constructing
//! sources from metadata, allowing a torrent client to efficiently pull data from either or both
//! types of sources based on the information contained in the [`MetaInfo`] file.

use std::sync::{atomic::AtomicUsize, Arc};

mod http_seeder_downloader;
mod sources;
mod tracker_downloader;

pub use http_seeder_downloader::HttpSeederDownloader;
pub use sources::DownloadSources;
pub use tracker_downloader::TrackerDownloader;
use tracker_downloader::TrackerDownloaderState;

use crate::meta_info::InfoHashEncoded;

// TODO: Come back to the Arc
#[derive(Debug)]
pub struct Download {
    sources: Arc<DownloadSources>,
    info_hash: InfoHashEncoded,
    left: Arc<AtomicUsize>,
    downloaded: Arc<AtomicUsize>,
}

impl Download {
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

    pub fn sources(&self) -> &DownloadSources {
        &self.sources
    }
}

pub trait DownloaderState {}

pub struct Uninitiated;

impl DownloaderState for Uninitiated {}

pub enum Downloader<S>
where
    S: DownloaderState,
{
    TrackerDownloader(TrackerDownloader<S>),
    HttpSeederDownloader(HttpSeederDownloader<S>),
}

impl Downloader<Uninitiated> {
    pub async fn tracker_download(
        &self,
    ) -> anyhow::Result<TrackerDownloader<impl TrackerDownloaderState>> {
        match self {
            Downloader::TrackerDownloader(tracker_downloader) => Ok(tracker_downloader
                .initiate()
                .announce_all()
                .handshake_all()
                .await
                .download_all()
                .await),

            Downloader::HttpSeederDownloader(..) => {
                Err(anyhow::anyhow!("Torrent doesnot contain any trackers"))
            }
        }
    }
}
