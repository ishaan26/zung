//! For handling torrent tracker requests and responses.
//!
//! See the [`Tracker`] documentation for more information.

mod request;
mod response;

pub use request::*;
pub use response::*;
use tokio::io::AsyncReadExt;
use tokio::sync::Semaphore;

use super::peers::Peer;
use crate::meta_info::InfoHashEncoded;
use crate::sources::peers::{BitfieldPayload, PeerMessage};

use anyhow::{bail, Result};

use dashmap::DashSet;
use futures::{stream::FuturesUnordered, StreamExt};
use parking_lot::{Mutex, MutexGuard};
use tokio::net::UdpSocket;
use tokio::task::JoinHandle;

use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;

/////////////////////////////////////////////////////////////////////////////
//                             TRACKER URL
/////////////////////////////////////////////////////////////////////////////

/// A warpper for the url (http or udp) of a given tracker.
#[derive(Debug)]
enum TrackerUrl {
    // TODO: Look into SmallStr
    Http(Arc<str>),
    Udp(Arc<str>),
    Invalid(Arc<str>),
}

impl Clone for TrackerUrl {
    fn clone(&self) -> Self {
        match self {
            Self::Http(arg0) => Self::Http(Arc::clone(arg0)),
            Self::Udp(arg0) => Self::Udp(Arc::clone(arg0)),
            Self::Invalid(arg0) => Self::Invalid(Arc::clone(arg0)),
        }
    }
}

impl TrackerUrl {
    fn new(tracker_url: &str) -> Self {
        if tracker_url.starts_with("http") {
            Self::Http(Arc::from(tracker_url))
        } else if tracker_url.starts_with("udp") {
            Self::Udp(Arc::from(tracker_url))
        } else {
            Self::Invalid(Arc::from(tracker_url))
        }
    }

    fn as_str(&self) -> &str {
        match self {
            TrackerUrl::Http(s) => s,
            TrackerUrl::Udp(s) => s,
            TrackerUrl::Invalid(s) => s,
        }
    }
}

impl std::fmt::Display for TrackerUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/////////////////////////////////////////////////////////////////////////////
//                             SINGLE TRACKER
/////////////////////////////////////////////////////////////////////////////

/// Represents a UDP or HTTP torrent tracker.
///
/// A torrent tracker is web service which responds to HTTP GET requests or UDP requests basesd on
/// the tracker urls contained in the [`MetaInfo`](crate::MetaInfo). The requests include metrics
/// from clients that help the tracker keep overall statistics about the torrent. The response
/// includes a [peer list](super::peers::PeersList) that helps the client participate in the
/// torrent. The base URL consists of the "announce URL" as defined in the
/// [`MetaInfo`](crate::MetaInfo) (.torrent) file. The parameters are then added to this URL, using
/// standard CGI methods (i.e. a '?' after the announce URL, followed by 'param=value' sequences
/// separated by '&').
#[derive(Debug)]
pub struct Tracker {
    url: TrackerUrl,
    inner: Arc<TrackerInner>,
}

#[derive(Debug)]
struct TrackerInner {
    request: Mutex<TrackerRequest>,
    response: Mutex<TrackerResponse>,
    announced: AtomicBool,
    trys: AtomicU32,
}

