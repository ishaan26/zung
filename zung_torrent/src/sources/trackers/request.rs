use std::sync::Arc;

use anyhow::{anyhow, bail, ensure, Context, Result};
use bytes::{BufMut, BytesMut};
use serde::Serialize;

use tokio::net::UdpSocket;
use tokio::time::timeout;
use tokio_util::time::FutureExt;
use tracing::{debug, instrument};
use zung_parsers::bencode;

use crate::client::PEER_ID;
use crate::meta_info::InfoHashEncoded;
use crate::sources::trackers::{HttpTrackerResponse, TrackerResponseState, UdpTrackerResponse};
use crate::{PeerID, TIMEOUT_DURATION};

use super::TrackerResponse;

pub const UDP_PROTOCOL_ID: i64 = 0x41727101980; // Magic torrent number. DNC!
pub const UDP_TRANSACTION_ID: i32 = 696969;
const MIN_UDP_RESPONSE_SIZE: usize = 20;

/// Represents different types of BitTorrent tracker requests.
///
/// This enum encapsulates the various types of requests that can be made to BitTorrent trackers,
/// supporting both HTTP and UDP protocols, as well as an empty state.
///
/// # Variants
///
/// * `Http` - Represents an HTTP tracker request with a URL and associated parameters
/// * `Udp` - Represents a UDP tracker request with connection details and parameters
/// * `Empty` - Represents an empty or uninitialized tracker request state
///
///# Notes
///
/// - The `url` fields are stored as `Arc<str>` for efficient sharing and minimal memory overhead
/// - UDP requests maintain their own socket connection through `Arc<UdpSocket>`
/// - The `Empty` variant can be used as a placeholder or to represent an invalid state
#[derive(Debug)]
pub struct TrackerRequest {
    pub(crate) state: TrackerRequestState,
}

#[derive(Debug)]
pub(crate) enum TrackerRequestState {
    /// HTTP tracker request variant.
    /// Contains the tracker's URL and request parameters.
    Http {
        /// The announce URL of the HTTP tracker.
        url: Arc<str>,

        params: HttpTrackerRequestParams,
    },

    /// UDP tracker request variant.
    /// Contains connection details and request parameters for UDP trackers.
    Udp {
        /// The announce URL of the UDP tracker.
        url: Arc<str>,

        /// Connection ID received from the UDP tracker during connection phase using
        /// [UdpConnectRequest]
        connection_id: i64,

        /// Shared UDP socket for communication with the tracker
        socket: UdpSocket,

        /// UDP-specific parameters for the tracker request based on the (torrent
        /// spec)[https://www.bittorrent.org/beps/bep_0015.html]
        params: UdpTrackerRequestParams,
    },

    /// Represents an empty or uninitialized tracker request.
    /// Can be used as a placeholder or to represent an invalid state.
    Empty,
}

impl TrackerRequest {
    pub fn empty() -> Self {
        Self {
            state: TrackerRequestState::Empty,
        }
    }

    pub(crate) fn state(&self) -> &TrackerRequestState {
        &self.state
    }

    /// Returns the `connection_id` (if) recieved from a UDP tracker.
    pub fn connection_id(&self) -> Option<i64> {
        if let TrackerRequestState::Udp { connection_id, .. } = self.state() {
            Some(*connection_id)
        } else {
            None
        }
    }

    /// Convert to url string for making a get request.
    pub fn to_url(&self) -> Result<String> {
        match &self.state() {
            TrackerRequestState::Http { url, params } => {
                let announce = url;
                let info_hash = params.info_hash.to_url_encoded();
                let peer_id = params.peer_id.to_url_encoded();
                let params = serde_urlencoded::to_string(params)?;

                Ok(format!(
                    "{announce}?info_hash={info_hash}&peer_id={peer_id}&{params}"
                ))
            }
            TrackerRequestState::Udp { url, .. } => Ok(url.to_string()),
            TrackerRequestState::Empty => Err(anyhow!("Cannot convert Empty request to url")),
        }
    }

