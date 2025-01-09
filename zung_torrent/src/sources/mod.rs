//! For handling torrent data sources.
//!
//! This module provides the [`DownloadSources`] enum, which categorizes sources into tracker
//! requests, HTTP seeders, or both (hybrid). It provides a unified interface for constructing
//! sources from metadata, allowing a torrent client to efficiently pull data from either or both
//! types of sources based on the information contained in the [`MetaInfo`] file.

use std::{net::Ipv4Addr, sync::Arc};

use crate::{
    meta_info::{InfoHashEncoded, MetaInfo},
    PeerID,
};

use colored::Colorize;
use futures::{stream::FuturesUnordered, StreamExt};

mod http_seeders;
mod trackers;

use anyhow::Result;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use tokio::{net::UdpSocket, task::JoinHandle};
use tracing::error;

pub use http_seeders::{HttpSeeder, HttpSeederList};
pub use trackers::{Action, Event, Tracker, TrackerRequest};

/// Representing different data sources (trackers and HTTP seeders) for a torrent.
///
/// This enum is constructed with the [`sources`](crate::Client::sources) method.
#[derive(Debug, Clone)]
pub enum DownloadSources<'a> {
    /// Genarated if only `announce` or `announce_list` keys are specified in the [`MetaInfo`]
    /// file.
    Trackers { tracker_list: Vec<Tracker> },

    /// Genarated if only `url_list` key is specified in the [`MetaInfo`] file.
    HttpSeeders {
        http_seeder_list: HttpSeederList<'a>,
    },

    /// Genarated if both `announce` / `announce_list` and `url_list` keys are specified in the
    /// [`MetaInfo`] file.
    Hybrid {
        tracker_list: Vec<Tracker>,
        http_seeder_list: HttpSeederList<'a>,
    },
}

impl<'a> DownloadSources<'a> {
    pub fn new(meta_info: &'a MetaInfo) -> Self {
        fn tracker_list(meta_info: &MetaInfo) -> Vec<Tracker> {
            // As per the torrent specification, if the `announce_list` field is present, the
            // `announce` field is ignored.
            match meta_info.announce_list() {
                Some(announce_list) => announce_list
                    .par_iter()
                    .flatten()
                    .map(|announce| Tracker::new(announce))
                    .collect(),
                None => match meta_info.announce() {
                    Some(announce) => vec![Tracker::new(announce)],
                    None => unreachable!(),
                },
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

        match meta_info.url_list() {
            Some(url_list) => {
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
            }
            None => Self::Trackers {
                tracker_list: tracker_list(meta_info),
            },
        }
    }

    /// Returns a reference to the list of trackers, if available.
    ///
    /// # Example
    /// ```
    ///use zung_torrent::sources::DownloadSources;
    ///
    /// # fn ughhh(download_sources: DownloadSources) {
    /// if let Some(tracker_list) = download_sources.tracker_list() {
    ///     for source in tracker_list {
    ///         // Process each tracker
    ///     }
    /// } else {
    ///     println!("No trackers available for this source.");
    /// }
    /// # }
    /// ```
    pub fn tracker_list(&self) -> Option<&Vec<Tracker>> {
        match self {
            DownloadSources::Trackers { tracker_list }
            | DownloadSources::Hybrid { tracker_list, .. } => Some(tracker_list),
            DownloadSources::HttpSeeders { .. } => None,
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
    pub fn hybrid(&self) -> Option<(&Vec<Tracker>, &HttpSeederList)> {
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

    pub async fn connect_all(&self, info_hash: InfoHashEncoded, peer_id: PeerID) {
        if let Some(list) = self.tracker_list() {
            let futures: FuturesUnordered<JoinHandle<Result<()>>> = list
                .iter()
                .cloned()
                .map(|tracker| {
                    tokio::spawn(async move {
                        let socket =
                            Arc::new(UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await.unwrap());
                        tracker.connect(socket, info_hash, peer_id).await
                    })
                })
                .collect();

            futures
                .for_each_concurrent(None, |connection| async move {
                    match connection {
                        Ok(Ok(_)) => {}
                        Ok(Err(e)) => error!("{}", e.to_string().red()),
                        Err(e) => error!("{}", e.to_string().red()),
                    }
                })
                .await;
        }
    }

    // pub async fn connect_all(
    //     &self,
    //     info_hash: InfoHashEncoded,
    //     peer_id: PeerID,
    // ) -> Option<Vec<Tracker>> {
    //     if let Some(list) = self.tracker_list() {
    //         let futures: FuturesUnordered<JoinHandle<Result<Tracker>>> = list
    //             .iter()
    //             .cloned()
    //             .map(|tracker| {
    //                 tokio::spawn(async move {
    //                     let socket =
    //                         Arc::new(UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await.unwrap());
    //                     tracker.connect(socket, info_hash, peer_id).await
    //                 })
    //             })
    //             .collect();
    //
    //         let result = Arc::new(Mutex::new(Vec::with_capacity(list.len())));
    //
    //         futures
    //             .for_each_concurrent(None, |connection| {
    //                 let result = Arc::clone(&result);
    //                 async move {
    //                     match connection {
    //                         Ok(Ok(value)) => {
    //                             info!("Connected with {}", value.url());
    //                             result.lock().expect("thread failed").push(value);
    //                         }
    //                         Ok(Err(e)) => error!("{}", e.to_string().red()),
    //                         Err(e) => error!("{}", e.to_string().red()),
    //                     }
    //                 }
    //             })
    //             .await;
    //
    //         let vec = Arc::try_unwrap(result).unwrap().into_inner().unwrap();
    //         return Some(vec);
    //     }
    //
    //     None
    // }
}
