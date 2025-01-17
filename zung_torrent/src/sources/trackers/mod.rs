//! For handleing torrent tracker requests and responses.
//!
//! See the [`Tracker`] documentation for more information.

mod request;
pub use request::*;

mod response;
pub use response::*;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use anyhow::{bail, Result};
use parking_lot::{Mutex, MutexGuard};
use tokio::net::UdpSocket;

use crate::meta_info::InfoHashEncoded;

/// For announcing to a tracker.
///
/// A torrent tracker is web service which responds to HTTP GET requests or UDP requests basesd on
/// the tracker urls contained in the [`MetaInfo`](crate::MetaInfo). The requests include
/// metrics from clients that help the tracker keep overall statistics about the torrent. The
/// response includes a peer list that helps the client participate in the torrent. The base URL
/// consists of the "announce URL" as defined in the metainfo (.torrent) file. The parameters are
/// then added to this URL, using standard CGI methods (i.e. a '?' after the announce URL, followed
/// by 'param=value' sequences separated by '&').
#[derive(Debug)]
pub struct Tracker {
    url: TrackerUrl,
    inner: Arc<TrackerInner>,
}

#[derive(Debug)]
struct TrackerInner {
    request: Mutex<TrackerRequest>,
    response: Mutex<TrackerResponse>,
    connected: AtomicBool,
    trys: AtomicU32,
}

impl Clone for Tracker {
    fn clone(&self) -> Self {
        Self {
            url: self.url.clone(),
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Tracker {
    pub fn new(url: &str) -> Self {
        let inner = TrackerInner {
            request: Mutex::new(TrackerRequest::empty()),
            response: Mutex::new(TrackerResponse::empty()),
            connected: AtomicBool::new(false),
            trys: AtomicU32::new(0),
        };

        Self {
            url: TrackerUrl::new(url),
            inner: Arc::new(inner),
        }
    }

    /// Constructs the [`TrackerRequest`].
    pub async fn tracker_request(
        &self,
        socket: Arc<UdpSocket>,
        info_hash: InfoHashEncoded,
    ) -> Result<TrackerRequest> {
        match &self.url {
            TrackerUrl::Http(url) => Ok(TrackerRequest {
                state: TrackerRequestState::Http {
                    url: url.clone(),
                    params: HttpTrackerRequestParams::new(info_hash),
                },
            }),
            TrackerUrl::Udp(url) => {
                let udp_url = url.strip_prefix("udp://").unwrap();
                let udp_url = match udp_url.split_once("/") {
                    Some(s) => s.0,
                    None => udp_url,
                };

                let connection = UdpConnectRequest::new(Arc::clone(&socket))
                    .connect_with(udp_url)
                    .await?;

                let connection_id = connection.connection_id();

                Ok(TrackerRequest {
                    state: TrackerRequestState::Udp {
                        url: Arc::clone(url),
                        connection_id,
                        socket: Arc::clone(&socket),
                        params: UdpTrackerRequestParams::new(connection_id, info_hash),
                    },
                })
            }
            TrackerUrl::Invalid(url) => bail!("Unsupproted : {url}"),
        }
    }

    pub async fn connect(&self, socket: Arc<UdpSocket>, info_hash: InfoHashEncoded) -> Result<()> {
        self.inner.trys.fetch_add(1, Ordering::SeqCst);

        let request = self.tracker_request(socket, info_hash).await?;

        // Make the HTTP or UDP request to recive a TrackerResponse
        let response = request.announce().await?;

        self.inner.connected.store(true, Ordering::Relaxed);

        self.set_request(request)?;
        self.set_response(response)?;

        Ok(())
    }

    pub fn get_response(&self) -> Result<MutexGuard<'_, TrackerResponse>> {
        let response = self.inner.response.lock();

        Ok(response)
    }

    pub fn url(&self) -> &str {
        self.url.url()
    }

    pub fn is_connected(&self) -> bool {
        self.inner.connected.load(Ordering::Relaxed)
    }

    pub fn trys(&self) -> u32 {
        self.inner.trys.load(Ordering::Relaxed)
    }

    pub(crate) fn set_request(&self, request: TrackerRequest) -> Result<()> {
        let mut guard = self.inner.request.lock();

        *guard = request;

        Ok(())
    }

    pub(crate) fn set_response(&self, response: TrackerResponse) -> Result<()> {
        let mut guard = self.inner.response.lock();

        *guard = response;

        Ok(())
    }
}

// TODO: Look into SmallStr
#[derive(Debug)]
pub enum TrackerUrl {
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
    pub fn new(tracker_url: &str) -> Self {
        if tracker_url.starts_with("http") {
            Self::Http(Arc::from(tracker_url))
        } else if tracker_url.starts_with("udp") {
            Self::Udp(Arc::from(tracker_url))
        } else {
            Self::Invalid(Arc::from(tracker_url))
        }
    }

    pub fn url(&self) -> &str {
        match self {
            TrackerUrl::Http(s) => s,
            TrackerUrl::Udp(s) => s,
            TrackerUrl::Invalid(s) => s,
        }
    }
}

#[cfg(test)]
mod tracker_tests {
    use std::net::Ipv4Addr;

    use super::*;
    use crate::meta_info::InfoHash;

    // Test creation of a new TrackerRequest with default parameters.
    #[tokio::test]
    async fn test_tracker_request_creation() {
        let sample_url = "http://example.com/announce";
        let info_hash = InfoHash::new(b"test info_hash").as_encoded();
        let socket = Arc::new(UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await.unwrap());
        let tracker = Tracker::new(sample_url);
        let tracker_request = tracker.tracker_request(socket, info_hash).await.unwrap();

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
        let socket = Arc::new(UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await.unwrap());
        let tracker = Tracker::new(url);
        let tracker_request = tracker.tracker_request(socket, info_hash).await.unwrap();

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
        let socket = Arc::new(UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await.unwrap());
        let tracker = Tracker::new(url);
        let mut tracker_request = tracker.tracker_request(socket, info_hash).await.unwrap();

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
        let socket = Arc::new(UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await.unwrap());
        let tracker_request = Tracker::new(url);

        let mut tracker_request = tracker_request
            .tracker_request(socket, info_hash)
            .await
            .unwrap();

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
