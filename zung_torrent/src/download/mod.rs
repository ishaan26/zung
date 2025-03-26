//! For handling torrent data sources.
//!
//! This module provides the [`DownloadSources`] enum, which categorizes sources into tracker
//! requests, HTTP seeders, or both (hybrid). It provides a unified interface for constructing
//! sources from metadata, allowing a torrent client to efficiently pull data from either or both
//! types of sources based on the information contained in the [`MetaInfo`] file.

pub mod sources;
pub mod tracker;

pub trait Downloader {
    fn download_all(&self);

    fn as_mut(&mut self) -> &mut Self {
        self
    }
}
