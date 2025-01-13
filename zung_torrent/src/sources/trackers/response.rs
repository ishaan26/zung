use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

use anyhow::{bail, Result};
use bytes::{Buf, BytesMut};
use serde::{de::Visitor, Deserialize, Serialize, Serializer};

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
    pub(crate) peers: TrackerPeers,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct TrackerPeers {
    peers: Option<PeersV4>,
    peers6: Option<PeersV6>,
}

impl TrackerPeers {
    fn from_udp_bytes(bytes: &[u8], recv_socket: SocketAddr) -> Result<Self> {
        if recv_socket.is_ipv4() {
            Ok(TrackerPeers {
                peers: Some(PeersV4::from_bytes(bytes)?),
                peers6: None,
            })
        } else if recv_socket.is_ipv6() {
            Ok(Self {
                peers: None,
                peers6: Some(PeersV6::from_bytes(bytes)?),
            })
        } else {
            bail!("Invalid Udp peers")
        }
    }

    pub const fn contains_peers_v4(&self) -> bool {
        self.peers.is_some()
    }

    pub const fn contains_peers_v6(&self) -> bool {
        self.peers6.is_some()
    }
}

#[derive(Debug)]
pub struct PeersV4(Vec<SocketAddrV4>);

impl PeersV4 {
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() % 6 != 0 {
            bail!("Invalid Peers length");
        }

        let peers = bytes.chunks_exact(6).map(|c| {
            SocketAddrV4::new(
                Ipv4Addr::new(c[0], c[1], c[2], c[3]),
                u16::from_be_bytes([c[4], c[5]]),
            )
        });

        Ok(PeersV4(peers.collect()))
    }
}

impl<'de> Deserialize<'de> for PeersV4 {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct PeersV4Visitor;

        impl Visitor<'_> for PeersV4Visitor {
            type Value = PeersV4;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("Expecting Torrent Peers")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                PeersV4::from_bytes(v).map_err(|e| E::custom(e))
            }
        }

        deserializer.deserialize_bytes(PeersV4Visitor)
    }
}

impl Serialize for PeersV4 {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut single_slice = Vec::with_capacity(6 * self.0.len());

        for peer in &self.0 {
            single_slice.extend(peer.ip().octets());
            single_slice.extend(peer.port().to_be_bytes());
        }

        serializer.serialize_bytes(&single_slice)
    }
}

#[derive(Debug)]
pub struct PeersV6(Vec<SocketAddrV6>);

impl PeersV6 {
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() % 18 != 0 {
            bail!("Invalid Peers Length");
        }

        let peers = bytes.chunks_exact(18).map(|c| {
            let ip: [u8; 16] = c[0..16].try_into().unwrap();
            let port = u16::from_be_bytes([c[16], c[17]]);

            SocketAddrV6::new(Ipv6Addr::from(ip), port, 0, 0)
        });

        Ok(PeersV6(peers.collect()))
    }
}

impl<'de> Deserialize<'de> for PeersV6 {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct PeersV6Visitor;

        impl Visitor<'_> for PeersV6Visitor {
            type Value = PeersV6;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("expecting peers v6")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                PeersV6::from_bytes(v).map_err(|e| E::custom(e))
            }
        }

        deserializer.deserialize_bytes(PeersV6Visitor)
    }
}

impl Serialize for PeersV6 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut single_slice = Vec::with_capacity(18 * self.0.len());

        for peer in &self.0 {
            single_slice.extend(peer.ip().octets());
            single_slice.extend(peer.port().to_be_bytes());
        }

        serializer.serialize_bytes(&single_slice)
    }
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
    peers: TrackerPeers,
}

impl UdpTrackerResponse {
    pub(crate) fn from_bytes(bytes: &[u8], recv_socket: SocketAddr) -> Result<Self> {
        let mut bytes = BytesMut::from(bytes);

        let action = Action::from_i32(bytes.get_i32())?;
        let transaction_id = bytes.get_i32();
        let interval = bytes.get_i32();
        let leechers = bytes.get_i32();
        let seeders = bytes.get_i32();
        let peers = TrackerPeers::from_udp_bytes(bytes.as_ref(), recv_socket)?;

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

    pub fn peers(&self) -> &TrackerPeers {
        &self.peers
    }
}
