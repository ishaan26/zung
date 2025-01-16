use std::net::SocketAddr;

use anyhow::Result;
use bytes::{Buf, BytesMut};
use serde::Deserialize;

use crate::sources::peers::PeersList;

use super::Action;

#[derive(Debug)]
pub struct TrackerResponse {
    pub(crate) state: TrackerResponseState,
}

impl TrackerResponse {
    pub fn empty() -> Self {
        TrackerResponse {
            state: TrackerResponseState::Empty,
        }
    }

    pub fn get_peers(&self) -> Option<&PeersList> {
        match &self.state {
            TrackerResponseState::Http(http_tracker_response) => {
                http_tracker_response.peers.as_ref()
            }
            TrackerResponseState::Udp(udp_tracker_response) => udp_tracker_response.peers.as_ref(),
            TrackerResponseState::Empty => None,
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self.state, TrackerResponseState::Empty)
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum TrackerResponseState {
    Http(HttpTrackerResponse),
    Udp(UdpTrackerResponse),
    Empty,
}

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

// Offset      Size            Name            Value
// 0           32-bit integer  action          1 // announce
// 4           32-bit integer  transaction_id
// 8           32-bit integer  interval
// 12          32-bit integer  leechers
// 16          32-bit integer  seeders
// 20 + 6 * n  32-bit integer  IP address
// 24 + 6 * n  16-bit integer  TCP port
// 20 + 6 * N
#[derive(Debug)]
#[allow(dead_code)]
pub struct UdpTrackerResponse {
    action: Action,
    transaction_id: i32,
    interval: i32,
    leechers: i32,
    seeders: i32,
    peers: Option<PeersList>,
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
