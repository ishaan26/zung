//! For handleing torrent tracker requests and responses.
//!
//! The tracker is an HTTP/HTTPS service which responds to HTTP GET requests. The requests include
//! metrics from clients that help the tracker keep overall statistics about the torrent. The
//! response includes a peer list that helps the client participate in the torrent. The base URL
//! consists of the "announce URL" as defined in the metainfo (.torrent) file. The parameters are
//! then added to this URL, using standard CGI methods (i.e. a '?' after the announce URL, followed
//! by 'param=value' sequences separated by '&').

use std::net::Ipv4Addr;

use crate::{meta_info::InfoHash, PeerID};
use anyhow::{bail, Context, Result};
use serde::Serialize;

pub const UDP_PROTOCOL_ID: i64 = 0x41727101980;
pub const UDP_TRANSACTION_ID: i32 = 696969;

#[derive(Debug)]
pub enum TrackerRequest<'a> {
    Http {
        url: String,
        params: HttpTrackerRequestParams<'a>,
    },
    Udp {
        url: String,
        connection_id: i64,
        params: UdpTrackerRequestParams,
    },
}

#[derive(Debug, Serialize)]
/// The parameters used in the client->tracker GET request are as follows:
pub struct HttpTrackerRequestParams<'a> {
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

/// Offset  Size       Name       Value
/// 0       64-bit    integer    connection_id
/// 8       32-bit    integer    action          1 // announce
/// 12      32-bit    integer    transaction_id
/// 16      20-byte   string     info_hash
/// 36      20-byte   string     peer_id
/// 56      64-bit    integer    downloaded
/// 64      64-bit    integer    left
/// 72      64-bit    integer    uploaded
/// 80      32-bit    integer    event           0 // 0: none; 1: completed; 2: started; 3: stopped
/// 84      32-bit    integer    IP address      0 // default
/// 88      32-bit    integer    key
/// 92      32-bit    integer    num_want        -1 // default
/// 96      16-bit    integer    port
/// 98
#[derive(Debug)]
#[repr(C)]
pub struct UdpTrackerRequestParams {
    connection_id: i64,
    action: i32,
    transaction_id: i32,
    info_hash: [u8; 20],
    peer_id: [u8; 20],
    downloaded: i64,
    left: i64,
    uploaded: i64,
    event: Event,
    ip_address: i32,
    key: i32,
    num_want: i32,
    port: u16,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[repr(i32)]
pub enum Event {
    /// Default event
    None = 0,

    /// Must be sent to the tracker when the download completes. However, must not be sent if the
    /// download was already 100% complete when the client started. Presumably, this is to allow
    /// the tracker to increment the "completed downloads" metric based solely on this event.
    Completed = 1,

    /// The first request to the tracker must include the event key with this value.
    Started = 2,

