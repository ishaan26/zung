//! For handleing torrent tracker requests and responses.
//!
//! The tracker is an HTTP/HTTPS service which responds to HTTP GET requests. The requests include
//! metrics from clients that help the tracker keep overall statistics about the torrent. The
//! response includes a peer list that helps the client participate in the torrent. The base URL
//! consists of the "announce URL" as defined in the metainfo (.torrent) file. The parameters are
//! then added to this URL, using standard CGI methods (i.e. a '?' after the announce URL, followed
//! by 'param=value' sequences separated by '&').

use crate::{meta_info::InfoHash, PeerID};
use anyhow::Result;
use serde::Serialize;

#[derive(Debug)]
pub struct TrackerRequest<'a> {
    url: String,
    params: TrackerRequestParams<'a>,
}

#[derive(Debug, Serialize)]
/// The parameters used in the client->tracker GET request are as follows:
pub struct TrackerRequestParams<'a> {
    /// The info_hash calculated from the meta_info file provided to the Client.
    #[serde(skip)]
    info_hash: &'a InfoHash,

    /// PeerID of the Client.
    #[serde(skip)]
    peer_id: PeerID,

    /// The port number that the client is listening on. Ports reserved for BitTorrent are typically
    /// 6881-6889. Clients may choose to give up if it cannot establish a port within this range.
    port: u16,

    /// The total amount uploaded (since the client sent the 'started' event to the tracker) in base
    /// ten ASCII. While not explicitly stated in the official specification, the consensus is that
    /// this should be the total number of bytes uploaded.
    uploaded: usize,

    /// The total amount downloaded (since the client sent the 'started' event to the tracker) in
    /// base ten ASCII. While not explicitly stated in the official specification, the consensus is
    /// that this should be the total number of bytes downloaded.
    downloaded: usize,

    /// The number of bytes this client still has to download in base ten ASCII. Clarification: The
    /// number of bytes needed to download to be 100% complete and get all the included files in the
    /// torrent.
    left: usize,

    /// Setting this to 1 indicates that the client accepts a compact response. The peers list is
    /// replaced by a peers string with 6 bytes per peer. The first four bytes are the host (in
    /// network byte order), the last two bytes are the port (again in network byte order). It
    /// should be noted that some trackers only support compact responses (for saving bandwidth) and
    /// either refuse requests without "compact=1" or simply send a compact response unless the
    /// request contains "compact=0" (in which case they will refuse the request.)
    #[serde(serialize_with = "bool_as_int")]
    compact: bool,

    /// Indicates that the tracker can omit peer id field in peers dictionary. This option is
    /// ignored if compact is enabled.
    #[serde(serialize_with = "bool_as_int")]
    no_peer_id: bool,

    /// If specified, must be one of started, completed, stopped, (or empty which is the same as not
    /// being specified). If not specified, then this request is one performed at regular intervals.
    event: Option<Event>,

    /// The true IP address of the client machine, in dotted quad format or rfc3513 defined hexed
    /// IPv6 address.
    ///
    /// Notes: In general this parameter is not necessary as the address of the client can be
    /// determined from the IP address from which the HTTP request came. The parameter is only
    /// needed in the case where the IP address that the request came in on is not the IP address of
    /// the client. This happens if the client is communicating to the tracker through a proxy (or a
    /// transparent web proxy/cache.) It also is necessary when both the client and the tracker are
    /// on the same local side of a NAT gateway. The reason for this is that otherwise the tracker
    /// would give out the internal (RFC1918) address of the client, which is not routable.
    /// Therefore the client must explicitly state its (external, routable) IP address to be given
    /// out to external peers. Various trackers treat this parameter differently. Some only honor it
    /// only if the IP address that the request came in on is in RFC1918 space. Others honor it
    /// unconditionally, while others ignore it completely. In case of IPv6 address (e.g.:
    /// 2001:db8:1:2::100) it indicates only that client can communicate via IPv6.
    ip: Option<String>,

    /// Number of peers that the client would like to receive from the tracker. This value is
    /// permitted to be zero. If omitted, typically defaults to 50 peers.
    numwant: Option<usize>,

    /// An additional identification that is not shared with any other peers. It is intended to
    /// allow a client to prove their identity should their IP address change.
    key: Option<String>,

    /// If a previous announce contained a tracker id, it should be set here.
    #[serde(serialize_with = "serialize_tracker_id")]
    trackerid: Option<TrackerID>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Event {
    /// The first request to the tracker must include the event key with this value.
    Started,

    /// Must be sent to the tracker if the client is shutting down gracefully.
    Stopped,

    /// Must be sent to the tracker when the download completes. However, must not be sent if the
    /// download was already 100% complete when the client started. Presumably, this is to allow
    /// the tracker to increment the "completed downloads" metric based solely on this event.
    Completed,
}

/// UID associated with each tracker
#[derive(Debug, Serialize)]
pub struct TrackerID {
    id: String,
}

// Torrent Trackers want 0 or 1 for bool values
fn bool_as_int<S>(value: &bool, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_u8(if *value { 1 } else { 0 })
}

