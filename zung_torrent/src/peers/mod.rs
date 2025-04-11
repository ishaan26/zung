//! For managing and iterating over IPv4 and IPv6 peers in BitTorrent networks.
//!
//! # Overview
//!
//! In BitTorrent terminology, peers are other users who have the file you're trying to download.
//! There are two types of peers:
//!
//! - **Seeders**: Users who have the complete file and are sharing it
//! - **Leechers**: Users who have only parts of the file and are still downloading
//!
//! A healthy torrent has a good number of seeders, which ensures faster downloads and reliable
//! streaming. This module provides the infrastructure to connect to, communicate with, and
//! manage these peers across both IPv4 and IPv6 networks.
//!
//! # Module Contents
//!
//! This module provides:
//! - Peer connection and handshake implementation
//! - Peer Message Protocol message handling
//! - Efficient peer list management for both IPv4 and IPv6
//! - Iterators for working with peer collections
//!
//! ## NOTE:
//!
//! This module is the lower level implementation to handle peer connections based on the
//! BitTorrent Protocol. If you intend to just download a torrent file, please see
//! [`crate::download`] module.
//!
//! # Usage:
//!
//! A list of [`Peer`], i.e., the [`PeersList`] can be obtained through a
//! [`TrackerResponse`](crate::trackers::TrackerResponse) which is obtained by
//! [announceing](crate::trackers::Tracker::announce) to a [`Tracker`](crate::trackers::Tracker).
//!
//! ## Handshake with a peer
//!
//! Once a peer is obtained, the next step in the bittorrent protocol is to perform a
//! [`handshake`](Peer::handshake) with each peer.
//!
//! ```ignore
//! # use zung_torrent::*;
//! let connected_peer = peer.handshake(info_hash).await?;
//! ```
//!
//! The above method returns a new [`Peer`] type which would contain a [`TcpStream`] if the
//! handshake was successful.
//!
//! Once the handshake is successful, next step is the back and forth of the [`PeerMessage`]s over
//! the handshake [`TcpStream`].
//!
//! ## Sending and receiving peer messages
//!
//! ```ignore
//! # use zung_torrent::*;
//! // if stream is `Some`, that means handshake was successful.
//!
//! if let Some(stream) = peer.get_stream_mut() {
//!     stream.recv_peer_message::<BitfieldPayload>();
//!     stream.send_peer_message(PeerMessage::interested());
//! }
//! ```
//!
//! Please refer the documentation of [`PeerMessage`] and [`PeerMessageExt`]
//! for more information on the usage of peer messages.

mod handshake;
mod peer_messages;

use bytes::BytesMut;
pub use handshake::*;
pub use peer_messages::*;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_util::time::FutureExt;

use std::{
    hash::Hash,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
};

use anyhow::{anyhow, bail, ensure, Result};
use rayon::{iter::ParallelIterator, slice::ParallelSlice};
use serde::{de::Visitor, Deserialize, Serialize, Serializer};

use crate::{meta_info::InfoHashEncoded, TIMEOUT_DURATION};

pub const BLOCK_MAX: u32 = 1024 * 16; /* 16 Kbi*/

/// Represents the initial state of a peer before any connection has been established.
#[derive(Debug)]
pub struct Unconnected;

/// Represents a peer that has successfully completed the BitTorrent handshake protocol.
///
/// Contains the TCP stream over which the handshake was performed and future peer messages
/// will be exchanged.
#[derive(Debug)]
pub struct Handshaken {
    stream: TcpStream,
}

/// Represents a peer that has been unchoked and is ready for piece transfers.
///
/// Contains both the TCP stream for communication and the peer's bitfield which indicates
/// which pieces they have available.
#[derive(Debug)]
pub struct Unchoked {
    stream: TcpStream,
    bitfield: PeerMessage<Bitfield>,
}

/// Represents a peer that is currently downloading pieces of a torrent.
///
/// This struct contains the TCP stream for communication with the peer
/// and the piece information that indicates which piece is currently being downloaded.
#[derive(Debug)]
pub struct Downloading {
    stream: TcpStream,
    piece: PeerMessage<Piece>,
}

