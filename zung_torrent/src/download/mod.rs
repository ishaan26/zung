//! For handling torrent data sources.
//!
//! This module provides the [`DownloadSources`] enum, which categorizes sources into tracker
//! requests, HTTP seeders, or both (hybrid). It provides a unified interface for constructing
//! sources from metadata, allowing a torrent client to efficiently pull data from either or both
//! types of sources based on the information contained in the [`MetaInfo`] file.

use std::sync::{atomic::AtomicUsize, Arc};

mod sources;
mod tracker_downloader;

pub use sources::DownloadSources;
pub use tracker_downloader::TrackerDownloader;

use crate::meta_info::InfoHashEncoded;

#[async_trait::async_trait]
pub trait Downloader {
    async fn download_all(self);

    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

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

    pub fn downloader(&self) -> impl Downloader {
        match self.sources() {
            DownloadSources::Trackers { tracker_list } => TrackerDownloader::new(
                Arc::clone(tracker_list),
                self.info_hash,
                Arc::clone(&self.left),
                Arc::clone(&self.downloaded),
            ),
            // TODO: Rest of the source types
            DownloadSources::Hybrid { tracker_list, .. } => {
                tracing::error!("Httpseeders downloader not yet implemented");
                TrackerDownloader::new(
                    Arc::clone(tracker_list),
                    self.info_hash,
                    Arc::clone(&self.left),
                    Arc::clone(&self.downloaded),
                )
            }
            DownloadSources::HttpSeeders { .. } => todo!(),
        }
    }

    pub fn sources(&self) -> &DownloadSources {
        &self.sources
    }
}