fn serialize_tracker_id<S>(value: &Option<TrackerID>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    if let Some(v) = value {
        serializer.serialize_str(&v.id)
    } else {
        serializer.serialize_none()
    }
}

impl<'a> TrackerRequest<'a> {
    pub(crate) fn new(url: &str, info_hash: &'a InfoHash, peer_id: PeerID) -> Self {
        let params = TrackerRequestParams {
            info_hash,
            peer_id,
            // TODO:: Listen on ports 6881 to 6889
            port: 6881,
            uploaded: 0,
            downloaded: 0,
            left: 0,
            compact: false,
            no_peer_id: false,
            event: Some(Event::Started),
            ip: None,
            numwant: Some(0),
            key: None,
            trackerid: None,
        };

        TrackerRequest {
            url: url.to_string(),
            params,
        }
    }

    pub fn to_url(&self) -> Result<String> {
        let announce = &self.url;
        let info_hash = self.params.info_hash.to_url_encoded();
        let peer_id = self.params.peer_id.to_url_encoded();
        let params = serde_urlencoded::to_string(&self.params)?;

        Ok(format!(
            "{announce}?info_hash={info_hash}&peer_id={peer_id}&{params}"
        ))
    }
}

#[cfg(test)]
mod tracker_tests {
    use super::*;
    use crate::meta_info::InfoHash;

    // Test creation of a new TrackerRequest with default parameters.
    #[test]
    fn test_tracker_request_creation() {
        let url = "http://example.com/announce";
        let info_hash = InfoHash::new(b"test info_hash");
        let peer_id = PeerID::default();
        let tracker_request = TrackerRequest::new(url, &info_hash, peer_id);

        assert_eq!(tracker_request.url, url.to_string());
        assert_eq!(tracker_request.params.port, 6881);
        assert_eq!(tracker_request.params.uploaded, 0);
        assert_eq!(tracker_request.params.downloaded, 0);
        assert_eq!(tracker_request.params.left, 0);
        assert!(!tracker_request.params.compact);
        assert!(!tracker_request.params.no_peer_id);
        assert_eq!(tracker_request.params.event, Some(Event::Started));
        assert_eq!(tracker_request.params.numwant, Some(0));
    }

    // Test to_url method to check if URL is correctly formatted with query parameters.
    #[test]
    fn test_tracker_request_to_url() {
        let url = "http://example.com/announce";
        let info_hash = InfoHash::new(b"test info_hash");
        let peer_id = PeerID::default();
        let tracker_request = TrackerRequest::new(url, &info_hash, peer_id);

        // Generate the URL with query parameters
        let generated_url = tracker_request.to_url().unwrap();

        // Verify that essential parts of the URL exist
        assert!(generated_url.contains("http://example.com/announce"));
        assert!(generated_url.contains(&format!("info_hash={}", info_hash.to_url_encoded())));
        assert!(generated_url.contains(&format!(
            "peer_id={}",
            tracker_request.params.peer_id.to_url_encoded()
        )));
    }

    // Test serialization of booleans as integers.
    #[test]
    fn test_bool_as_int_serialization() {
        let url = "http://example.com/announce";
        let info_hash = InfoHash::new(b"test info_hash");

        let peer_id = PeerID::default();
        let mut tracker_request = TrackerRequest::new(url, &info_hash, peer_id);

        // Set compact and no_peer_id to true to check if they serialize to 1
        tracker_request.params.compact = true;
        tracker_request.params.no_peer_id = true;

        let generated_url = tracker_request.to_url().unwrap();

        // Check that the values serialize as integers (1 for true)
        assert!(generated_url.contains("compact=1"));
        assert!(generated_url.contains("no_peer_id=1"));

        // Set them to false to check if they serialize to 0
        tracker_request.params.compact = false;
        tracker_request.params.no_peer_id = false;

        let generated_url = tracker_request.to_url().unwrap();

        // Check that the values serialize as integers (0 for false)
        assert!(generated_url.contains("compact=0"));
        assert!(generated_url.contains("no_peer_id=0"));
    }

    // Test optional parameters like IP, numwant, key, and trackerid.
    #[test]
    fn test_optional_parameters() {
        let url = "http://example.com/announce";
        let info_hash = InfoHash::new(b"test info_hash");
        let peer_id = PeerID::default();
        let mut tracker_request = TrackerRequest::new(url, &info_hash, peer_id);

        // Set optional parameters
        tracker_request.params.ip = Some("2001db81".to_string());
        tracker_request.params.numwant = Some(25);
        tracker_request.params.key = Some("unique-key".to_string());
        tracker_request.params.trackerid = Some(TrackerID {
            id: "tracker-id-123".to_string(),
        });

        let generated_url = tracker_request.to_url().unwrap();

        // Check that the optional parameters are included in the URL if provided
        assert!(generated_url.contains("ip=2001db81"));
        assert!(generated_url.contains("numwant=25"));
        assert!(generated_url.contains("key=unique-key"));
        assert!(generated_url.contains("trackerid=tracker-id-123"));
    }
}
