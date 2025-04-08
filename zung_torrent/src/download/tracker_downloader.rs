use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use bytes::BytesMut;
use dashmap::DashSet;
use futures::{stream::FuturesUnordered, StreamExt};
use tokio::{net::TcpStream, sync::Semaphore, task::JoinHandle};
use tokio_util::time::FutureExt;

use crate::{
    meta_info::InfoHashEncoded,
    peers::{BitfieldPayload, Peer, PeerMessage, PeerMessageFrame, PiecePayload},
    trackers::{Tracker, TrackersList},
    CONCURRENCY_LIMIT, TIMEOUT_DURATION,
};

use super::{DownloaderState, Uninitiated};

pub const BLOCK_MAX: u32 = 1024 * 16; /* 16 Kbi*/

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
    pub fn new(
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
    #[tracing::instrument(name = "Announce All", skip(self))]
    pub fn announce_all(self) -> TrackerDownloader<Announced> {
        let left = self.left();

        let futures: FuturesUnordered<_> = self
            .trackers
            .iter()
            .filter(|t| !t.is_connected())
            .cloned() // This performs an Arc clone on the interal type.
            .map(|tracker| -> JoinHandle<Tracker> {
                let counter = Arc::clone(&self.counters.announced);

                // TODO: Think reannouncing.
                tokio::spawn(async move {
                    // Created a separate Functions because tracing doesnot work otherwise.
                    Self::announce_to_tracker(tracker, self.info_hash, left, counter).await
                })
            })
            .collect();

        self.update_state(Announced { list: futures })
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
    list: FuturesUnordered<JoinHandle<Tracker>>,
}

impl DownloaderState for Announced {}
impl TrackerDownloaderState for Announced {}

impl TrackerDownloader<Announced> {
    #[tracing::instrument(name = "Handshake", skip_all)]
    pub async fn handshake_all(mut self) -> TrackerDownloader<Handshaken> {
        let peers_buff = DashSet::new();

        // Limit the number of outgoing requests being sent at the same time
        let semaphore = Arc::new(Semaphore::new(CONCURRENCY_LIMIT));

        // Buffer to contain thread join handles.
        let futures = FuturesUnordered::new();

        // Await on the Announced joinhandles asyncly and if successfull, handshake with the peers
        // inside the TrackerResponse.
        while let Some(tracker) = self.state.list.next().await {
            match tracker {
                Err(e) => {
                    tracing::error!("Announce thread failed: {e}");
                    continue;
                }
                Ok(announced_tracker) => {
                    if !announced_tracker.is_connected() {
                        continue;
                    }

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
                            for peer in list.iter() {
                                // If the unique peer is inserted in the hashset, spawn a thread to
                                // handshake with it.

                                if peers_buff.insert(peer.clone()) {
                                    let peer = peer.clone(); // This performs an Arc Clone
                                    let semaphore = semaphore.clone();

                                    tracing::debug!(
                                    peer = %peer.get_addr(),
                                    "Unique peer found"
                                    );

                                    tracing::info!(
                                        peer= %peer.get_addr(),
                                        "Initiating Handshake"
                                    );

                                    let counter = Arc::clone(&self.counters.handshaken);

                                    let handle = tokio::spawn(async move {
                                        let _permit = semaphore.acquire().await.unwrap();

                                        // Created a separate Functions because tracing doesnot work otherwise.
                                        Self::handshake_unique_peer(peer, self.info_hash, counter)
                                            .await
                                    });

                                    futures.push(handle);
                                }
                            }
                        }
                    }
                }
            }
        }

        self.counters
            .peers
            .store(peers_buff.len(), Ordering::Relaxed);

        self.update_state(Handshaken { list: futures })
    }

    #[tracing::instrument(
        name = "Handshake::handshake_unique_peer"
        skip_all
        fields(peer = %peer.get_addr())
    )]
    async fn handshake_unique_peer(
        peer: Peer,
        info_hash: InfoHashEncoded,
        counter: Arc<AtomicUsize>,
    ) -> Peer {
        match peer.handshake(info_hash).await {
            Ok(peer) => {
                tracing::info!("Connected to Peer");
                counter.fetch_add(1, Ordering::SeqCst);
                peer
            }
            Err(e) => {
                tracing::warn!("Unable to connect to Peer: {}", e.to_string());
                peer
            }
        }
    }
}

/////////////////////////////////////////////////////////////////////////////
//                              Handshaken Peers
/////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct Handshaken {
    list: FuturesUnordered<JoinHandle<Peer>>,
}

impl DownloaderState for Handshaken {}
impl TrackerDownloaderState for Handshaken {}

