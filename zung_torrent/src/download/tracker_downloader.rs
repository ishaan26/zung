use std::{
    net::SocketAddr,
    sync::{atomic::AtomicUsize, Arc},
};

use bytes::BytesMut;
use dashmap::DashSet;
use futures::{stream::FuturesUnordered, StreamExt};
use tokio::{net::TcpStream, sync::Semaphore, task::JoinHandle};
use tokio_util::time::FutureExt;

use crate::{
    meta_info::InfoHashEncoded,
    peers::{BitfieldPayload, Peer, PeerMessage, PeerMessageFrame},
    trackers::{Tracker, TrackersList},
    TIMEOUT_DURATION,
};

use super::{DownloaderState, Uninitiated};

pub struct TrackerDownloader<T> {
    state: T,
    trackers: Arc<TrackersList>,
    info_hash: InfoHashEncoded,
    left: Arc<AtomicUsize>,
    downloaded: Arc<AtomicUsize>,
}

impl<T> TrackerDownloader<T>
where
    T: DownloaderState,
{
    fn update_state<N>(self, state: N) -> TrackerDownloader<N> {
        TrackerDownloader {
            state,
            trackers: self.trackers,
            info_hash: self.info_hash,
            left: self.left,
            downloaded: self.downloaded,
        }
    }

    pub fn left(&self) -> usize {
        self.left.load(std::sync::atomic::Ordering::SeqCst)
    }
}

pub trait TrackerDownloaderState: DownloaderState {}

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
        }
    }

    pub fn initiate(&self) -> TrackerDownloader<UnAnnounced> {
        TrackerDownloader {
            state: UnAnnounced,
            trackers: Arc::clone(&self.trackers),
            info_hash: self.info_hash,
            left: Arc::clone(&self.left),
            downloaded: Arc::clone(&self.downloaded),
        }
    }
}

/////////////////////////////////////////////////////////////////////////////
//                          UnAnnounced Trackers
/////////////////////////////////////////////////////////////////////////////

impl TrackerDownloader<UnAnnounced> {
    // TODO: Add counters
    #[tracing::instrument(name = "Announce All", skip(self))]
    pub fn announce_all(self) -> TrackerDownloader<Announced> {
        let left = self.left();

        let futures: FuturesUnordered<_> = self
            .trackers
            .iter()
            .filter(|t| !t.is_connected())
            .cloned() // This performs an Arc clone on the interal type.
            .map(|tracker| -> JoinHandle<Tracker> {
                // TODO: Think reannouncing.
                tokio::spawn(async move {
                    // Created a separate Functions because tracing doesnot work otherwise.
                    Self::announce_to_tracker(tracker, self.info_hash, left).await
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
    ) -> Tracker {
        tracing::debug!("Announcing to tracker");

        match tracker.announce(info_hash, left_pieces).await {
            Ok(_) => {
                tracing::debug!( tracker = %tracker.url(), "Successfully Announced");

                tracker.set_announced(true);
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

impl TrackerDownloader<Announced> {
    #[tracing::instrument(name = "Handshake", skip_all)]
    pub async fn handshake_all(mut self) -> TrackerDownloader<Handshaken> {
        let peers_buff = DashSet::new();

        // Limit the number of outgoing requests being sent at the same time
        let semaphore = Arc::new(Semaphore::new(100));

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

                                    let handle = tokio::spawn(async move {
                                        let _permit = semaphore.acquire().await.unwrap();

                                        // Created a separate Functions because tracing doesnot work otherwise.
                                        Self::handshake_unique_peer(peer, self.info_hash).await
                                    });

                                    futures.push(handle);
                                }
                            }
                        }
                    }
                }
            }
        }

        self.update_state(Handshaken { list: futures })
    }

    #[tracing::instrument(
        name = "Handshake::handshake_unique_peer"
        skip(info_hash)
        fields(peer = %peer.get_addr())
    )]
    async fn handshake_unique_peer(peer: Peer, info_hash: InfoHashEncoded) -> Peer {
        match peer.handshake(info_hash).await {
            Ok(peer) => {
                tracing::info!("Connected to Peer");
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
    #[tracing::instrument(name = "Download::download_all", skip_all)]
    pub async fn download_all(mut self) -> TrackerDownloader<TrackerDownload> {
        let mut handles = FuturesUnordered::new();

        while let Some(handshake_result) = self.state.list.next().await {
            match handshake_result {
                Ok(mut peer) => {
                    let handle = tokio::spawn(async move {
                        let addr = peer.get_addr();
                        if let Some(stream) = peer.get_stream_mut() {
                            tracing::info!(peer = %addr, "Initiating Download");

                            match Self::run_peer_messages(stream, addr).await {
                                Ok(_) => {
                                    tracing::info!(peer = %peer.get_addr(), "Download initiated successfully")
                                }
                                Err(e) => {
                                    tracing::warn!(peer = %addr, "Unable to Download from peer: {e}")
                                }
                            };
                        }
                        peer
                    });
                    handles.push(handle);
                }
                Err(e) => tracing::error!("{e}"),
            }
        }

        while let Some(future) = handles.next().await {
            if future.is_ok() {}
        }

        self.update_state(TrackerDownload)
    }

    #[tracing::instrument(
        name = "Download::peer_messages"
        skip(stream)
    )]
    #[inline]
    async fn run_peer_messages(
        stream: &mut TcpStream,
        peer_addr: SocketAddr,
    ) -> anyhow::Result<()> {
        let mut buf = BytesMut::with_capacity(40960);

        tracing::debug!("Seeking bitfield message");

        stream
            .recv_peer_message::<BitfieldPayload>(&mut buf)
            .timeout(TIMEOUT_DURATION)
            .await??;

        tracing::debug!("Bitfield message received");

        drop(buf);

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

        Ok(())
    }
}

pub struct TrackerDownload;

impl DownloaderState for TrackerDownload {}
impl TrackerDownloaderState for TrackerDownload {}
