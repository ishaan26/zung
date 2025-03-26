use std::sync::Arc;

use dashmap::DashSet;
use futures::{stream::FuturesUnordered, StreamExt};
use tokio::{io::AsyncReadExt, sync::Semaphore, task::JoinHandle};

use crate::{
    meta_info::InfoHashEncoded,
    peers::{BitfieldPayload, Peer, PeerMessage},
    trackers::{Tracker, TrackersList},
};

use super::Downloader;

pub struct TrackerDownloader<T = UnAnnounced> {
    state: T,
    trackers: Arc<TrackersList>,
    info_hash: InfoHashEncoded,
}

impl TrackerDownloader {
    pub fn new(
        trackers: Arc<TrackersList>,
        info_hash: InfoHashEncoded,
    ) -> TrackerDownloader<UnAnnounced> {
        TrackerDownloader {
            state: UnAnnounced,
            trackers,
            info_hash,
        }
    }

    // TODO: Add counters
    #[tracing::instrument(name = "Announce All", skip(self))]
    pub fn announce_all(self, left_pieces: usize) -> TrackerDownloader<Announced> {
        let futures: FuturesUnordered<_> = self
            .trackers
            .iter()
            .cloned() // This performs an Arc clone on the interal type.
            .map(|tracker| -> JoinHandle<Tracker> {
                // TODO: Think reannouncing.
                tokio::spawn(async move {
                    // Created a separate Functions because tracing doesnot work otherwise.
                    Self::announce_to_tracker(tracker, self.info_hash, left_pieces).await
                })
            })
            .collect();

        TrackerDownloader {
            state: Announced { list: futures },
            trackers: self.trackers,
            info_hash: self.info_hash,
        }
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

/////////////////////////////////////////////////////////////////////////////
//                          Announced Trackers
/////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct Announced {
    list: FuturesUnordered<JoinHandle<Tracker>>,
}

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

        TrackerDownloader {
            state: Handshaken { list: futures },
            trackers: self.trackers,
            info_hash: self.info_hash,
        }
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

impl TrackerDownloader<Handshaken> {
    #[tracing::instrument(skip_all)]
    pub async fn download_all(self) {
        self.state
            .list
            .for_each_concurrent(None, async |peer| match peer {
                Ok(mut peer) => {
                    let addr = peer.get_addr();
                    if let Some(stream) = peer.get_stream_mut() {
                        tracing::info!(peer = %addr, "Initiating Download");

                        let mut recv_bitfield = [0_u8; 409600];
                        let read = stream.read(&mut recv_bitfield).await.unwrap();
                        dbg!(read);
                        if read == 0 {
                            tracing::debug!("Empty Message sent");
                        } else {
                            match PeerMessage::<BitfieldPayload>::from_bytes(&recv_bitfield[..read])
                            {
                                Ok(m) => println!("REC: {m:?}"),
                                Err(e) => tracing::warn!("{e}"),
                            }
                        }
                    }
                }
                Err(e) => tracing::error!("{e}"),
            })
            .await;
    }
}

#[async_trait::async_trait]
impl Downloader for TrackerDownloader<UnAnnounced> {
    async fn download_all(self, left_pieces: usize) {
        self.announce_all(left_pieces)
            .handshake_all()
            .await
            .download_all()
            .await
    }
}
