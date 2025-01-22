use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use bytes::{Buf, BytesMut};
use serde::Deserialize;

use crate::sources::peers::PeersList;

use super::Action;

/// Represents the response received from a torrent tracker.
///
/// A `TrackerResponse` encapsulates the state returned by the tracker, which may include a list of
/// peers and other relevant metadata. It supports both HTTP and UDP tracker protocols, as well as
/// an empty state indicating no response has been received yet.
#[derive(Debug)]
pub struct TrackerResponse {
    pub(crate) state: TrackerResponseState,
    pub(crate) peers: Option<Arc<PeersList>>,
}

impl TrackerResponse {
    /// Creates an empty tracker response.
    pub(crate) fn empty() -> Self {
        TrackerResponse {
            state: TrackerResponseState::Empty,
            peers: None,
        }
    }

    /// Retrieves the current state of the tracker response.
    pub(crate) fn state(&self) -> &TrackerResponseState {
        &self.state
    }

    /// Returns a referece to the list of peers contained in a tracker response (if the tracker has
    /// bothered to provide such a thing... Man torrent protocol is WILD!)
    pub fn get_peers_list(&self) -> Option<Arc<PeersList>> {
        self.peers.as_ref().map(Arc::clone)
    }

    /// Checks if the Tracker has responded with any peers.
    pub fn containes_peers(&self) -> bool {
        match &self.state() {
            TrackerResponseState::Http(http) => http.peers.is_some(),
            TrackerResponseState::Udp(udp) => udp.peers.is_some(),
            TrackerResponseState::Empty => false,
        }
    }

    /// Checks if the tracker response is uninitialized, meaning that the tracker has not been
    /// connected to yet.
    pub fn is_uninitialized(&self) -> bool {
        matches!(self.state(), &TrackerResponseState::Empty)
    }
}

impl AsRef<Self> for TrackerResponse {
    fn as_ref(&self) -> &Self {
        self
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum TrackerResponseState {
    Http(HttpTrackerResponse),
    Udp(UdpTrackerResponse),
    Empty,
}

/// Represents an HTTP tracker response.
///
/// This structure is used to deserialize responses from an HTTP tracker. The response may include
/// various fields such as errors, warnings, intervals, and peer information.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct HttpTrackerResponse {
    // If present, then no other keys may be present. The value is a human-readable error message as to why the request failed (string).
    #[serde(rename = "failure reason")]
    pub(crate) failure_reason: Option<String>,

    // (new, optional) Similar to failure reason, but the response still gets processed normally. The warning message is shown just like an error.
    #[serde(rename = "warning message")]
    pub(crate) warning_message: Option<String>,

    // Interval in seconds that the client should wait between sending regular requests to the tracker
    pub(crate) interval: u32,

    // (optional) Minimum announce interval. If present clients must not reannounce more frequently than this.
    #[serde(rename = "min interval")]
    pub(crate) min_interval: Option<u32>,

    // A string that the client should send back on its next announcements. If absent and a previous announce sent a tracker id, do not discard the old value; keep using it.
    #[serde(rename = "tracker id")]
    pub(crate) tracker_id: Option<u32>,

    // number of peers with the entire file, i.e. seeders (integer)
    pub(crate) complete: Option<u64>,

    // number of non-seeder peers, aka "leechers" (integer)
    pub(crate) incomplete: Option<u64>,

    #[serde(flatten)]
    pub(crate) peers: Option<PeersList>,
}

/// Represents a UDP tracker response.
///
/// This structure is used to parse and handle responses from a UDP tracker. The response includes
/// fields such as action type, transaction ID, intervals, and peer information.
///
/// Th response is structured as follows:
///
/// ```text
/// Offset      Size            Name            Value
/// 0           32-bit integer  action          1 // announce
/// 4           32-bit integer  transaction_id
/// 8           32-bit integer  interval
/// 12          32-bit integer  leechers
/// 16          32-bit integer  seeders
/// 20 + 6 * n  32-bit integer  IP address
/// 24 + 6 * n  16-bit integer  TCP port
/// 20 + 6 * N
/// ```
#[derive(Debug)]
#[allow(dead_code)]
pub struct UdpTrackerResponse {
    action: Action,
    transaction_id: i32,
    interval: i32,
    leechers: i32,
    seeders: i32,
    pub(crate) peers: Option<PeersList>,
}

impl UdpTrackerResponse {
    pub(crate) fn from_bytes(bytes: &[u8], recv_socket: SocketAddr) -> Result<Self> {
        let mut bytes = BytesMut::from(bytes);

        let action = Action::from_i32(bytes.get_i32())?;
        let transaction_id = bytes.get_i32();
        let interval = bytes.get_i32();
        let leechers = bytes.get_i32();
        let seeders = bytes.get_i32();
        let peers = if bytes.len() >= 6 {
            Some(PeersList::from_udp_bytes(bytes.as_ref(), recv_socket)?)
        } else {
            None
        };

        Ok(Self {
            action,
            transaction_id,
            interval,
            leechers,
            seeders,
            peers,
        })
    }

    pub fn is_error(&self) -> bool {
        self.action == Action::Error
    }

    pub fn transaction_id(&self) -> i32 {
        self.transaction_id
    }
}
