//! For handling torrent data sources.
//!
//! This module provides the `RequestSources` enum, which categorizes sources into tracker
//! requests, HTTP seeders, or both (hybrid). It provides a unified interface for constructing
//! sources from metadata, allowing a torrent client to efficiently pull data from either or both
//! types of sources based on the information contained in the [`MetaInfo`] file.

pub mod http_seeder;
pub mod trackers;

use http_seeder::HttpSeederRequest;
use trackers::TrackerRequest;

use crate::{
    meta_info::{Files, InfoHash, MetaInfo},
    PeerID,
};

/// Representing different data sources (trackers and HTTP seeders) for a torrent.
///
///
///
/// This enum is constructed with the [`sources`](crate::Client::sources) method.
#[derive(Debug)]
pub enum DownloadSources<'a> {
    /// Genarated if only `announce` or `announce_list` keys are specified in the [`MetaInfo`] file.
    Tracker {
        tracker_request_list: Vec<TrackerRequest<'a>>,
    },

    /// Genarated if only `url_list` key is specified in the [`MetaInfo`] file.
    HttpSeeder {
        http_sources_list: Vec<HttpSeederRequest>,
    },

    /// Genarated if both `announce` / `announce_list` and `url_list` keys are specified in the
    /// [`MetaInfo`] file.
    Hybrid {
        tracker_request_list: Vec<TrackerRequest<'a>>,
        http_sources_list: Vec<HttpSeederRequest>,
    },
}

impl<'a> DownloadSources<'a> {
    pub(crate) fn new(meta_info: &'a MetaInfo, info_hash: &'a InfoHash, peer_id: PeerID) -> Self {
        if let Some(url_list) = meta_info.url_list() {
            let name = &meta_info.info().name;

            let http_sources_list = match &meta_info.info().files {
                Files::SingleFile { .. } => url_list
                    .iter()
                    .map(|s| HttpSeederRequest::new(s, None, name))
                    .collect(),
                Files::MultiFile { files } => {
                    let mut list = Vec::new();
                    for file in files {
                        for path in &file.path {
                            if let Some(attr) = &file.attr {
                                if attr.is_padding_file() {
                                    continue;
                                }
                            }

                            for url in url_list {
                                list.push(HttpSeederRequest::new(
                                    url,
                                    Some(&meta_info.info().name),
                                    path,
                                ));
                            }
                        }
                    }
                    list
                }
            };

            if let Some(announce_list) = meta_info.announce_list() {
                let tracker_request_list = announce_list
                    .iter()
                    .flatten()
                    .map(|s| TrackerRequest::new(s, info_hash, peer_id))
                    .collect();

                Self::Hybrid {
                    tracker_request_list,
                    http_sources_list,
                }
            } else if let Some(announce) = meta_info.announce() {
                let tracker_request_list = vec![TrackerRequest::new(announce, info_hash, peer_id)];
                Self::Hybrid {
                    tracker_request_list,
                    http_sources_list,
                }
            } else {
                Self::HttpSeeder { http_sources_list }
            }
        } else if let Some(announce_list) = meta_info.announce_list() {
            let tracker_request_list = announce_list
                .iter()
                .flatten()
                .map(|s| TrackerRequest::new(s, info_hash, peer_id))
                .collect();

            Self::Tracker {
                tracker_request_list,
            }
        } else if let Some(announce) = meta_info.announce() {
            let tracker_request_list = vec![TrackerRequest::new(announce, info_hash, peer_id)];
            Self::Tracker {
                tracker_request_list,
            }
        } else {
            unreachable!()
        }
    }

    /// Returns `true` if the download source is [`Tracker`].
    ///
    /// [`Tracker`]: DownloadSources::Tracker
    #[must_use]
    pub fn is_tracker(&self) -> bool {
        matches!(self, Self::Tracker { .. })
    }

    /// Returns `true` if the download sources is [`HttpSeeder`].
    ///
    /// [`HttpSeeder`]: DownloadSources::HttpSeeder
    #[must_use]
    pub fn is_http_seeder(&self) -> bool {
        matches!(self, Self::HttpSeeder { .. })
    }

    /// Returns `true` if the download sources is [`Hybrid`].
    ///
    /// [`Hybrid`]: DownloadSources::Hybrid
    #[must_use]
    pub fn is_hybrid(&self) -> bool {
        matches!(self, Self::Hybrid { .. })
    }

    /// Returns a reference to the list of tracker requests, if available.
    ///
    /// # Returns
    /// - `Some(&Vec<TrackerRequest>)` if the `DownloadSources` variant contains tracker requests.
    /// - `None` if the `DownloadSources` variant only includes HTTP seeders.
    ///
    /// # Example
    /// ```
    ///use zung_torrent::sources::DownloadSources;
    ///
    /// # fn ughhh(download_sources: DownloadSources) {
    /// if let Some(tracker_request_list) = download_sources.get_tracker_requests() {
    ///     for source in tracker_request_list {
    ///         // Process each tracker
    ///     }
    /// } else {
    ///     println!("No trackers available for this source.");
    /// }
    /// # }
    /// ```
    pub fn get_tracker_requests(&self) -> Option<&Vec<TrackerRequest<'a>>> {
        match self {
            DownloadSources::Tracker {
                tracker_request_list,
            } => Some(tracker_request_list),
            DownloadSources::HttpSeeder { .. } => None,
            DownloadSources::Hybrid {
                tracker_request_list,
                ..
            } => Some(tracker_request_list),
        }
    }

    /// Returns a reference to the list of http seeder requests, if available.
    ///
    /// # Returns
    /// - `Some(&Vec<HttpSeederRequest>)` if the `DownloadSources` variant contains http seeders
    ///    requests.
    /// - `None` if the `DownloadSources` variant only includes trackers only.
    ///
    /// # Example
    /// ```
    ///use zung_torrent::sources::DownloadSources;
    ///
    /// # fn ughhh(download_sources: DownloadSources) {
    /// if let Some(http_sources_list) = download_sources.get_http_seeders_requests() {
    ///     for source in http_sources_list {
    ///         // Process each tracker
    ///     }
    /// } else {
    ///     println!("No http seeder available for this source.");
    /// }
    /// # }
    /// ```
    pub fn get_http_seeders_requests(&self) -> Option<&Vec<HttpSeederRequest>> {
        match self {
            DownloadSources::Tracker { .. } => None,
            DownloadSources::HttpSeeder { http_sources_list } => Some(http_sources_list),
            DownloadSources::Hybrid {
                http_sources_list, ..
            } => Some(http_sources_list),
        }
    }
}
