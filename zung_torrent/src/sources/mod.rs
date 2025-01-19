//! For handling torrent data sources.
//!
//! This module provides the [`DownloadSources`] enum, which categorizes sources into tracker
//! requests, HTTP seeders, or both (hybrid). It provides a unified interface for constructing
//! sources from metadata, allowing a torrent client to efficiently pull data from either or both
//! types of sources based on the information contained in the [`MetaInfo`] file.

pub mod http_seeders;
pub mod peers;
pub mod trackers;

use std::collections::HashSet;

use colored::Colorize;
use futures::{stream::FuturesUnordered, StreamExt};

use anyhow::Result;
use peers::Peer;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use tokio::task::JoinHandle;
use tracing::{info, instrument, warn};

use crate::meta_info::{InfoHashEncoded, MetaInfo};
use http_seeders::{HttpSeeder, HttpSeederList};
use trackers::Tracker;

/// Representing different data sources (trackers and HTTP seeders) for a torrent.
///
/// This enum is constructed when the [`Client`](crate::Client) is initialized. A reference to it
/// can be drawn from the [`sources`](crate::Client::sources) method.
#[derive(Debug, Clone)]
pub enum DownloadSources {
    /// Genarated if only `announce` or `announce_list` keys are specified in the [`MetaInfo`]
    /// file.
    Trackers { tracker_list: Vec<Tracker> },

    /// Genarated if only `url_list` key is specified in the [`MetaInfo`] file.
    HttpSeeders { http_seeder_list: HttpSeederList },

    /// Genarated if both `announce` / `announce_list` and `url_list` keys are specified in the
    /// [`MetaInfo`] file.
    Hybrid {
        tracker_list: Vec<Tracker>,
        http_seeder_list: HttpSeederList,
    },
}

impl DownloadSources {
    /// Creates a new [`DownloadSources`] from the provided [`MetaInfo`] file.
    ///
    /// IMPORTANT NOTE:
    ///
    /// This type is created and stored in the [`Client`](crate::Client) type and the recommended
    /// way to access this is by using the [`sources`](crate::Client::sources) method.
    /// But you do you! :).
    pub fn new(meta_info: &MetaInfo) -> Self {
        let tracker_list = match meta_info.announce_list() {
            Some(announce_list) => announce_list
                .par_iter()
                .flatten()
                .map(|announce| Tracker::new(announce))
                .collect(),
            None => match meta_info.announce() {
                Some(announce) => vec![Tracker::new(announce)],
                None => Vec::new(),
            },
        };

        let http_seeder_list = |url_list: &[String]| {
            let mut list = Vec::with_capacity(url_list.len());
            for url in url_list {
                if !url.is_empty() {
                    list.push((url.to_owned(), HttpSeeder::new(url, meta_info)));
                }
            }
            HttpSeederList::new(list)
        };

        match meta_info.url_list() {
            Some(url_list) => {
                if meta_info.announce.is_some() || meta_info.announce_list.is_some() {
                    let http_seeder_list = http_seeder_list(url_list);
                    if http_seeder_list.is_empty() {
                        return Self::Trackers { tracker_list };
                    }
                    Self::Hybrid {
                        tracker_list,
                        http_seeder_list,
                    }
                } else {
                    Self::HttpSeeders {
                        http_seeder_list: http_seeder_list(url_list),
                    }
                }
            }
            None => Self::Trackers { tracker_list },
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
    ///     for tracker in tracker_list {
    ///         // Process each tracker
    ///     }
    /// } else {
    ///     println!("No trackers available for this source.");
    /// }
    /// # }
    /// ```
    ///
    /// # NOTE:
    ///
    /// Please note that if this method is used before performing the
    /// [`announce_all`](DownloadSources::announce_all) method, this will return the [`Tracker`] in its
    /// uninitialized state, meaning that each Tracker will have to be announced mannually.
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
    ///     for seeder in http_seeders_list.iter() {
    ///         // Process each seeder
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

    /// Announce to all trackers in the torrent
    #[instrument(skip_all)]
    pub async fn announce_all(&self, info_hash: InfoHashEncoded) {
        if let Some(list) = self.tracker_list() {
            let futures: FuturesUnordered<JoinHandle<Result<Tracker>>> = list
                .iter()
                .cloned() // This performs an arc clone on the interal type.
                .map(|tracker| -> JoinHandle<Result<Tracker>> {
                    tokio::spawn(async move {
                        for _ in 0..10 {
                            match tracker.announce(info_hash).await {
                                Ok(_) => return Ok(tracker),
                                Err(e) => {
                                    warn!("{}: {}", tracker.url().italic(), e.to_string());
                                    continue;
                                }
                            }
                        }
                        Ok(tracker)
                    })
                })
                .collect();

            futures
                .for_each_concurrent(None, |connection| async move {
                    match connection {
                        Ok(Ok(t)) => info!("Connected! {}", t.url()),
                        Ok(Err(e)) => warn!("{}", e.to_string()),
                        Err(e) => warn!("{}", e.to_string()),
                    }
                })
                .await;
        }
    }

    pub async fn announce_all_loop(&self, info_hash: InfoHashEncoded) {
        self.announce_all(info_hash).await;

        let mut futures: FuturesUnordered<_> = (0..10)
            .map(|i| async move {
                dbg!(i);
                self.announce_all(info_hash).await;
            })
            .collect();

        while futures.next().await.is_some() {}
    }

    pub async fn retry_connect_all(&self, info_hash: InfoHashEncoded) {
        if let Some(list) = self.tracker_list() {
            let mut handles = Vec::new();
            for tracker in list {
                if !tracker.is_connected() {
                    let tracker = tracker.clone();
                    let handle = tokio::spawn(async move {
                        let mut i = 0;
                        loop {
                            i += 1;

                            match tracker.announce(info_hash).await {
                                Ok(_) => {
                                    info!("Connected to : {}", tracker.url());
                                    break;
                                }
                                Err(_) => {
                                    if i < 10 {
                                        continue;
                                    } else {
                                        break;
                                    }
                                }
                            }
                        }
                    });

                    handles.push(handle);
                }
            }

            tokio::spawn(async {
                futures::future::join_all(handles).await;
            })
            .await
            .unwrap();
        }
    }

    pub fn peers_list(&self) -> HashSet<Peer> {
        match self {
            DownloadSources::Trackers { tracker_list }
            | DownloadSources::Hybrid { tracker_list, .. } => {
                let mut list = HashSet::new();

                for tracker in tracker_list {
                    if let Some(peers) = tracker.get_response_guarded().get_peers_list() {
                        for peer in peers {
                            if list.insert(peer.clone()) {
                                peer.set_connected();
                            }
                        }
                    }
                }

                list
            }
            DownloadSources::HttpSeeders { .. } => HashSet::new(),
        }
    }
}
