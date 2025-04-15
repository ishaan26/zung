use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use dashmap::DashSet;
use futures::{stream::FuturesUnordered, StreamExt};
use tokio::sync::{
    mpsc::{self, Receiver},
    Semaphore,
};

use crate::{
    meta_info::InfoHashEncoded,
    trackers::{Tracker, TrackersList},
    CONCURRENCY_LIMIT,
};

use super::{DownloaderState, Uninitiated};

/// A state machine that manages the process of downloading from trackers in a BitTorrent client.
///
/// This struct transitions through different states during the download process:
/// - `Uninitiated`: Initial state before any tracker communication
/// - `UnAnnounced`: Ready to announce to trackers
/// - `Announced`: Successfully announced to trackers
/// - `Handshaken`: Successfully handshaken with peers
/// - `TrackerDownload`: Actively downloading from peers
pub struct TrackerDownloader<T> {
    state: T,
    trackers: Arc<TrackersList>,
    info_hash: InfoHashEncoded,
    left: Arc<AtomicUsize>,
    downloaded: Arc<AtomicUsize>,
    counters: Counters,
}

impl<T> TrackerDownloader<T>
where
    T: DownloaderState,
{
    /// Updates the state of the tracker downloader, transitioning to a new state.
    fn update_state<N>(self, state: N) -> TrackerDownloader<N>
    where
        N: TrackerDownloaderState,
    {
        TrackerDownloader {
            state,
            trackers: self.trackers,
            info_hash: self.info_hash,
            left: self.left,
            downloaded: self.downloaded,
            counters: self.counters,
        }
    }

    /// Returns the number of bytes left to download.
    pub fn left(&self) -> usize {
        self.left.load(Ordering::Relaxed)
    }

    /// Returns the number of trackers that have been successfully announced to.
    pub fn announced_count(&self) -> usize {
        self.counters.announced.load(Ordering::Relaxed)
    }

    /// Returns the number of unique peers discovered from trackers.
    pub fn unique_peers_count(&self) -> usize {
        self.counters.peers.load(Ordering::Relaxed)
    }

    /// Returns the number of peers that have been successfully handshaken with.
    pub fn handshaken_count(&self) -> usize {
        self.counters.handshaken.load(Ordering::Relaxed)
    }

    /// Returns the number of peers that are actively being downloaded from.
    pub fn downloaded_count(&self) -> usize {
        self.counters.downloaded.load(Ordering::Relaxed)
    }
}

/// Trait that represents a state in the tracker downloader state machine.
///
/// This trait is implemented by all states in the [`TrackerDownloader`] state machine and serves
/// as a marker trait that extends the base `DownloaderState` trait. It helps to distinguish states
/// that are specific to the tracker downloading process.
pub trait TrackerDownloaderState: DownloaderState {}

struct Counters {
    announced: Arc<AtomicUsize>,
    handshaken: Arc<AtomicUsize>,
    peers: Arc<AtomicUsize>,
    downloaded: Arc<AtomicUsize>,
}

