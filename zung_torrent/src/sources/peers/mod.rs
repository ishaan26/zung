//! Provides functionality for managing and iterating over IPv4 and IPv6 peers.
//!
//! Peers refer to the number of users that have the file and are seeding (sharing). If the torrent
//! file has a healthy number of peers, it should result in faster and more reliable file transfer
//! and a quality streaming experience. If your torrent file has zero or few peers (i.e., few
//! people are sharing the file), you may experience buffering, or may not be able to view the file
//! at all.
//!
//! To monitor the number of peers after you have started a torrent stream, look to the bottom of
//! the player window and count the number of active peers (seeders). If you run into problems with
//! the quality of the stream, or the media cannot play at all, it may be due to a low number of
//! peers, or no peers at all. If this is the case, try to find a different torrent file or keep
//! the existing torrent file and try again later. You might find that more seeders come online,
//! and the file becomes more available to stream.

mod handshake;
mod peer_messages;

pub use handshake::*;
pub use peer_messages::*;

use std::{
    hash::Hash,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use anyhow::{bail, Result};
use rayon::{iter::ParallelIterator, slice::ParallelSlice};
use serde::{de::Visitor, Deserialize, Serialize, Serializer};

/// Reprasents a single peer within the [`PeersList`]
#[derive(Debug)]
pub struct Peer {
    addr: SocketAddr,
    connected: Arc<AtomicBool>,
}

impl Peer {
    fn get_octets(&self) -> Vec<u8> {
        match &self.addr {
            SocketAddr::V4(socket_addr_v4) => socket_addr_v4.ip().octets().to_vec(),
            SocketAddr::V6(socket_addr_v6) => socket_addr_v6.ip().octets().to_vec(),
        }
    }

    pub const fn get_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Sets the peer state to`connected`.
    pub fn set_connected(&self) {
        self.connected.store(true, Ordering::Relaxed);
    }

    /// Check if the peer is connected or not.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }
}

impl Clone for Peer {
    fn clone(&self) -> Self {
        Self {
            addr: self.addr,
            connected: Arc::clone(&self.connected),
        }
    }
}

impl Hash for Peer {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.addr.hash(state);
    }
}

impl PartialEq for Peer {
    fn eq(&self, other: &Self) -> bool {
        self.addr == other.addr
    }
}

impl PartialEq<SocketAddr> for Peer {
    fn eq(&self, other: &SocketAddr) -> bool {
        &self.addr == other
    }
}

impl PartialEq<SocketAddrV4> for Peer {
    fn eq(&self, other: &SocketAddrV4) -> bool {
        match &self.addr {
            SocketAddr::V4(socket_addr_v4) => socket_addr_v4 == other,
            SocketAddr::V6(..) => false,
        }
    }
}

impl PartialEq<SocketAddrV6> for Peer {
    fn eq(&self, other: &SocketAddrV6) -> bool {
        match &self.addr {
            SocketAddr::V4(..) => false,
            SocketAddr::V6(socket_addr_v6) => socket_addr_v6 == other,
        }
    }
}

impl Eq for Peer {}

