//! For handleing torrent tracker requests and responses.
//!
//! The tracker is an HTTP/HTTPS service which responds to HTTP GET requests. The requests include
//! metrics from clients that help the tracker keep overall statistics about the torrent. The
//! response includes a peer list that helps the client participate in the torrent. The base URL
//! consists of the "announce URL" as defined in the metainfo (.torrent) file. The parameters are
//! then added to this URL, using standard CGI methods (i.e. a '?' after the announce URL, followed
//! by 'param=value' sequences separated by '&').

mod request;
pub use request::*;

use zung_parsers::bencode;

use anyhow::{anyhow, bail, Result};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use tokio::net::UdpSocket;

use crate::meta_info::InfoHashEncoded;
use crate::PeerID;

// TODO: Need inplace mutation of the tracker type, maybe the following will work??:
//struct Tracker { inner: Arc<Mutex<TrackerInner>>}
#[derive(Debug)]
pub struct Tracker {
    url: TrackerUrl,
    inner: Arc<TrackerInner>,
}

#[derive(Debug)]
struct TrackerInner {
    request: Mutex<TrackerRequest>,
    response: Mutex<bencode::Value>,
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
            request: Mutex::new(TrackerRequest::Empty),
            response: Mutex::new(bencode::Value::Integer(0)),
            connected: AtomicBool::new(false),
            trys: AtomicU32::new(0),
        };

        Self {
            url: TrackerUrl::new(url),
            inner: Arc::new(inner),
        }
    }

    pub fn url(&self) -> &str {
        self.url.url()
    }

    pub async fn connect(
        &self,
        socket: Arc<UdpSocket>,
        info_hash: InfoHashEncoded,
        peer_id: PeerID,
    ) -> Result<()> {
        // Generate Tracker request.
        // - Http => Generates a url.
        // - UDP => Sends a UDP connect request
        let request = self
            .url
            .generate_request(socket, info_hash, peer_id)
            .await?;

        // Make the HTTP or UDP request to recive a TrackerResponse
        let response = request.make_request().await?;

        self.inner.connected.store(true, Ordering::Relaxed);
        self.inner.trys.fetch_add(1, Ordering::SeqCst);

        self.set_request(request)?;
        self.set_response(response)?;

        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.inner.connected.load(Ordering::Relaxed)
    }

    pub fn trys(&self) -> u32 {
        self.inner.trys.load(Ordering::Relaxed)
    }

    pub(crate) fn set_request(&self, request: TrackerRequest) -> Result<()> {
        let mut guard = self
            .inner
            .request
            .lock()
            .map_err(|e| anyhow!("Lock poisened while setting request: {e}"))?;

        *guard = request;

        Ok(())
    }

    pub(crate) fn set_response(&self, response: bencode::Value) -> Result<()> {
        let mut guard = self
            .inner
            .response
            .lock()
            .map_err(|e| anyhow!("Lock poisened while setting response: {e}"))?;

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

    pub async fn generate_request(
        &self,
        socket: Arc<UdpSocket>,
        info_hash: InfoHashEncoded,
        peer_id: PeerID,
    ) -> Result<TrackerRequest> {
        match self {
            TrackerUrl::Http(url) => Ok(TrackerRequest::Http {
                url: url.clone(),
                params: HttpTrackerRequestParams::new(info_hash, peer_id),
            }),
            TrackerUrl::Udp(url) => {
                let udp_url = url.strip_prefix("udp://").unwrap();
                let udp_url = match udp_url.split_once("/") {
                    Some(s) => s.0,
                    None => udp_url,
                };
                let connection = UdpConnectRequest::new(socket).connect_with(udp_url).await?;

                let connection_id = connection.connection_id();
                Ok(TrackerRequest::Udp {
                    url: url.clone(),
                    connection_id,
                    params: UdpTrackerRequestParams::new(connection_id, info_hash, peer_id),
                })
            }
            TrackerUrl::Invalid(url) => bail!("Unsupproted : {url}"),
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
        let peer_id = PeerID::default();
        let socket = Arc::new(UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await.unwrap());
        let tracker_request = TrackerUrl::new(sample_url);
        let tracker_request = tracker_request
            .generate_request(socket, info_hash, peer_id)
            .await
            .unwrap();

        match tracker_request {
            TrackerRequest::Http { url, params } => {
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
            TrackerRequest::Udp { .. } => {
                unreachable!("Why is http being read as upd?")
            }
            TrackerRequest::Empty => {}
        }
    }

    // Test to_url method to check if URL is correctly formatted with query parameters.
    #[tokio::test]
    async fn test_tracker_request_to_url() {
        let url = "http://example.com/announce";
        let info_hash = InfoHash::new(b"test info_hash").as_encoded();
        let peer_id = PeerID::default();
        let socket = Arc::new(UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await.unwrap());
        let tracker_request = TrackerUrl::new(url);
        let tracker_request = tracker_request
            .generate_request(socket, info_hash, peer_id)
            .await
            .unwrap();

        // Generate the URL with query parameters
        let generated_url = tracker_request.to_url().unwrap();

        match tracker_request {
            TrackerRequest::Http { params, .. } => {
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
        let peer_id = PeerID::default();
        let tracker_request = TrackerUrl::new(url);
        let mut tracker_request = tracker_request
            .generate_request(socket, info_hash, peer_id)
            .await
            .unwrap();

        match &mut tracker_request {
            TrackerRequest::Http { params, .. } => {
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

        match &mut tracker_request {
            TrackerRequest::Http { params, .. } => {
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
        let peer_id = PeerID::default();
        let tracker_request = TrackerUrl::new(url);
        let mut tracker_request = tracker_request
            .generate_request(socket, info_hash, peer_id)
            .await
            .unwrap();

        match &mut tracker_request {
            TrackerRequest::Http { params, .. } => {
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