impl Counters {
    fn new() -> Counters {
        Counters {
            announced: Arc::new(AtomicUsize::new(0)),
            handshaken: Arc::new(AtomicUsize::new(0)),
            peers: Arc::new(AtomicUsize::new(0)),
            downloaded: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl Clone for Counters {
    fn clone(&self) -> Self {
        Self {
            announced: self.announced.clone(),
            handshaken: self.handshaken.clone(),
            peers: self.peers.clone(),
            downloaded: self.downloaded.clone(),
        }
    }
}

/////////////////////////////////////////////////////////////////////////////
//                                Uninitiated State
/////////////////////////////////////////////////////////////////////////////

impl TrackerDownloader<Uninitiated> {
    /// Creates a new [`TrackerDownloader`] in the [`Uninitiated`] state.
    ///
    /// # Parameters
    ///
    /// * `trackers` - A thread-safe reference to the list of trackers to download from
    /// * `info_hash` - The encoded info hash of the torrent
    /// * `left` - A thread-safe counter for the number of bytes left to download
    /// * `downloaded` - A thread-safe counter for the number of bytes already downloaded
    ///
    /// # Returns
    ///
    /// A new `TrackerDownloader` in the `Uninitiated` state
    pub(crate) fn new(
        trackers: Arc<TrackersList>,
        info_hash: InfoHashEncoded,
        left: Arc<AtomicUsize>,
        downloaded: Arc<AtomicUsize>,
    ) -> TrackerDownloader<Uninitiated> {
        TrackerDownloader {
            state: Uninitiated,
            trackers,
            info_hash,
            left,
            downloaded,
            counters: Counters::new(),
        }
    }
}

impl TrackerDownloader<Uninitiated> {
    /// Transitions the downloader from the `Uninitiated` state to the `UnAnnounced` state.
    ///
    /// This method prepares the downloader to announce to trackers by creating a new
    /// `TrackerDownloader` instance in the `UnAnnounced` state, while preserving all
    /// the internal state and counters.
    ///
    /// # Returns
    ///
    /// A new `TrackerDownloader` in the `UnAnnounced` state
    pub fn initiate(&self) -> TrackerDownloader<UnAnnounced> {
        TrackerDownloader {
            state: UnAnnounced,
            trackers: Arc::clone(&self.trackers),
            info_hash: self.info_hash,
            left: Arc::clone(&self.left),
            downloaded: Arc::clone(&self.downloaded),
            counters: self.counters.clone(),
        }
    }
}

/////////////////////////////////////////////////////////////////////////////
//                          UnAnnounced Trackers
/////////////////////////////////////////////////////////////////////////////

impl TrackerDownloader<UnAnnounced> {
    /// Announces to all trackers in the tracker list that haven't been announced to yet.
    ///
    /// This method initiates the announcement process to all unconnected trackers in parallel.
    /// For each tracker, it spawns a separate task that attempts to announce the client's presence
    /// and interest in downloading the torrent. Successfully announced trackers will have their
    /// connection state updated.
    ///
    /// # Returns
    ///
    /// A new `TrackerDownloader` in the `Announced` state, which contains a channel receiver
    /// that will receive the results of the announcement attempts.
    ///
    /// # State Transition
    ///
    /// This method transitions the downloader from the `UnAnnounced` state to the `Announced` state.
    #[tracing::instrument(name = "Announce All", skip(self))]
    pub fn announce_all(self) -> TrackerDownloader<Announced> {
        let left = self.left();

        let (tx, rx) = mpsc::channel(1024);

        let trackers = self.trackers.iter().filter(|t| !t.is_connected()).cloned(); // This performs an Arc clone on the interal type.

        for tracker in trackers {
            let counter = Arc::clone(&self.counters.announced);
            let tx = tx.clone();
            tokio::spawn(async move {
                tx.send(Self::announce_to_tracker(tracker, self.info_hash, left, counter).await)
                    .await
                    .unwrap();
            });
        }

        self.update_state(Announced { stream: rx })
    }

    #[tracing::instrument(
        name = "Announce All::announce_to_tracker"
        skip_all
        fields(tracker=%tracker.url())
    )]
    async fn announce_to_tracker(
        tracker: Tracker,
        info_hash: InfoHashEncoded,
        left_pieces: usize,
        counter: Arc<AtomicUsize>,
    ) -> Tracker {
        tracing::debug!("Announcing to tracker");

        match tracker.announce(info_hash, left_pieces).await {
            Ok(_) => {
                tracing::debug!( tracker = %tracker.url(), "Successfully Announced");

                tracker.set_announced(true);
                counter.fetch_add(1, Ordering::SeqCst);
                tracker
            }
            Err(e) => {
                tracing::warn!( tracker = %tracker.url(), "Unable to Announce: {e}");

                tracker.set_announced(false);
                tracker
            }
        }
    }
}

/// Represents the state where the tracker downloader is ready to announce to trackers.
///
/// This is the state after initialization but before any announcements have been made to trackers.
/// From this state, the downloader can transition to the `Announced` state by calling
/// [`TrackerDownloader::announce_all()`].
pub struct UnAnnounced;

impl DownloaderState for UnAnnounced {}
impl TrackerDownloaderState for UnAnnounced {}

/////////////////////////////////////////////////////////////////////////////
//                          Announced Trackers
/////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct Announced {
    stream: Receiver<Tracker>,
}