    pub fn set_uploaded(&mut self, uploaded: usize) {
        match &mut self.state {
            TrackerRequestState::Http { params, .. } => {
                params.uploaded = uploaded;
            }
            TrackerRequestState::Udp { params, .. } => {
                params.uploaded = uploaded as i64;
            }
            TrackerRequestState::Empty => {}
        }
    }

    /// Makes the Tracker request and retunrs the Tracker Response.
    #[instrument(skip_all, name = "Tracker Request")]
    pub(crate) async fn make_announce_request(&self) -> Result<TrackerResponse> {
        match &self.state() {
            // HTTP request wherein response is recieved as a bencode dictionary.
            TrackerRequestState::Http { .. } => {
                let url = self.to_url()?;
                let request = timeout(TIMEOUT_DURATION, reqwest::get(&url))
                    .await
                    .with_context(|| format!("Connection Timed Out: {url}"))?
                    .context(format!("Failed to connect: {url}"))?;

                debug!(url, "Request made successfully");

                let response = request.bytes().await?;

                // let response_ohter: bencode::Value = bencode::from_bytes(&response)?;
                // println!("{response_ohter}");

                let mut response: HttpTrackerResponse = bencode::from_bytes(&response)?;
                debug!(url, "Received response");

                let peers = response.peers.take().map(Arc::new);

                Ok(TrackerResponse {
                    state: TrackerResponseState::Http(response),
                    peers,
                })
            }

            // UDP Request wherein response is recieved as binary data.
            TrackerRequestState::Udp {
                socket,
                params,
                url,
                ..
            } => {
                let mut response = Vec::with_capacity(4096);

                let request_bytes = params.to_bytes();

                timeout(TIMEOUT_DURATION, socket.send(&request_bytes))
                    .await
                    .with_context(|| format!("Send Timed Out: {url}"))?
                    .context("Sending connect request")?;

                debug!("UDP Tracker Request Sent");

                let (rec, socket) = timeout(TIMEOUT_DURATION, socket.recv_buf_from(&mut response))
                    .await
                    .with_context(|| format!("Recieve Timed Out: {url}"))?
                    .context(format!("Failed to recieve any response: {url}"))?;

                if rec < MIN_UDP_RESPONSE_SIZE {
                    bail!("Invalid or No response recieved: {url}")
                }

                debug!("UDP Tracker Response Recieved");

                let mut response = UdpTrackerResponse::from_bytes(&response[..rec], socket)?;

                if response.is_error() {
                    bail!("Server returned an Announce Error: {url}")
                }

                if response.transaction_id() != UDP_TRANSACTION_ID {
                    bail!("Invalid Transaction ID recieved: {url}")
                }

                let peers = response.peers.take().map(Arc::new);

                Ok(TrackerResponse {
                    state: TrackerResponseState::Udp(response),
                    peers,
                })
            }
            TrackerRequestState::Empty => Err(anyhow!("Making Request on empty string")),
        }
    }
}

/// HTTP-specific parameters for the tracker request based on the [torrent
/// spec](https://wiki.theory.org/BitTorrentSpecification#Tracker_Request_Parameters)
#[derive(Debug, Serialize, Clone)]
pub struct HttpTrackerRequestParams {
    /// The info_hash calculated from the meta_info file provided to the Client.
    #[serde(skip)] // NOTE: becuause serde_urlencoded doesnot correctly serialize.
    pub(crate) info_hash: InfoHashEncoded,

    /// PeerID of the Client.
    #[serde(skip)] // NOTE: becuause serde_urlencoded doesnot correctly serialize.
    pub(crate) peer_id: PeerID,

    /// The port number that the client is listening on. Ports reserved for BitTorrent are typically
    /// 6881-6889. Clients may choose to give up if it cannot establish a port within this range.
    pub(crate) port: u16,

    /// The total amount uploaded (since the client sent the 'started' event to the tracker) in base
    /// ten ASCII. While not explicitly stated in the official specification, the consensus is that
    /// this should be the total number of bytes uploaded.
    pub(crate) uploaded: usize,