/// A trait representing a connected peer in the BitTorrent protocol.
///
/// This trait provides methods to access the underlying TCP stream for communication
/// with the peer. Implementations of this trait should provide the necessary functionality
/// to retrieve mutable and owned references to the TCP stream.
pub trait ConnectedPeer {
    /// Returns a mutable reference to the TCP stream.
    fn get_stream_mut(&mut self) -> &mut TcpStream;
    /// Consumes the implementing type and returns the owned TCP stream.
    fn get_stream_owned(self) -> TcpStream;
}

impl ConnectedPeer for Handshaken {
    fn get_stream_mut(&mut self) -> &mut TcpStream {
        &mut self.stream
    }

    fn get_stream_owned(self) -> TcpStream {
        self.stream
    }
}

impl ConnectedPeer for Unchoked {
    fn get_stream_mut(&mut self) -> &mut TcpStream {
        &mut self.stream
    }

    fn get_stream_owned(self) -> TcpStream {
        self.stream
    }
}

impl ConnectedPeer for Downloading {
    fn get_stream_mut(&mut self) -> &mut TcpStream {
        &mut self.stream
    }

    fn get_stream_owned(self) -> TcpStream {
        self.stream
    }
}

/// Represents a single peer within the [`PeersList`].
///
/// The `Peer` struct holds the address of the peer and its current state,
/// which can be in various connection states (e.g., unconnected, handshaken, unchoked).
///
/// In the context of a torrent, a peer is a participant in the file-sharing network
/// that can upload and download pieces of the file being shared. Each peer maintains
/// a state that reflects its current interaction with the torrent, including which pieces
/// it has available for sharing and whether it is currently able to send or receive data.
#[derive(Debug)]
pub struct Peer<T = Unconnected> {
    addr: SocketAddr,
    state: T,
}

impl Peer<Unconnected> {
    /// Creates a new unconnected peer from a socket address.
    ///
    /// This constructor creates a peer in the initial unconnected state,
    /// ready for handshaking with the BitTorrent protocol.
    ///
    /// # Note
    ///
    /// This type is also use to reprasent a single peer in the [`PeersList`] type which is
    /// obtained from a [`TrackerResponse`](crate::trackers::TrackerResponse)
    ///
    /// # Arguments
    ///
    /// * `addr` - The socket address of the peer to connect to
    ///
    /// # Returns
    ///
    /// A new `Peer` instance in the unconnected state
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::{SocketAddr, Ipv4Addr};
    /// use zung_torrent::peers::Peer;
    ///
    /// let addr = SocketAddr::from((Ipv4Addr::new(127, 0, 0, 1), 6881));
    /// let peer = Peer::new(addr);
    /// ```
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            state: Unconnected,
        }
    }

    /// Performs the BitTorrent handshake with the [`Peer`].
    ///
    /// This method establishes a TCP connection with the peer and exchanges handshake messages
    /// according to the BitTorrent protocol. The handshake verifies that both parties are
    /// interested in the same torrent by comparing `info hashes`.
    ///
    /// # Arguments
    ///
    /// * `info_hash` - The encoded info hash of the torrent to be shared
    ///
    /// # Returns
    ///
    /// * `Result<Self>` - A new `Peer` instance with an established connection if successful
    ///
    /// # Errors
    ///
    /// This method will return an error if:
    /// - The TCP connection cannot be established
    /// - The handshake message cannot be sent or received
    /// - The received handshake is invalid or doesn't match the expected format
    /// - The peer doesn't respond within the timeout period
    #[tracing::instrument(
        name = "Handshake"
        skip_all
        fields(peer = %self.get_addr())
    )]
    pub async fn handshake(self, info_hash: InfoHashEncoded) -> Result<Peer<Handshaken>> {
        let mut stream = TcpStream::connect(self.addr)
            .timeout(TIMEOUT_DURATION)
            .await??;

        let handshake = Handshake::new(info_hash);

        stream
            .write_all(&handshake.as_bytes())
            .timeout(TIMEOUT_DURATION)
            .await??;

        stream.flush().await?;

        let mut buff = [0; Handshake::SIZE];
        let read = stream
            .read_exact(&mut buff)
            .timeout(TIMEOUT_DURATION)
            .await??;

        // Check if the size of received message is the same as the sent.
        ensure!(
            read == std::mem::size_of::<Handshake>(),
            "Handshake: required bytes not sent by the peer"
        );

        let recv_handshake = Handshake::from_bytes(buff);

        // Validate the data received.
        ensure!(recv_handshake.pstr() == Handshake::PROTOCOL_V1);
        ensure!(recv_handshake.pstrlen() == Handshake::PROTOCOL_V1.len() as u8);
        ensure!(recv_handshake.info_hash() == handshake.info_hash());

        tracing::info!("Handshake complete");

        Ok(Peer {
            addr: self.addr,
            state: Handshaken { stream },
        })
    }
}