impl Clone for Tracker {
    /// Clones the `Tracker` instance, sharing its inner state between clones.
    ///
    /// # Note
    ///
    /// Internally this is just an [`Arc::clone`] on the type fields.
    fn clone(&self) -> Self {
        Self {
            url: self.url.clone(),
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Tracker {
    /// Creates a new tracker with empty state.
    pub(crate) fn new(url: &str) -> Self {
        let inner = TrackerInner {
            request: Mutex::new(TrackerRequest::empty()),
            response: Mutex::new(TrackerResponse::empty()),
            announced: AtomicBool::new(false),
            trys: AtomicU32::new(0),
        };

        Self {
            url: TrackerUrl::new(url),
            inner: Arc::new(inner),
        }
    }

    /// Constructs the [`TrackerRequest`] which then allows you to
    /// [`announce`](Tracker::announce) to the subject [`Tracker`].
    ///
    /// # NOTES
    ///
    /// - *This is not intended to be used as an end user of this library*. Please see
    ///   [`announce`](Tracker::announce) method instead which internally performs this function. This
    ///   method is only useful if you are a big torrent nerd.
    ///
    /// - In case the Tracker contains a Udp url, this will perform the [`UdpConnectRequest`] (see
    ///   its documentation for more information) to obtain the `connection_id` from the tracker.
    pub async fn tracker_request(
        &self,
        info_hash: InfoHashEncoded,
        left_pieces: usize,
    ) -> Result<TrackerRequest> {
        match &self.url {
            TrackerUrl::Http(url) => Ok(TrackerRequest {
                state: TrackerRequestState::Http {
                    url: url.clone(),
                    params: HttpTrackerRequestParams::new(info_hash, left_pieces),
                },
            }),
            TrackerUrl::Udp(url) => {
                let udp_url = url.strip_prefix("udp://").unwrap();
                let udp_url = udp_url.split_once('/').map(|url| url.0).unwrap_or(udp_url);

                let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await.unwrap();

                let connection = UdpConnectRequest::new()
                    .connect_with(udp_url, &socket)
                    .await?;

                let connection_id = connection.connection_id();

                Ok(TrackerRequest {
                    state: TrackerRequestState::Udp {
                        url: Arc::clone(url),
                        connection_id,
                        socket,
                        params: UdpTrackerRequestParams::new(connection_id, info_hash, left_pieces),
                    },
                })
            }
            TrackerUrl::Invalid(url) => bail!("Unsupproted : {url}"),
        }
    }

    /// Sends an announce request to a tracker and updates its state.     
    ///
    /// This method sends a request to the tracker (via HTTP or UDP) to retrieve the response.
    /// The response is then stored in the tracker's state for later use.
    ///
    /// # Parameters
    /// - `info_hash`: The encoded info hash for the torrent.
    pub async fn announce(&self, info_hash: InfoHashEncoded, left_pieces: usize) -> Result<()> {
        self.inner.trys.fetch_add(1, Ordering::SeqCst);

        let request = self.tracker_request(info_hash, left_pieces).await?;

        let response = request.make_tracker_request().await?;
        self.inner.announced.store(true, Ordering::Relaxed);

        self.set_request(request)?;

        self.set_response(response)?;

        Ok(())
    }

    pub fn set_announced(&self, value: bool) {
        self.inner.announced.store(value, Ordering::Relaxed);
    }

    pub fn get_response_guarded(&self) -> MutexGuard<'_, TrackerResponse> {
        self.inner.response.lock()
    }

    pub fn url(&self) -> &str {
        self.url.as_str()
    }

    pub fn is_connected(&self) -> bool {
        self.inner.announced.load(Ordering::Relaxed)
    }

    pub fn trys(&self) -> u32 {
        self.inner.trys.load(Ordering::Relaxed)
    }

    fn set_request(&self, request: TrackerRequest) -> Result<()> {
        let mut guard = self.inner.request.lock();

        *guard = request;

        Ok(())
    }

    fn set_response(&self, response: TrackerResponse) -> Result<()> {
        let mut guard = self.inner.response.lock();

        *guard = response;

        Ok(())
    }
}

/////////////////////////////////////////////////////////////////////////////
//                             LIST OF TRACKERS
/////////////////////////////////////////////////////////////////////////////

/// A list of all the [`Tracker`]s contained in the [`MetaInfo`](crate::MetaInfo) file.
#[derive(Debug)]
pub struct TrackersList {
    list: Vec<Tracker>,
    num_connected: Arc<AtomicU32>,
    left_pieces: Arc<AtomicUsize>,
}

impl TrackersList {
    pub(crate) fn new(list: Vec<Tracker>, left_pieces: AtomicUsize) -> Self {
        TrackersList {
            list,
            num_connected: Arc::new(AtomicU32::new(0)),
            left_pieces: Arc::new(left_pieces),
        }
    }

    pub fn as_slice(&self) -> &[Tracker] {
        &self.list
    }

    pub fn number_of_trackers(&self) -> usize {
        self.list.len()
    }

    /// Retruns the number of connected trackers.
    pub fn number_of_connected(&self) -> u32 {
        self.num_connected.load(Ordering::Relaxed)
    }

    pub fn left_pieces(&self) -> usize {
        self.left_pieces.load(Ordering::SeqCst)
    }

    /// Returns an iterator over the lracker list.
    ///
    /// The iterator yields a reference to all [`Tracker`] (s) in the tracker list from start to
    /// end.
    pub fn iter(&self) -> TrackersListIter<'_> {
        TrackersListIter::new(self)
    }

    // TODO: Add counters
    #[tracing::instrument(name = "Announce All", skip(self, info_hash))]
    pub fn announce_all(&self, info_hash: InfoHashEncoded) -> Announced {
        let left_pieces = self.left_pieces();

        let futures: FuturesUnordered<_> = self
            .list
            .iter()
            .cloned() // This performs an Arc clone on the interal type.
            .map(|tracker| -> JoinHandle<Tracker> {
                // TODO: Think reannouncing.
                tokio::spawn(async move {
                    // Created a separate Functions because tracing doesnot work otherwise.
                    Self::announce_to_tracker(tracker, info_hash, left_pieces).await
                })
            })
            .collect();

        Announced {
            list: futures,
            info_hash,
        }
    }

    #[tracing::instrument(
        name = "Announce All::announce_to_tracker"
        skip_all
        fields(tracker=%tracker.url)
    )]
    async fn announce_to_tracker(
        tracker: Tracker,
        info_hash: InfoHashEncoded,
        left_pieces: usize,
    ) -> Tracker {
        tracing::debug!("Announcing to tracker");

        match tracker.announce(info_hash, left_pieces).await {
            Ok(_) => {
                tracing::debug!( tracker = %tracker.url, "Successfully Announced");

                tracker.set_announced(true);
                tracker
            }
            Err(e) => {
                tracing::warn!( tracker = %tracker.url, "Unable to Announce: {e}");

                tracker.set_announced(false);
                tracker
            }
        }
    }
}