    /// The total amount downloaded (since the client sent the 'started' event to the tracker) in
    /// base ten ASCII. While not explicitly stated in the official specification, the consensus is
    /// that this should be the total number of bytes downloaded.
    pub(crate) downloaded: usize,

    /// The number of bytes this client still has to download in base ten ASCII. Clarification: The
    /// number of bytes needed to download to be 100% complete and get all the included files in the
    /// torrent.
    pub(crate) left: usize,

    /// Setting this to 1 indicates that the client accepts a compact response. The peers list is
    /// replaced by a peers string with 6 bytes per peer. The first four bytes are the host (in
    /// network byte order), the last two bytes are the port (again in network byte order). It
    /// should be noted that some trackers only support compact responses (for saving bandwidth) and
    /// either refuse requests without "compact=1" or simply send a compact response unless the
    /// request contains "compact=0" (in which case they will refuse the request.)
    #[serde(serialize_with = "bool_as_int")]
    pub(crate) compact: bool,

    /// Indicates that the tracker can omit peer id field in peers dictionary. This option is
    /// ignored if compact is enabled.
    #[serde(serialize_with = "bool_as_int")]
    pub(crate) no_peer_id: bool,

    /// If specified, must be one of started, completed, stopped, (or empty which is the same as not
    /// being specified). If not specified, then this request is one performed at regular intervals.
    pub(crate) event: Option<Event>,

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
    pub(crate) ip: Option<String>,

    /// Number of peers that the client would like to receive from the tracker. This value is
    /// permitted to be zero. If omitted, typically defaults to 50 peers.
    pub(crate) numwant: Option<usize>,

    /// An additional identification that is not shared with any other peers. It is intended to
    /// allow a client to prove their identity should their IP address change.
    pub(crate) key: Option<String>,

    /// If a previous announce contained a tracker id, it should be set here.
    #[serde(serialize_with = "serialize_tracker_id")]
    pub(crate) trackerid: Option<TrackerID>,
}

/// UID associated with each tracker
#[derive(Debug, Serialize, Clone)]
pub struct TrackerID {
    pub(crate) id: String,
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

impl HttpTrackerRequestParams {
    pub(crate) fn new(info_hash: InfoHashEncoded) -> Self {
        HttpTrackerRequestParams {
            info_hash,
            peer_id: *PEER_ID,
            // TODO:: Listen on ports 6881 to 6889
            port: 6881,
            uploaded: 0,
            downloaded: 0,
            left: 0,
            compact: true,
            no_peer_id: false,
            event: Some(Event::Started),
            ip: None,
            numwant: Some(0),
            key: None,
            trackerid: None,
        }
    }
}

/// UDP-specific parameters for the tracker request based on the [UDP Tracker Protocol for
/// BitTorrent](https://www.bittorrent.org/beps/bep_0015.html).
///
/// The request is structured as follows:
///
/// ```text
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
/// ```
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct UdpTrackerRequestParams {
    connection_id: i64,
    action: i32,
    transaction_id: i32,
    info_hash: InfoHashEncoded,
    peer_id: PeerID,
    downloaded: i64,
    left: i64,
    uploaded: i64,
    event: i32,
    ip_address: i32,
    key: i32,
    num_want: i32,
    port: i16,
}

impl UdpTrackerRequestParams {
    pub(crate) fn new(connection_id: i64, info_hash: InfoHashEncoded) -> Self {
        UdpTrackerRequestParams {
            connection_id,
            action: Action::Announce as i32, // 1 -> Announce
            transaction_id: UDP_TRANSACTION_ID,
            info_hash,
            peer_id: *PEER_ID,
            downloaded: 0,
            left: 0, // TODO: update this.
            uploaded: 0,
            event: Event::None as i32,
            ip_address: 0,
            num_want: -1,
            key: 0,
            port: 6886,
        }
    }

