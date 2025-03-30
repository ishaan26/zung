use std::sync::Arc;

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::http_seeders::{HttpSeeder, HttpSeedersList};
use crate::meta_info::MetaInfo;
use crate::trackers::{Tracker, TrackersList};

/// Representing different data sources (trackers and HTTP seeders) for a torrent.
///
/// This enum is constructed when the [`Client`](crate::Client) is initialized. A reference to it
/// can be drawn from the [`sources`](crate::Client::sources) method.
#[derive(Debug)]
pub enum DownloadSources {
    /// Genarated if only `announce` or `announce_list` keys are specified in the [`MetaInfo`]
    /// file.
    Trackers { tracker_list: Arc<TrackersList> },

    /// Genarated if only `url_list` key is specified in the [`MetaInfo`] file.
    HttpSeeders { http_seeder_list: HttpSeedersList },

    /// Genarated if both `announce` / `announce_list` and `url_list` keys are specified in the
    /// [`MetaInfo`] file.
    Hybrid {
        tracker_list: Arc<TrackersList>,
        http_seeder_list: HttpSeedersList,
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
            HttpSeedersList::new(list)
        };

        match meta_info.url_list() {
            Some(url_list) => {
                if meta_info.announce.is_some() || meta_info.announce_list.is_some() {
                    let http_seeder_list = http_seeder_list(url_list);
                    if http_seeder_list.is_empty() {
                        return Self::Trackers {
                            tracker_list: Arc::new(TrackersList::new(tracker_list)),
                        };
                    }
                    DownloadSources::Hybrid {
                        tracker_list: Arc::new(TrackersList::new(tracker_list)),
                        http_seeder_list,
                    }
                } else {
                    DownloadSources::HttpSeeders {
                        http_seeder_list: http_seeder_list(url_list),
                    }
                }
            }
            None => DownloadSources::Trackers {
                tracker_list: Arc::new(TrackersList::new(tracker_list)),
            },
        }
    }

    /// Returns a reference to the list of trackers, if available.
    ///
    /// # Example
    ///
    /// ```
    /// use zung_torrent::download::sources::DownloadSources;
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
    /// # NOTE
    ///
    /// Please note that if this method is used before performing the
    /// [`announce_all`](DownloadSources::announce_all) method, this will return the [`Tracker`] in its
    /// uninitialized state, meaning that each Tracker will have to be announced mannually.
    pub fn tracker_list(&self) -> Option<&TrackersList> {
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
        matches!(self, DownloadSources::Trackers { .. })
    }

    /// Returns a reference to the list of http seeders, if available.
    ///
    /// # Example
    /// ```
    /// use zung_torrent::download::sources::DownloadSources;
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
    pub fn http_seeders(&self) -> Option<&HttpSeedersList> {
        match &self {
            DownloadSources::HttpSeeders { http_seeder_list } => Some(http_seeder_list),
            DownloadSources::Hybrid {
                http_seeder_list, ..
            } => Some(http_seeder_list),
            _ => None,
        }
    }

    /// Returns `true` if the download sources is [`HttpSeeders`].
    ///
    /// [`HttpSeeders`]: DownloadSources::HttpSeeders
    #[must_use]
    pub fn is_http_seeders(&self) -> bool {
        matches!(self, DownloadSources::HttpSeeders { .. })
    }

    /// Returns `true` if the download sources is [`Hybrid`].
    ///
    /// [`Hybrid`]: DownloadSources::Hybrid
    #[must_use]
    pub fn is_hybrid(&self) -> bool {
        matches!(self, DownloadSources::Hybrid { .. })
    }
}
