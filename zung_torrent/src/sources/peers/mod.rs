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

    pub fn get_addrs(&self) -> (&[SocketAddrV4], &[SocketAddrV6]) {
        static EMPTY_V4: Vec<SocketAddrV4> = Vec::new();
        static EMPTY_V6: Vec<SocketAddrV6> = Vec::new();

        let listv4 = self
            .peers
            .as_ref()
            .map(|peerv4| &peerv4.0)
            .unwrap_or(&EMPTY_V4);

        let listv6 = self
            .peers6
            .as_ref()
            .map(|peerv6| &peerv6.0)
            .unwrap_or(&EMPTY_V6);

        (listv4, listv6)
    }

    pub fn num_of_peers(&self) -> usize {
        let v4_len = self.peers.as_ref().map(|p| p.0.len()).unwrap_or(0);
        let v6_len = self.peers6.as_ref().map(|p| p.0.len()).unwrap_or(0);

        v4_len + v6_len
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    const BYTES_V4: &[u8] = &[
        192, 168, 1, 1, 0x1F, 0x90, // 192.168.1.1:8080
        192, 168, 1, 2, 0x23, 0x28, // 192.168.1.2:9000
    ];

    const BYTES_V6: &[u8] = &[
        // [IPv6 address (16 bytes)] [Port (2 bytes)]
        0x20, 0x01, 0x0D, 0xB8, 0x85, 0xA3, 0x00, 0x00, 0x00, 0x00, 0x8A, 0x2E, 0x03, 0x70, 0x73,
        0x34, // IPv6 address
        0x1F, 0x90, // Port 8080
    ];

    #[test]
    fn test_peersv4_from_bytes_valid() {
        let peers = PeersV4::from_bytes(BYTES_V4).unwrap();
        let expected = vec![
            SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 1), 8080),
            SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 2), 9000),
        ];
        assert_eq!(peers.0, expected);
    }

    #[test]
    fn test_peersv4_from_bytes_invalid() {
        let bytes: &[u8] = &[192, 168, 1, 1, 0x1F];
        let result = PeersV4::from_bytes(bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_peersv6_from_bytes_valid() {
        let peers = PeersV6::from_bytes(BYTES_V6).unwrap();
        let expected = vec![SocketAddrV6::new(
            Ipv6Addr::new(
                0x2001, 0x0DB8, 0x85A3, 0x0000, 0x0000, 0x8A2E, 0x0370, 0x7334,
            ),
            8080,
            0,
            0,
        )];
        assert_eq!(peers.0, expected);
    }

    #[test]
    fn test_peersv6_from_bytes_invalid() {
        let bytes: &[u8] = &[
            0x20, 0x01, 0x0D, 0xB8, 0x85, 0xA3, 0x00, 0x00, 0x00, 0x00, 0x8A, 0x2E, 0x03, 0x70,
            0x73,
        ];
        let result = PeersV6::from_bytes(bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_tracker_peers_from_udp_bytes_ipv4() {
        let recv_socket = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 12345));
        let tracker_peers = TrackerPeers::from_udp_bytes(BYTES_V4, recv_socket).unwrap();
        assert!(tracker_peers.contains_peers_v4());
        assert!(!tracker_peers.contains_peers_v6());
        assert_eq!(tracker_peers.num_of_peers(), 2);
    }

    #[test]
    fn test_tracker_peers_from_udp_bytes_ipv6() {
        let recv_socket = SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 12345, 0, 0));
        let tracker_peers = TrackerPeers::from_udp_bytes(BYTES_V6, recv_socket).unwrap();
        assert!(!tracker_peers.contains_peers_v4());
        assert!(tracker_peers.contains_peers_v6());
        assert_eq!(tracker_peers.num_of_peers(), 1);
    }

    #[test]
    fn test_tracker_peers_get_addrs() {
        let tracker_peers = TrackerPeers {
            peers: Some(PeersV4::from_bytes(BYTES_V4).unwrap()),
            peers6: Some(PeersV6::from_bytes(BYTES_V6).unwrap()),
        };

        let (v4_addrs, v6_addrs) = tracker_peers.get_addrs();

        assert_eq!(
            v4_addrs,
            vec![
                SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 1), 8080),
                SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 2), 9000),
            ]
        );

        assert_eq!(
            v6_addrs,
            &vec![SocketAddrV6::new(
                Ipv6Addr::new(0x2001, 0x0DB8, 0x85A3, 0x0000, 0x0000, 0x8A2E, 0x0370, 0x7334),
                8080,
                0,
                0
            )]
        );
    }
}