/// Obtained from [`Peer::handshake`]
impl Peer<Handshaken> {
    /// Transitions a handshaken peer to the unchoked state.
    ///
    /// After a successful handshake, this method performs the BitTorrent protocol sequence to:
    /// 1. Receive the peer's bitfield (indicating which pieces they have)
    /// 2. Send an "interested" message to the peer
    /// 3. Wait for an "unchoke" message from the peer
    ///
    /// Once unchoked, the peer connection can be used to request and download pieces.
    ///
    /// # Returns
    ///
    /// * `Result<Peer<Unchoked>>` - A peer in the unchoked state if successful
    ///
    /// # Errors
    ///
    /// This method will return an error if:
    /// - The bitfield message cannot be received
    /// - The interested message cannot be sent
    /// - The unchoke message is not received
    /// - Any operation times out
    #[tracing::instrument(
        name = "Unchoke"
        skip_all
        fields(peer = %self.get_addr())
    )]
    pub async fn unchoke(mut self) -> anyhow::Result<Peer<Unchoked>> {
        let mut buf = BytesMut::with_capacity(BLOCK_MAX as usize);
        let addr = self.addr;

        tracing::debug!("Seeking bitfield message");

        let bitfield = self
            .recv_peer_message::<Bitfield>(&mut buf)
            .timeout(TIMEOUT_DURATION)
            .await??;

        tracing::debug!("Bitfield message received");

        buf.clear();

        tracing::debug!("Sending unchoke message");

        self.send_peer_message(PeerMessage::interested())
            .timeout(TIMEOUT_DURATION)
            .await??;

        tracing::info!("Unchoke Message sent");

        tracing::debug!("Seeking unchoke message");

        self.recv_unchoke_message()
            .timeout(TIMEOUT_DURATION)
            .await??;

        tracing::debug!("Unchoke message received");

        Ok(Peer {
            addr,
            state: Unchoked {
                stream: self.get_stream_owned(),
                bitfield,
            },
        })
    }
}

/// Obtained from [`Peer::unchoke`]
impl Peer<Unchoked> {
    /// Downloads a piece of data from the peer.
    ///
    /// Sends a request to the peer for a specific piece of data and waits for
    /// the response. Upon receiving the piece, it updates the peer's state to
    /// reflect that it is now in the downloading state.
    #[tracing::instrument(
        name = "GetPiece"
        skip_all
        fields(peer = %self.get_addr())
    )]
    #[inline]
    pub async fn download_piece(mut self) -> anyhow::Result<Peer<Downloading>> {
        // NOTE: Now comes the hard part... Send request for each piece in the torrent file.
        // TODO: Implement this bitch.

        let mut buf = bytes::BytesMut::with_capacity(BLOCK_MAX as usize);

        tracing::debug!("Sending request message");

        self.send_peer_message(PeerMessage::request(0, 0, BLOCK_MAX))
            .timeout(TIMEOUT_DURATION)
            .await??;

        tracing::debug!("Request Message Sent");

        tracing::debug!("Seeking piece message");

        let piece = self
            .recv_peer_message::<Piece>(&mut buf)
            .timeout(TIMEOUT_DURATION)
            .await??;

        tracing::debug!("Piece message received");

        Ok(Peer {
            addr: self.addr,
            state: Downloading {
                stream: self.get_stream_owned(),
                piece,
            },
        })
    }

    /// Get a reference to the [`Bitfield`] of the unchoked [`Peer`].
    pub fn get_bitfield(&self) -> &Bitfield {
        self.state.bitfield.payload()
    }
}