/// An Iterator over the [`TrackersList`] which yeilds a reference to the [`Tracker`].
pub struct TrackersListIter<'a> {
    iter: std::slice::Iter<'a, Tracker>,
}

impl<'a> TrackersListIter<'a> {
    pub fn new(list: &'a TrackersList) -> Self {
        Self {
            iter: list.list.iter(),
        }
    }
}

impl<'a> Iterator for TrackersListIter<'a> {
    type Item = &'a Tracker;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<'a> IntoIterator for &'a TrackersList {
    type Item = &'a Tracker;
    type IntoIter = TrackersListIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        TrackersListIter::new(self)
    }
}

/////////////////////////////////////////////////////////////////////////////
//                          Announced Trackers
/////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct Announced {
    list: FuturesUnordered<JoinHandle<Tracker>>,
    info_hash: InfoHashEncoded,
}

impl Announced {
    #[tracing::instrument(name = "Handshake", skip_all)]
    pub async fn handshake_all(mut self) -> Handshaken {
        let peers_buff = DashSet::new();

        // Limit the number of outgoing requests being sent at the same time
        let semaphore = Arc::new(Semaphore::new(100));

        // Buffer to contain thread join handles.
        let futures = FuturesUnordered::new();

        // Await on the Announced joinhandles asyncly and if successfull, handshake with the peers
        // inside the TrackerResponse.
        while let Some(tracker) = self.list.next().await {
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
                                tracker_url = %announced_tracker.url,
                                "Tracker does not contain any peers"
                            );

                            continue;
                        }
                        Some(list) if list.num_of_peers() == 0 => {
                            tracing::warn!(
                                tracker_url = %announced_tracker.url,
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

        Handshaken { list: futures }
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

impl Handshaken {
    #[tracing::instrument(skip_all)]
    pub async fn download(self) {
        self.list
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

/////////////////////////////////////////////////////////////////////////////
//                              Tests
/////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tracker_tests {

    use super::*;
    use crate::meta_info::InfoHash;

    // Test creation of a new TrackerRequest with default parameters.
    #[tokio::test]
    async fn test_tracker_request_creation() {
        let sample_url = "http://example.com/announce";
        let info_hash = InfoHash::new(b"test info_hash").as_encoded();
        let tracker = Tracker::new(sample_url);
        let tracker_request = tracker.tracker_request(info_hash, 0).await.unwrap();

        match tracker_request.state() {
            TrackerRequestState::Http { url, params } => {
                assert_eq!(url.as_ref(), sample_url);
                assert_eq!(params.port, 6881);
                assert_eq!(params.uploaded, 0);
                assert_eq!(params.downloaded, 0);
                assert_eq!(params.left, 0);
                assert!(params.compact);
                assert!(!params.no_peer_id);
                assert_eq!(params.event, Some(Event::Started));
                assert_eq!(params.numwant, Some(0));
            }
            TrackerRequestState::Udp { .. } => {
                unreachable!("Why is http being read as upd?")
            }
            TrackerRequestState::Empty => {}
        }
    }

    // Test to_url method to check if URL is correctly formatted with query parameters.
    #[tokio::test]
    async fn test_tracker_request_to_url() {
        let url = "http://example.com/announce";
        let info_hash = InfoHash::new(b"test info_hash").as_encoded();
        let tracker = Tracker::new(url);
        let tracker_request = tracker.tracker_request(info_hash, 0).await.unwrap();

        // Generate the URL with query parameters
        let generated_url = tracker_request.to_url().unwrap();

        match tracker_request.state() {
            TrackerRequestState::Http { params, .. } => {
                // Verify that essential parts of the URL exist
                assert!(generated_url.contains("http://example.com/announce"));
                assert!(
                    generated_url.contains(&format!("info_hash={}", info_hash.to_url_encoded()))
                );
                assert!(
                    generated_url.contains(&format!("peer_id={}", params.peer_id.to_url_encoded()))
                );
            }
            _ => panic!(),
        }
    }

    // Test serialization of booleans as integers.
    #[tokio::test]
    async fn test_bool_as_int_serialization() {
        let url = "http://example.com/announce";
        let info_hash = InfoHash::new(b"test info_hash").as_encoded();
        let tracker = Tracker::new(url);
        let mut tracker_request = tracker.tracker_request(info_hash, 0).await.unwrap();

        match &mut tracker_request.state {
            TrackerRequestState::Http { params, .. } => {
                // Set compact and no_peer_id to true to check if they serialize to 1
                params.compact = true;
                params.no_peer_id = true;
            }
            _ => panic!(),
        }

        let generated_url = tracker_request.to_url().unwrap();

        // Check that the values serialize as integers (1 for true)
        assert!(generated_url.contains("compact=1"));
        assert!(generated_url.contains("no_peer_id=1"));

        match &mut tracker_request.state {
            TrackerRequestState::Http { params, .. } => {
                // Set them to false to check if they serialize to 0
                params.compact = false;
                params.no_peer_id = false;
            }
            _ => panic!(),
        }

        let generated_url = tracker_request.to_url().unwrap();

        // Check that the values serialize as integers (0 for false)
        assert!(generated_url.contains("compact=0"));
        assert!(generated_url.contains("no_peer_id=0"));
    }

    // Test optional parameters like IP, numwant, key, and trackerid.
    #[tokio::test]
    async fn test_optional_parameters() {
        let url = "http://example.com/announce";
        let info_hash = InfoHash::new(b"test info_hash").as_encoded();
        let tracker_request = Tracker::new(url);

        let mut tracker_request = tracker_request.tracker_request(info_hash, 0).await.unwrap();

        match &mut tracker_request.state {
            TrackerRequestState::Http { params, .. } => {
                // Set optional parameters
                params.ip = Some("2001db81".to_string());
                params.numwant = Some(25);
                params.key = Some("unique-key".to_string());
                params.trackerid = Some(TrackerID {
                    id: "tracker-id-123".to_string(),
                });

                let generated_url = tracker_request.to_url().unwrap();

                // Check that the optional parameters are included in the URL if provided
                assert!(generated_url.contains("ip=2001db81"));
                assert!(generated_url.contains("numwant=25"));
                assert!(generated_url.contains("key=unique-key"));
                assert!(generated_url.contains("trackerid=tracker-id-123"));
            }
            _ => panic!(),
        }
    }
}
