//! For handling torrent data sources.
//!
//! This module provides the [`DownloadSources`] enum, which categorizes sources into tracker
//! requests, HTTP seeders, or both (hybrid). It provides a unified interface for constructing
//! sources from metadata, allowing a torrent client to efficiently pull data from either or both
//! types of sources based on the information contained in the [`MetaInfo`] file.

use std::sync::Arc;

use sources::DownloadSources;
use tracker::TrackerDownloader;

use crate::meta_info::{InfoHashEncoded, MetaInfo};

pub mod sources;
pub mod tracker;

#[async_trait::async_trait]
pub trait Downloader {
    async fn download_all(self, left_pieces: usize);

    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

#[derive(Debug)]
pub struct Download {
    sources: Arc<DownloadSources>,
    info_hash: InfoHashEncoded,
    // TODO: Come back to the Arc
    _meta_info: Arc<MetaInfo>,
}

impl Download {
    pub fn new(
        sources: Arc<DownloadSources>,
        info_hash: InfoHashEncoded,
        meta_info: Arc<MetaInfo>,
    ) -> Self {
        Self {
            sources,
            info_hash,
            _meta_info: meta_info,
        }
    }

    pub fn downloader(&self) -> impl Downloader {
        match self.sources() {
            DownloadSources::Trackers { tracker_list } => {
                TrackerDownloader::new(Arc::clone(tracker_list), self.info_hash)
            }
            // TODO: Rest of the source types
            _ => todo!(),
        }
    }

    pub fn sources(&self) -> &DownloadSources {
        &self.sources
    }
}