/// Obtained from [`Peer::download_piece`]
impl Peer<Downloading> {
    /// Get a reference to the [`Piece`] of the downloading [`Peer`].
    pub fn get_downloaded_piece(&self) -> &Piece {
        self.state.piece.payload()
    }
}

/// Methods for a [`Peer`] that is atleast [handshaken](Peer::handshake)
impl<T> Peer<T>
where
    T: ConnectedPeer,
{
    /// Get a mutable reference to the TCP stream if the [`handshake`](Self::handshake) was successful.
    pub fn get_stream_mut(&mut self) -> &mut TcpStream {
        self.state.get_stream_mut()
    }

    /// Get a owned reference to the TCP stream if the [`handshake`](Self::handshake) was successful.
    pub fn get_stream_owned(self) -> TcpStream {
        self.state.get_stream_owned()
    }

    /// Reads and parses a peer message from the [`Peer`] stream.
    ///
    /// This method reads bytes from the stream into the provided buffer until a complete peer
    /// message is received.     
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The stream is closed unexpectedly
    /// - The message length is invalid
    /// - The message tag doesn't match the expected type
    /// - The payload cannot be parsed
    pub async fn recv_peer_message<P>(&mut self, buf: &mut BytesMut) -> Result<PeerMessage<P>>
    where
        P: PeerMessagePayload,
    {
        {
            let stream = self.get_stream_mut();

            loop {
                let read = stream.read_buf(buf).await?;

                if read == 0 {
                    continue;
                }

                if buf.len() < 4 {
                    tracing::trace!("Peer sent less than 4 bytes. RETRYING");
                    continue;
                }

                let len = u32::from_be_bytes(buf[0..4].try_into()?);

                if len as usize > buf.len() {
                    tracing::trace!(
                        "Peer sent insuffecient data, expected: {len}, recv: {}. RETRYING",
                        buf.len()
                    );

                    continue;
                }

                ensure!(len as usize <= (buf.len() - 4));

                let tag = PeerMessagesTag::try_from(buf[4]).map_err(|e| anyhow!(e))?;

                ensure!(tag == P::message_tag());

                let payload = P::from_bytes(&buf[5..(5 + len - 1) as usize])?;

                buf.truncate(len as usize + 4);

                let message: PeerMessage<P> = PeerMessage::new(len, tag, payload);

                break Ok(message);
            }
        }
    }

    /// Receives an [`unchoke`](PeerMessage::unchoke) message from the stream.
    ///
    /// This is a specialized method for receiving unchoke messages that is more efficient than
    /// `recv_peer_message` since unchoke messages have a fixed size of 5 bytes (4 bytes length + 1 byte tag).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The stream is closed unexpectedly
    /// - The message is not a valid unchoke message
    pub async fn recv_unchoke_message(&mut self) -> Result<PeerMessage<Unchoke>> {
        let mut buf = [0; 5];

        self.get_stream_mut().read_exact(&mut buf).await?;

        PeerMessage::from_bytes(&buf)
    }

    /// Sends a peer message over the stream.
    ///
    /// This method serializes the message according to the BitTorrent peer protocol and writes it
    /// to the stream.
    ///
    /// # Returns
    ///
    /// A `Future` that resolves to a `Result<()>` indicating whether the message was sent successfully.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The stream is closed unexpectedly
    /// - The message cannot be written completely
    pub async fn send_peer_message<'a, P>(&'a mut self, message: PeerMessage<P>) -> Result<usize>
    where
        P: 'a + PeerMessagePayload + Send,
    {
        let stream = self.get_stream_mut();
        let written = stream.write(&message.to_bytes()).await?;

        stream.flush().await?;

        ensure!(message.size() == written);

        Ok(written)
    }
}

impl<T> Peer<T> {
    /// Returns the [`SocketAddr`] of the Peer.
    pub const fn get_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Get ip addr octests
    fn get_octets(&self) -> Vec<u8> {
        match &self.addr {
            SocketAddr::V4(socket_addr_v4) => {
                let mut buff = Vec::with_capacity(4);
                buff.extend(socket_addr_v4.ip().octets());
                buff
            }
            SocketAddr::V6(socket_addr_v6) => {
                let mut buff = Vec::with_capacity(16);
                buff.extend(socket_addr_v6.ip().octets());
                buff
            }
        }
    }
}

