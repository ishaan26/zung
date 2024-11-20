//! For handling torrent data sources.
//!
//! This module provides the [`DownloadSources`] enum, which categorizes sources into tracker
//! requests, HTTP seeders, or both (hybrid). It provides a unified interface for constructing
//! sources from metadata, allowing a torrent client to efficiently pull data from either or both
//! types of sources based on the information contained in the [`MetaInfo`] file.

use crate::{
    meta_info::{InfoHash, MetaInfo},
    PeerID,
};

use anyhow::Result;
use futures::stream::FuturesUnordered;
use std::sync::Arc;
use tokio::task::JoinHandle;

mod http_seeders;
mod trackers;

pub use http_seeders::{HttpSeeder, HttpSeederList};
pub use trackers::{Action, Event, Tracker, TrackerList, TrackerRequest};

/// Representing different data sources (trackers and HTTP seeders) for a torrent.
///
///
///
/// This enum is constructed with the [`sources`](crate::Client::sources) method.
#[derive(Debug, Clone)]
pub enum DownloadSources<'a> {
    /// Genarated if only `announce` or `announce_list` keys are specified in the [`MetaInfo`]
    /// file.
    Trackers { tracker_list: TrackerList },

    /// Genarated if only `url_list` key is specified in the [`MetaInfo`] file.
    HttpSeeders {
        http_seeder_list: HttpSeederList<'a>,
    },

    /// Genarated if both `announce` / `announce_list` and `url_list` keys are specified in the
    /// [`MetaInfo`] file.
    Hybrid {
        tracker_list: TrackerList,
        http_seeder_list: HttpSeederList<'a>,
    },
}

impl<'a> DownloadSources<'a> {
    pub fn new(meta_info: &'a MetaInfo) -> Self {
        fn tracker_list(meta_info: &MetaInfo) -> TrackerList {
            // As per the torrent specification, if the `announce_list` field is present, the
            // `announce` field is ignored.
            if let Some(announce_list) = meta_info.announce_list() {
                let mut tracker_list = Vec::new();
                for tracker_url in announce_list.iter().flatten() {
                    tracker_list.push(Tracker::new(tracker_url));
                }
                TrackerList::new(tracker_list)
            } else if let Some(announce) = meta_info.announce() {
                TrackerList::new(vec![Tracker::new(announce)])
            } else {
                unreachable!()
            }
        }

        fn http_seeder_list<'a>(
            url_list: &'a Vec<String>,
            meta_info: &'a MetaInfo,
        ) -> HttpSeederList<'a> {
            let mut list = Vec::with_capacity(url_list.len());
            for url in url_list {
                if !url.is_empty() {
                    list.push((url.as_str(), HttpSeeder::new(url, meta_info)));
                }
            }
            HttpSeederList::new(list)
        }

        if let Some(url_list) = meta_info.url_list() {
            if meta_info.announce.is_some() || meta_info.announce_list.is_some() {
                let http_seeder_list = http_seeder_list(url_list, meta_info);
                if http_seeder_list.is_empty() {
                    return Self::Trackers {
                        tracker_list: tracker_list(meta_info),
                    };
                }
                Self::Hybrid {
                    tracker_list: tracker_list(meta_info),
                    http_seeder_list,
                }
            } else {
                Self::HttpSeeders {
                    http_seeder_list: http_seeder_list(url_list, meta_info),
                }
            }
        } else {
            Self::Trackers {
                tracker_list: tracker_list(meta_info),
            }
        }
    }

    /// Returns a reference to the list of trackers, if available.
    ///
    /// # Example
    /// ```
    ///use zung_torrent::sources::DownloadSources;
    ///
    /// # fn ughhh(download_sources: DownloadSources) {
    /// if let Some(tracker_list) = download_sources.trackers() {
    ///     for source in tracker_list {
    ///         // Process each tracker
    ///     }
    /// } else {
    ///     println!("No trackers available for this source.");
    /// }
    /// # }
    /// ```
    pub fn trackers(&self) -> Option<&TrackerList> {
        if let Self::Trackers { tracker_list } = self {
            Some(tracker_list)
        } else if let Self::Hybrid { tracker_list, .. } = self {
            Some(tracker_list)
        } else {
            None
        }
    }

    /// Returns `true` if the download sources is [`Trackers`].
    ///
    /// [`Trackers`]: DownloadSources::Trackers
    #[must_use]
    pub fn is_trackers(&self) -> bool {
        matches!(self, Self::Trackers { .. })
    }

    /// Returns a reference to the list of http seeders, if available.
    ///
    /// # Example
    /// ```
    /// use zung_torrent::sources::DownloadSources;
    ///
    /// # fn ughhh(download_sources: DownloadSources) {
    /// if let Some(http_seeders_list) = download_sources.http_seeders() {
    ///     for source in http_seeders_list {
    ///         // Process each tracker
    ///     }
    /// } else {
    ///     println!("No http seeder available for this source.");
    /// }
    /// # }
    /// ```
    pub fn http_seeders(&self) -> Option<&HttpSeederList> {
        if let Self::HttpSeeders { http_seeder_list } = self {
            Some(http_seeder_list)
        } else if let Self::Hybrid {
            http_seeder_list, ..
        } = self
        {
            Some(http_seeder_list)
        } else {
            None
        }
    }

    /// Returns `true` if the download sources is [`HttpSeeders`].
    ///
    /// [`HttpSeeders`]: DownloadSources::HttpSeeders
    #[must_use]
    pub fn is_http_seeders(&self) -> bool {
        matches!(self, Self::HttpSeeders { .. })
    }

    /// Returns the hybrid_sources, if any, contained in the [`DownloadSources`].
    pub fn hybrid(&self) -> Option<(&TrackerList, &HttpSeederList)> {
        if let Self::Hybrid {
            tracker_list,
            http_seeder_list,
        } = self
        {
            Some((tracker_list, http_seeder_list))
        } else {
            None
        }
    }

    /// Returns `true` if the download sources is [`Hybrid`].
    ///
    /// [`Hybrid`]: DownloadSources::Hybrid
    #[must_use]
    pub fn is_hybrid(&self) -> bool {
        matches!(self, Self::Hybrid { .. })
    }

    pub fn tracker_requests(
        &self,
        info_hash: Arc<InfoHash>,
        peer_id: PeerID,
    ) -> Option<FuturesUnordered<JoinHandle<Result<TrackerRequest>>>> {
        match self {
            DownloadSources::Trackers { tracker_list }
            | DownloadSources::Hybrid { tracker_list, .. } => {
                Some(tracker_list.generate_requests(info_hash, peer_id))
            }
            DownloadSources::HttpSeeders { .. } => None,
        }
    }
}