impl DownloaderState for Announced {}
impl TrackerDownloaderState for Announced {}

impl TrackerDownloader<Announced> {
    #[tracing::instrument(name = "Handshake", skip_all)]
    /// Initiates the download process from all announced trackers.
    ///
    /// This method processes the announced trackers and attempts to download from their peers.
    /// For each unique peer discovered from the trackers:
    /// 1. It establishes a handshake connection
    /// 2. Exchanges BitTorrent protocol messages
    /// 3. Requests and downloads pieces
    ///
    /// The method uses a semaphore to limit concurrent connections to [`crate::CONCURRENCY_LIMIT`]
    /// and tracks unique peers to avoid duplicate connections.
    ///
    /// # Returns
    ///
    /// A new `TrackerDownloader` in the `Downloaded` state, indicating that the download
    /// process has been completed or attempted for all available peers.
    ///
    /// # State Transition
    ///
    /// This method transitions the downloader from the `Announced` state to the `Downloaded` state.
    pub async fn download_all(mut self) -> TrackerDownloader<Downloaded> {
        let peers_buff = DashSet::new();

        // Limit the number of outgoing requests being sent at the same time
        let semaphore = Arc::new(Semaphore::new(CONCURRENCY_LIMIT));

        let mut handles = FuturesUnordered::new();

        // Await on the Announced joinhandles asyncly and if successfull, handshake with the peers
        // inside the TrackerResponse.
        while let Some(announced_tracker) = self.state.stream.recv().await {
            let guraded_response = announced_tracker.get_response_guarded();
            let peers_list = guraded_response.get_peers_list();

            match peers_list {
                None => {
                    tracing::warn!(
                        tracker_url = %announced_tracker.url(),
                        "Tracker does not contain any peers"
                    );

                    continue;
                }

                Some(list) if list.num_of_peers() == 0 => {
                    tracing::warn!(
                        tracker_url = %announced_tracker.url(),
                        "Tracker sent an empty peers list"
                    );

                    continue;
                }

                Some(list) => {
                    let counter = Arc::clone(&self.counters.handshaken);
                    for peer in list.iter() {
                        // If the unique peer is inserted in the hashset, spawn a thread to
                        // handshake with it.

                        if peers_buff.insert(peer.clone()) {
                            let peer = peer.clone(); // This performs an Arc Clone
                            let semaphore = semaphore.clone();

                            tracing::debug!(
                            peer = %peer.socket_addr(),
                            "Unique peer found"
                            );

                            tracing::info!(
                                peer= %peer.socket_addr(),
                                "Initiating Handshake"
                            );

                            let counter = Arc::clone(&counter);

                            let handle = tokio::spawn(async move {
                                let _permit = semaphore.acquire().await.unwrap();

                                // Unchoke the peer
                                let peer_unchoked =
                                    peer.handshake(self.info_hash).await?.unchoke().await?;

                                counter.fetch_add(1, Ordering::SeqCst);

                                let addr = peer_unchoked.socket_addr();

                                // Download the peer
                                match peer_unchoked.download_piece().await {
                                    Ok(d) => {
                                        tracing::info!(
                                            peer = %addr,
                                            "Download complete for piece_index: {}",
                                            d.get_downloaded_piece().index()
                                        )
                                    }
                                    Err(e) => {
                                        tracing::error!(peer= %addr, "Unable to download: {e}")
                                    }
                                }

                                Ok::<(), anyhow::Error>(())
                            });

                            handles.push(handle);
                        }
                    }
                }
            };
        }

        while let Some(handle) = handles.next().await {
            match handle {
                Ok(result) => {
                    if let Err(e) = result {
                        tracing::warn!("Peer download task failed: {e}");
                    }
                }
                Err(e) => tracing::error!("Peer download task panicked: {e}"),
            }
        }

        self.counters
            .peers
            .store(peers_buff.len(), Ordering::Relaxed);

        self.update_state(Downloaded)
    }
}

pub struct Downloaded;
impl DownloaderState for Downloaded {}
impl TrackerDownloaderState for Downloaded {}