    /// Must be sent to the tracker if the client is shutting down gracefully.
    Stopped = 3,
}

impl Event {
    pub fn from_i32(num: i32) -> Result<Self> {
        match num {
            0 => Ok(Event::None),
            1 => Ok(Event::Completed),
            2 => Ok(Event::Started),
            3 => Ok(Event::Stopped),
            num => bail!("Invalid event parameter: {num}"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(i32)]
pub enum Action {
    Connect = 0,

    Announce = 1,

    Scrape = 2,

    Error = 3,
}

impl Action {
    pub fn from_i32(num: i32) -> Result<Self> {
        match num {
            0 => Ok(Action::Connect),
            1 => Ok(Action::Announce),
            2 => Ok(Action::Scrape),
            3 => Ok(Action::Error),
            num => bail!("Invalid action parameter: {num}"),
        }
    }
}

impl<'a> TrackerRequest<'a> {
    pub(crate) fn new(url: &str, info_hash: &'a InfoHash, peer_id: PeerID) -> Self {
        let url = url.to_lowercase();
        if url.starts_with("http") {
            TrackerRequest::Http {
                url: url.to_string(),
                params: HttpTrackerRequestParams::new(info_hash, peer_id),
            }
        } else if url.starts_with("udp") {
            TrackerRequest::Udp {
                url: url.to_string(),
                connection_id: 0,
                params: UdpTrackerRequestParams::new(0, info_hash, peer_id),
            }
        } else {
            panic!("invalid tracker url in the torrent file")
        }
    }

    pub fn to_url(&self) -> Result<String> {
        match self {
            TrackerRequest::Http { url, params } => {
                let announce = url;
                let info_hash = params.info_hash.to_url_encoded();
                let peer_id = params.peer_id.to_url_encoded();
                let params = serde_urlencoded::to_string(params)?;

                Ok(format!(
                    "{announce}?info_hash={info_hash}&peer_id={peer_id}&{params}"
                ))
            }
            TrackerRequest::Udp { url, .. } => Ok(url.to_string()),
        }
    }

    pub fn set_uploaded(&mut self, uploaded: usize) {
        match self {
            TrackerRequest::Http { params, .. } => {
                params.uploaded = uploaded;
            }
            TrackerRequest::Udp { params, .. } => {
                params.uploaded = uploaded as i64;
            }
        }
    }
}

impl<'a> HttpTrackerRequestParams<'a> {
    fn new(info_hash: &'a InfoHash, peer_id: PeerID) -> Self {
        HttpTrackerRequestParams {
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
        }
    }
}

impl UdpTrackerRequestParams {
    fn new(connection_id: i64, info_hash: &InfoHash, peer_id: PeerID) -> Self {
        UdpTrackerRequestParams {
            connection_id,
            action: Action::Announce as i32, // 1 -> Announce
            transaction_id: UDP_TRANSACTION_ID,
            info_hash: info_hash.as_bytes(),
            peer_id: peer_id.as_bytes(),
            downloaded: 0,
            left: 0, // TODO: update this.
            uploaded: 0,
            event: Event::None,
            ip_address: 0,
            key: 0,
            num_want: -1,
            port: 6886,
        }
    }
}

///connect request:
/// Offset  Size            Name            Value
/// 0       64-bit integer  protocol_id     0x41727101980 // magic constant
/// 8       32-bit integer  action          0 // connect
/// 12      32-bit integer  transaction_id
/// 16
#[derive(Debug)]
#[repr(C)]
pub struct UdpConnectRequest {
    protocol_id: i64,
    action: Action,
    transaction_id: i32,
}

/// connect response:
///
/// Offset  Size            Name            Value
/// 0       32-bit integer  action          0 // connect
/// 4       32-bit integer  transaction_id
/// 8       64-bit integer  connection_id
/// 16
#[derive(Debug)]
#[repr(C)]
pub struct UdpConnectResponse {
    action: Action,
    transaction_id: i32,
    connection_id: i64,
}

impl UdpConnectRequest {
    pub(crate) fn new() -> Self {
        Self {
            protocol_id: UDP_PROTOCOL_ID,
            action: Action::Connect,
            transaction_id: UDP_TRANSACTION_ID,
        }
    }

    pub(crate) fn as_bytes(&self) -> [u8; 16] {
        let mut bytes = [0_u8; 16];

        bytes[0..8].copy_from_slice(&self.protocol_id.to_be_bytes());
        bytes[8..12].copy_from_slice(&(self.action as i32).to_be_bytes());
        bytes[12..16].copy_from_slice(&self.transaction_id.to_be_bytes());

        bytes
    }

    // TODO: Convert to async
    pub(crate) fn connect_with(udp_url: &str) -> Result<UdpConnectResponse> {
        let request = UdpConnectRequest::new();
        let request_bytes = request.as_bytes();
        let mut response = [0_u8; 16];
        let socket = std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;

        socket
            .connect(udp_url)
            .context("connecting to udp tracker")?;

        socket
            .send(&request_bytes)
            .context("Sending connect request")?;

        socket
            .recv(&mut response)
            .context("Failed to recieve any response")?;

        let udp_response = UdpConnectResponse {
            action: Action::from_i32(i32::from_be_bytes(response[0..4].try_into()?))?,
            transaction_id: i32::from_be_bytes(response[4..8].try_into()?),
            connection_id: i64::from_be_bytes(response[8..16].try_into()?),
        };

        assert_eq!(request.transaction_id, udp_response.transaction_id);

        Ok(udp_response)
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

        match tracker_request {
            TrackerRequest::Http { url, params } => {
                assert_eq!(url, url.to_string());
                assert_eq!(params.port, 6881);
                assert_eq!(params.uploaded, 0);
                assert_eq!(params.downloaded, 0);
                assert_eq!(params.left, 0);
                assert!(!params.compact);
                assert!(!params.no_peer_id);
                assert_eq!(params.event, Some(Event::Started));
                assert_eq!(params.numwant, Some(0));
            }
            TrackerRequest::Udp { .. } => {
                unreachable!("Why is http being read as upd?")
            }
        }
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
    #[test]
    fn test_bool_as_int_serialization() {
        let url = "http://example.com/announce";
        let info_hash = InfoHash::new(b"test info_hash");

        let peer_id = PeerID::default();
        let mut tracker_request = TrackerRequest::new(url, &info_hash, peer_id);

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
    #[test]
    fn test_optional_parameters() {
        let url = "http://example.com/announce";
        let info_hash = InfoHash::new(b"test info_hash");
        let peer_id = PeerID::default();
        let mut tracker_request = TrackerRequest::new(url, &info_hash, peer_id);

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
