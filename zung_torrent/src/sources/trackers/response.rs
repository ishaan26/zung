use anyhow::Result;
use bytes::{Buf, BytesMut};
use serde::Deserialize;

use super::Action;

#[derive(Debug)]
#[allow(dead_code)]
pub enum TrackerReponse {
    Http(HttpTrackerResponse),
    Udp(UdpTrackerResponse),
    Empty,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct HttpTrackerResponse {
    // If present, then no other keys may be present. The value is a human-readable error message as to why the request failed (string).
    #[serde(rename = "failure reason")]
    failure_reason: Option<String>,

    // (new, optional) Similar to failure reason, but the response still gets processed normally. The warning message is shown just like an error.
    #[serde(rename = "warning message")]
    warning_message: Option<String>,

    // Interval in seconds that the client should wait between sending regular requests to the tracker
    interval: u32,

    // (optional) Minimum announce interval. If present clients must not reannounce more frequently than this.
    #[serde(rename = "min interval")]
    min_interval: Option<u32>,

    // A string that the client should send back on its next announcements. If absent and a previous announce sent a tracker id, do not discard the old value; keep using it.
    #[serde(rename = "tracker id")]
    tracker_id: Option<u32>,

    // number of peers with the entire file, i.e. seeders (integer)
    complete: Option<u64>,

    // number of non-seeder peers, aka "leechers" (integer)
    incomplete: Option<u64>,
    // peer: Option<>,
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
    peers: Vec<u8>,
}

impl UdpTrackerResponse {
    pub(crate) fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut bytes = BytesMut::from(bytes);

        let mut peers = Vec::new();

        let action = Action::from_i32(bytes.get_i32())?;
        let transaction_id = bytes.get_i32();
        let interval = bytes.get_i32();
        let leechers = bytes.get_i32();
        let seeders = bytes.get_i32();

        peers.extend(bytes);

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