impl TrackerDownloader<Handshaken> {
    #[tracing::instrument(name = "Download::unchoke_all", skip_all)]
    pub async fn unchoke_all(mut self) -> TrackerDownloader<Downloading> {
        let handles = FuturesUnordered::new();

        while let Some(handshake_result) = self.state.list.next().await {
            match handshake_result {
                Ok(peer) => {
                    let handle = tokio::spawn(async move {
                        let addr = peer.get_addr();

                        if let Some(stream) = peer.get_stream_owned() {
                            tracing::info!(peer = %addr, "Initiating Download");

                            match Self::unchoke_peer(stream, addr).await {
                                Ok(downloading) => {
                                    tracing::info!(
                                        peer = %addr,
                                        "Download initiated successfully"
                                    );

                                    return Some(downloading);
                                }
                                Err(e) => {
                                    tracing::warn!(peer = %addr, "Unable to Download from peer: {e}");
                                    return None;
                                }
                            }
                        }
                        None
                    });

                    handles.push(handle);
                }
                Err(e) => tracing::error!("{e}"),
            }
        }

        self.update_state(Downloading { inner: handles })
    }

    // Unchockes a Peer
    #[tracing::instrument(
        name = "Download::peer_messages"
        skip(stream)
    )]
    #[inline]
    async fn unchoke_peer(
        mut stream: TcpStream,
        peer: SocketAddr,
    ) -> anyhow::Result<DownloadingPeer> {
        let mut buf = BytesMut::with_capacity(BLOCK_MAX as usize);

        tracing::debug!("Seeking bitfield message");

        stream
            .recv_peer_message::<BitfieldPayload>(&mut buf)
            .timeout(TIMEOUT_DURATION)
            .await??;

        tracing::debug!("Bitfield message received");

        buf.clear();

        tracing::debug!("Sending unchoke message");

        stream
            .send_peer_message(PeerMessage::interested())
            .timeout(TIMEOUT_DURATION)
            .await??;

        tracing::info!("Unchoke Message sent");

        tracing::debug!("Seeking unchoke message");

        stream
            .recv_unchoke_message()
            .timeout(TIMEOUT_DURATION)
            .await??;

        tracing::debug!("Unchoke message received");

        // TODO: Now comes the hard part... send request for each piece in the torrent file

        Ok(DownloadingPeer {
            peer: Peer::with_stream(peer, stream),
        })
    }
}

/////////////////////////////////////////////////////////////////////////////
//                              Downloading
/////////////////////////////////////////////////////////////////////////////

pub struct Downloading {
    inner: FuturesUnordered<JoinHandle<Option<DownloadingPeer>>>,
}

struct DownloadingPeer {
    peer: Peer,
}

impl TrackerDownloader<Downloading> {
    #[tracing::instrument(name = "Download::download_all", skip_all)]
    pub async fn download_all(mut self) -> TrackerDownloader<Downloaded> {
        let mut handles = FuturesUnordered::new();
        let counter = Arc::clone(&self.counters.downloaded);

        while let Some(future) = self.state.inner.next().await {
            match future {
                Ok(peer) => {
                    if let Some(peer) = peer {
                        let counter = Arc::clone(&counter);
                        let addr = peer.peer.get_addr();

                        if let Some(stream) = peer.peer.get_stream_owned() {
                            let handle = tokio::spawn(async move {
                                counter.fetch_add(1, Ordering::SeqCst);

                                match Self::get_piece(addr, stream).await {
                                    Ok(_) => tracing::info!("Download Compelte"),
                                    Err(e) => tracing::warn!("Unable to download: {e}"),
                                }
                            });

                            handles.push(handle);
                        }
                    }
                }
                Err(e) => tracing::error!("{e}"),
            }
        }

        while let Some(f) = handles.next().await {
            if f.is_ok() {}
        }

        self.update_state(Downloaded)
    }

    // Downloads Piece data
    #[tracing::instrument(
        name = "Download::get_piece"
        skip(stream)
    )]
    async fn get_piece(peer: SocketAddr, mut stream: TcpStream) -> anyhow::Result<()> {
        // TODO: buffer should be a disk io, and not a in memory buffer.
        let mut buf = BytesMut::with_capacity(16 * 1024);

        tracing::debug!("Sending request message");

        stream
            .send_peer_message(PeerMessage::request(0, 0, BLOCK_MAX))
            .timeout(TIMEOUT_DURATION)
            .await??;

        tracing::debug!("Request Message Sent");

        tracing::debug!("Seeking piece message");

        let piece = stream
            .recv_peer_message::<PiecePayload>(&mut buf)
            .timeout(TIMEOUT_DURATION)
            .await??;

        dbg!(piece.payload().block().len());

        tracing::debug!("Piece message received");

        Ok(())
    }
}

impl DownloaderState for Downloading {}
impl TrackerDownloaderState for Downloading {}

pub struct Downloaded;
impl DownloaderState for Downloaded {}
impl TrackerDownloaderState for Downloaded {}