    fn to_bytes(self) -> [u8; 98] {
        let mut bytes = BytesMut::with_capacity(98);

        bytes.put_slice(&self.connection_id.to_be_bytes());
        bytes.put_slice(&self.action.to_be_bytes());
        bytes.put_slice(&self.transaction_id.to_be_bytes());
        bytes.put_slice(self.info_hash.as_ref());
        bytes.put_slice(self.peer_id.as_bytes().as_ref());
        bytes.put_slice(&self.downloaded.to_be_bytes());
        bytes.put_slice(&self.left.to_be_bytes());
        bytes.put_slice(&self.uploaded.to_be_bytes());
        bytes.put_slice(&self.event.to_be_bytes());
        bytes.put_slice(&self.ip_address.to_be_bytes());
        bytes.put_slice(&self.key.to_be_bytes());
        bytes.put_slice(&self.num_want.to_be_bytes());
        bytes.put_slice(&self.port.to_be_bytes());

        bytes
            .as_ref()
            .try_into()
            .expect("Faliure in converting bytes")
    }
}

#[derive(Debug, Serialize, PartialEq, Eq, Clone, Copy)]
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

/// For obtaining the `connection_id` from the tracker.
///
/// The request is structured as follows:
///
/// ```text
/// Offset  Size            Name            Value
/// 0       64-bit integer  protocol_id     0x41727101980 // magic constant
/// 8       32-bit integer  action          0 // connect
/// 12      32-bit integer  transaction_id
/// 16
/// ```
#[derive(Debug)]
pub struct UdpConnectRequest {
    protocol_id: i64,
    action: Action,
    transaction_id: i32,
}

/// The response received from a tracker upon making the [`UdpConnectRequest`].
///
/// The response is structured as follows:
///
/// ```text
/// Offset  Size            Name            Value
/// 0       32-bit integer  action          0 // connect
/// 4       32-bit integer  transaction_id
/// 8       64-bit integer  connection_id
/// 16
/// ```
#[derive(Debug)]
#[repr(C)]
pub struct UdpConnectResponse {
    action: Action,
    transaction_id: i32,
    connection_id: i64,
}

impl UdpConnectResponse {
    pub(crate) fn connection_id(&self) -> i64 {
        self.connection_id
    }
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
        let mut bytes = BytesMut::with_capacity(16);

        bytes.put_slice(&self.protocol_id.to_be_bytes());
        bytes.put_slice(&(self.action as i32).to_be_bytes());
        bytes.put_slice(&self.transaction_id.to_be_bytes());

        bytes
            .as_ref()
            .try_into()
            .expect("Faliure in converting Bytes")
    }

    #[instrument(name = "udp_connect_request", skip(self, socket))]
    pub(crate) async fn connect_with(
        &self,
        udp_url: &str,
        socket: &UdpSocket,
    ) -> Result<UdpConnectResponse> {
        let request_bytes = self.as_bytes();
        let mut response = [0_u8; 16];

        socket
            .connect(udp_url)
            .timeout(TIMEOUT_DURATION)
            .await
            .with_context(|| format!("Connection Timed Out: {udp_url}"))?
            .with_context(|| format!("Failed to connect: {udp_url}"))?;

        debug!("Connected");

        socket
            .send(&request_bytes)
            .timeout(TIMEOUT_DURATION)
            .await
            .with_context(|| format!("Send Timed Out: {udp_url}"))?
            .with_context(|| format!("Sending connect request: {udp_url}"))?;

        debug!("Request Sent");

        socket
            .recv(&mut response)
            .timeout(TIMEOUT_DURATION)
            .await
            .with_context(|| format!("Recieve Timed Out: {udp_url}"))?
            .with_context(|| format!("Failed to recieve any response: {udp_url}"))?;

        debug!("Response Recieved");

        let udp_response = UdpConnectResponse {
            action: Action::from_i32(i32::from_be_bytes(response[0..4].try_into()?))?,
            transaction_id: i32::from_be_bytes(response[4..8].try_into()?),
            connection_id: i64::from_be_bytes(response[8..16].try_into()?),
        };

        ensure!(
            udp_response.transaction_id == self.transaction_id,
            "Transaction ids mismatch!, Invalid response from udp server"
        );

        Ok(udp_response)
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