impl Clone for Peer<Unconnected> {
    fn clone(&self) -> Self {
        Self {
            addr: self.addr,
            state: Unconnected,
        }
    }
}

impl<T> Hash for Peer<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.addr.hash(state);
    }
}

impl<T> PartialEq for Peer<T> {
    fn eq(&self, other: &Self) -> bool {
        self.addr == other.addr
    }
}

impl<T> PartialEq<SocketAddr> for Peer<T> {
    fn eq(&self, other: &SocketAddr) -> bool {
        &self.addr == other
    }
}

impl<T> PartialEq<SocketAddrV4> for Peer<T> {
    fn eq(&self, other: &SocketAddrV4) -> bool {
        match &self.addr {
            SocketAddr::V4(socket_addr_v4) => socket_addr_v4 == other,
            SocketAddr::V6(..) => false,
        }
    }
}

impl<T> PartialEq<SocketAddrV6> for Peer<T> {
    fn eq(&self, other: &SocketAddrV6) -> bool {
        match &self.addr {
            SocketAddr::V4(..) => false,
            SocketAddr::V6(socket_addr_v6) => socket_addr_v6 == other,
        }
    }
}

impl<T> Eq for Peer<T> {}

impl From<SocketAddrV4> for Peer<Unconnected> {
    fn from(value: SocketAddrV4) -> Self {
        Self {
            addr: SocketAddr::from(value),
            state: Unconnected,
        }
    }
}

impl From<SocketAddrV6> for Peer<Unconnected> {
    fn from(value: SocketAddrV6) -> Self {
        Self {
            addr: SocketAddr::from(value),
            state: Unconnected,
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
/// [`TrackerResponse`]: crate::trackers::TrackerResponse
/// [`Tracker`]: crate::trackers::Tracker
/// [`announce`]: crate::trackers::Tracker::announce
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

    /// Returns `true` if this `PeersList` contains IPv4 peers.
    ///
    /// This method can be used to check if the tracker response included any IPv4 peers
    /// before attempting to iterate over them.
    pub const fn contains_peers_v4(&self) -> bool {
        self.peers.is_some()
    }

    /// Returns `true` if this `PeersList` contains IPv6 peers.
    ///
    /// This method can be used to check if the tracker response included any IPv6 peers
    /// before attempting to iterate over them.
    pub const fn contains_peers_v6(&self) -> bool {
        self.peers6.is_some()
    }

    /// Converts the peer list into a vector of [`Peer`] instances.
    ///
    /// This method combines both IPv4 and IPv6 peers into a single vector.
    /// If either type of peers is not present, an empty slice is used instead.
    ///
    /// # Returns
    ///
    /// A `Vec<Peer>` containing all peers from both IPv4 and IPv6 lists.
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

    /// Returns the total number of peers in the list, combining both IPv4 and IPv6 peers.
    ///
    /// This method counts all peers from both the IPv4 and IPv6 lists, even if one of them is empty.
    ///
    /// # Returns
    ///
    /// The total count of peers across both IPv4 and IPv6 lists.
    pub fn num_of_peers(&self) -> usize {
        let v4_len = self.peers.as_ref().map(|p| p.0.len()).unwrap_or(0);
        let v6_len = self.peers6.as_ref().map(|p| p.0.len()).unwrap_or(0);

        v4_len + v6_len
    }

    /// Returns an iterator over all peers in the list.
    ///
    /// The iterator will first yield all IPv4 peers, followed by all IPv6 peers.
    /// If either type of peers is not present, the iterator will seamlessly move
    /// to the next available type.
    ///
    /// # Returns
    ///
    /// A [`PeersIter`] that iterates over references to all peers in the list.
    pub fn iter(&self) -> PeersIter<'_> {
        PeersIter::new(self)
    }
}

/// An iterator over the [`PeersList`] that returns a [`Peer`] on each iteration.
///
/// Accessed through the [`PeersList::iter`] method
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
