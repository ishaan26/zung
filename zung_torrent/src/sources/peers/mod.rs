use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

use anyhow::{bail, Result};
use serde::{de::Visitor, Deserialize, Serialize, Serializer};

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct TrackerPeers {
    peers: Option<PeersV4>,
    peers6: Option<PeersV6>,
}

impl TrackerPeers {
    pub fn from_udp_bytes(bytes: &[u8], recv_socket: SocketAddr) -> Result<Self> {
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

    pub fn get_addrs(&self) -> (Vec<SocketAddrV4>, Vec<SocketAddrV6>) {
        let mut list = Vec::new();
        let mut list6 = Vec::new();

        if let Some(peers) = &self.peers {
            list.extend(&peers.0);
        }

        if let Some(peers6) = &self.peers6 {
            list6.extend(&peers6.0);
        }

        (list, list6)
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