impl From<SocketAddrV4> for Peer {
    fn from(value: SocketAddrV4) -> Self {
        Self {
            addr: SocketAddr::from(value),
            connected: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl From<SocketAddrV6> for Peer {
    fn from(value: SocketAddrV6) -> Self {
        Self {
            addr: SocketAddr::from(value),
            connected: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// The list of network peers as recieved from a tracker in a [`TrackerResponse`].
///
/// This type is automatically generated when [`TrackerResponse`] is initialized for a [`Tracker`]
/// while using the [`announce`] method on the [`Tracker`].
///
/// # Examples
///
///  TODO: update examples when the Client API is finalized.
///
/// # NOTES
///
/// A [`TrackerResponse`] can contain peers with both IPv4 and IPv6 addresses. Therefore this type
/// provides a unified interface for working with peers across different IP versions. It supports
/// iteration over peers in both borrowed and owned contexts, automatically handling the transition
/// between IPv4 and IPv6 peers.
///
/// The iterator implementations are designed to be zero-cost, with no allocation overhead when
/// iterating. Both borrowed and owned iteration use efficient standard library iterators
/// internally.
///
/// [`TrackerResponse`]: crate::sources::trackers::TrackerResponse
/// [`Tracker`]: crate::sources::trackers::Tracker
/// [`announce`]: crate::sources::trackers::Tracker::announce
#[derive(Debug, Deserialize)]
pub struct PeersList {
    peers: Option<PeersV4>,
    peers6: Option<PeersV6>,
}

impl PeersList {
    pub(crate) fn from_udp_bytes(bytes: &[u8], recv_socket: SocketAddr) -> Result<Self> {
        if recv_socket.is_ipv4() {
            Ok(PeersList {
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

    pub fn to_vec(&self) -> Vec<Peer> {
        static EMPTY: Vec<Peer> = Vec::new();

        let listv4 = self
            .peers
            .as_ref()
            .map(|peerv4| &peerv4.0)
            .unwrap_or(&EMPTY);

        let listv6 = self
            .peers6
            .as_ref()
            .map(|peerv6| &peerv6.0)
            .unwrap_or(&EMPTY);

        let mut combined = Vec::with_capacity(listv4.len() + listv6.len());
        combined.extend_from_slice(listv4);
        combined.extend_from_slice(listv6);
        combined
    }

    pub fn num_of_peers(&self) -> usize {
        let v4_len = self.peers.as_ref().map(|p| p.0.len()).unwrap_or(0);
        let v6_len = self.peers6.as_ref().map(|p| p.0.len()).unwrap_or(0);

        v4_len + v6_len
    }

    pub fn iter(&self) -> PeersIter<'_> {
        PeersIter::new(self)
    }
}

pub struct PeersIter<'a> {
    v4_iter: Option<std::slice::Iter<'a, Peer>>,
    v6_iter: Option<std::slice::Iter<'a, Peer>>,
}

impl<'a> PeersIter<'a> {
    fn new(list: &'a PeersList) -> Self {
        PeersIter {
            v4_iter: list.peers.as_ref().map(|p| p.0.iter()),
            v6_iter: list.peers6.as_ref().map(|p| p.0.iter()),
        }
    }
}

impl<'a> Iterator for PeersIter<'a> {
    type Item = &'a Peer;

    fn next(&mut self) -> Option<Self::Item> {
        self.v4_iter.as_mut().and_then(|v4| v4.next()).or_else(|| {
            if let Some(ref mut v4) = self.v4_iter {
                if v4.len() == 0 {
                    self.v4_iter = None;
                }
            }
            self.v6_iter.as_mut().and_then(|v6| v6.next())
        })
    }
}

impl ExactSizeIterator for PeersIter<'_> {}

impl<'a> IntoIterator for &'a PeersList {
    type Item = &'a Peer;
    type IntoIter = PeersIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter::new(self)
    }
}

impl IntoIterator for PeersList {
    type Item = Peer;
    type IntoIter = std::iter::Chain<std::vec::IntoIter<Peer>, std::vec::IntoIter<Peer>>;

    fn into_iter(self) -> Self::IntoIter {
        let v4_iter = self.peers.map(|v4| v4.0).unwrap_or_default().into_iter();
        let v6_iter = self.peers6.map(|v6| v6.0).unwrap_or_default().into_iter();
        v4_iter.chain(v6_iter)
    }
}

#[derive(Debug)]
struct PeersV4(Vec<Peer>);

impl PeersV4 {
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() % 6 != 0 {
            bail!("Invalid Peers length");
        }

        let peers = bytes
            .par_chunks_exact(6)
            .map(|c| {
                SocketAddrV4::new(
                    Ipv4Addr::new(c[0], c[1], c[2], c[3]),
                    u16::from_be_bytes([c[4], c[5]]),
                )
            })
            .map(Peer::from);

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
            single_slice.extend(peer.get_octets());
            single_slice.extend(peer.addr.port().to_be_bytes());
        }

        serializer.serialize_bytes(&single_slice)
    }
}

#[derive(Debug, Clone)]
struct PeersV6(Vec<Peer>);

impl PeersV6 {
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() % 18 != 0 {
            bail!("Invalid Peers Length");
        }

        let peers = bytes
            .par_chunks_exact(18)
            .map(|c| {
                let ip: [u8; 16] = c[0..16].try_into().unwrap();
                let port = u16::from_be_bytes([c[16], c[17]]);

                SocketAddrV6::new(Ipv6Addr::from(ip), port, 0, 0)
            })
            .map(Peer::from);

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
            single_slice.extend(peer.get_octets());
            single_slice.extend(peer.addr.port().to_be_bytes());
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
        let tracker_peers = PeersList::from_udp_bytes(BYTES_V4, recv_socket).unwrap();
        assert!(tracker_peers.contains_peers_v4());
        assert!(!tracker_peers.contains_peers_v6());
        assert_eq!(tracker_peers.num_of_peers(), 2);
    }

    #[test]
    fn test_tracker_peers_from_udp_bytes_ipv6() {
        let recv_socket = SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 12345, 0, 0));
        let tracker_peers = PeersList::from_udp_bytes(BYTES_V6, recv_socket).unwrap();
        assert!(!tracker_peers.contains_peers_v4());
        assert!(tracker_peers.contains_peers_v6());
        assert_eq!(tracker_peers.num_of_peers(), 1);
    }

    #[test]
    fn test_tracker_peers_get_addrs() {
        let tracker_peers = PeersList {
            peers: Some(PeersV4::from_bytes(BYTES_V4).unwrap()),
            peers6: Some(PeersV6::from_bytes(BYTES_V6).unwrap()),
        };

        let peers = tracker_peers.to_vec();

        assert_eq!(
            peers,
            vec![
                Peer::from(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 1), 8080)),
                Peer::from(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 2), 9000)),
                Peer::from(SocketAddrV6::new(
                    Ipv6Addr::new(0x2001, 0x0DB8, 0x85A3, 0x0000, 0x0000, 0x8A2E, 0x0370, 0x7334),
                    8080,
                    0,
                    0
                )),
            ]
        );
    }

    #[test]
    fn test_peers_list_iter() {
        let tracker_peers = PeersList {
            peers: Some(PeersV4::from_bytes(BYTES_V4).unwrap()),
            peers6: Some(PeersV6::from_bytes(BYTES_V6).unwrap()),
        };

        let mut iter = tracker_peers.iter();

        // First v4
        assert_eq!(
            iter.next(),
            Some(&Peer::from(SocketAddrV4::new(
                Ipv4Addr::new(192, 168, 1, 1),
                8080
            )))
        );

        // Second v4
        assert_eq!(
            iter.next(),
            Some(&Peer::from(SocketAddrV4::new(
                Ipv4Addr::new(192, 168, 1, 2),
                9000
            )))
        );

        // First v6
        assert_eq!(
            iter.next(),
            Some(&Peer::from(SocketAddrV6::new(
                Ipv6Addr::new(0x2001, 0x0DB8, 0x85A3, 0x0000, 0x0000, 0x8A2E, 0x0370, 0x7334),
                8080,
                0,
                0
            )))
        );
    }
}
