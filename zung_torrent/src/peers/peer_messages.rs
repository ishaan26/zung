use std::{fmt::Display, future::Future, ops::Deref};

use bytes::{BufMut, Bytes, BytesMut};

use anyhow::{anyhow, ensure, Result};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
};

/////////////////////////////////////////////////////////////////////////////
//                                  Main Message
/////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy)]

/// Represents a peer message with a tag and a payload.
///
/// The Peer Wire Protocol is used for communication between peers in a BitTorrent network. Each
/// message has a `tag` identifying the type of the message (e.g., `Choke`, `Unchoke`,
/// `Interested`, etc.) and a `payload` containing any associated data.
pub struct PeerMessage<T> {
    len: u32,
    tag: PeerMessagesTag,
    payload: T,
}

impl<T> PeerMessage<T> {
    pub(crate) const fn new(len: u32, tag: PeerMessagesTag, payload: T) -> PeerMessage<T>
    where
        T: PeerMessagePayload,
    {
        Self { len, tag, payload }
    }
}

impl PeerMessage<()> {
    /// Creates a `PeerMessage` representing the `choke` message.
    ///
    /// `Choke` is a notification that no requests will be answered until the client is unchoked. The
    /// client should not attempt to send requests for blocks, and it should consider all pending
    /// (unanswered) requests to be discarded by the remote peer.
    pub const fn choke() -> PeerMessage<ChokePayload> {
        PeerMessage {
            len: (size_of::<ChokePayload>() + 1) as u32,
            tag: PeerMessagesTag::Choke,
            payload: ChokePayload,
        }
    }

    /// Creates a `PeerMessage` representing the `unchoke` message.
    ///
    /// This message, as the name suggests, opens the chocked connection.
    pub const fn unchoke() -> PeerMessage<UnchokePayload> {
        PeerMessage {
            len: (size_of::<UnchokePayload>() + 1) as u32,
            tag: PeerMessagesTag::Unchoke,
            payload: UnchokePayload,
        }
    }

    /// Creates a `PeerMessage` representing the `interested` message.
    ///
    /// This message is used to inform the receiver that the sender is interested in downloading
    /// pieces the peer on the other hand has.
    pub const fn interested() -> PeerMessage<InterestedPayload> {
        PeerMessage {
            len: (size_of::<InterestedPayload>() + 1) as u32,
            tag: PeerMessagesTag::Interested,
            payload: InterestedPayload,
        }
    }

    /// Creates a `PeerMessage` representing the `not interested` message.
    ///
    /// This message is used to inform the receiver that the sender is not interested in
    /// downloading pieces.
    pub const fn not_interested() -> PeerMessage<NotInterestedPayload> {
        PeerMessage {
            len: (size_of::<NotInterestedPayload>() + 1) as u32,
            tag: PeerMessagesTag::NotInterested,
            payload: NotInterestedPayload,
        }
    }

    /// Creates a `PeerMessage` representing the `have` message.
    ///
    /// This messavge just informs the index which that downloader just completed and checked the
    /// hash of.
    pub const fn have(index: u32) -> PeerMessage<HavePayload> {
        PeerMessage {
            len: (size_of::<HavePayload>() + 1) as u32,
            tag: PeerMessagesTag::Have,
            payload: HavePayload {
                index: index.to_be_bytes(),
            },
        }
    }

    /// Creates a `PeerMessage` representing the `bitfield` message.
    ///
    /// The bitfield message may only be sent immediately after the handshaking sequence is
    /// completed, and before any other messages are sent. It is optional, and need not be sent if
    /// a client has no pieces.
    ///
    /// The bitfield message is variable length, where X is the length of the bitfield. The payload
    /// is a bitfield representing the pieces that have been successfully downloaded. The high bit
    /// in the first byte corresponds to piece index 0. Bits that are cleared indicated a missing
    /// piece, and set bits indicate a valid and available piece. Spare bits at the end are set to
    /// zero.
    ///
    /// A bitfield of the wrong length is considered an error. Clients should drop the connection
    /// if they receive bitfields that are not of the correct size, or if the bitfield has any of
    /// the spare bits set.
    pub const fn bitfield(bitfield: &'static [u8]) -> PeerMessage<BitfieldPayload> {
        let payload = BitfieldPayload(Bytes::from_static(bitfield));
        PeerMessage {
            // NOTE: Cannot use std::mem::size_of here as that would return the size of the fat
            // pointer of the slice instead of the actual size of the underlying slice.

            //<len=0001+X>
            len: 1 + payload.get_size_const() as u32,
            tag: PeerMessagesTag::Bitfield,
            payload,
        }
    }

    /// Creates a `PeerMessage` representing the `request` message.
    ///
    /// A request message is used to request a block.
    pub const fn request(index: u32, begin: u32, length: u32) -> PeerMessage<RequestPayload> {
        PeerMessage {
            len: (size_of::<RequestPayload>() + 1) as u32,
            tag: PeerMessagesTag::Request,
            payload: RequestPayload {
                index: index.to_be_bytes(),
                begin: begin.to_be_bytes(),
                length: length.to_be_bytes(),
            },
        }
    }

    /// Creates a `PeerMessage` representing the `piece` message.
    ///
    /// A request message is used to request a block.
    pub fn piece(index: u32, begin: u32, block: &'static [u8]) -> PeerMessage<PiecePayload> {
        let meta = PiecePayloadMetaData {
            index: index.to_be_bytes(),
            begin: begin.to_be_bytes(),
        };

        let mut all = BytesMut::with_capacity(8 + block.len());

        all.put_slice(&meta.index);
        all.put_slice(&meta.begin);
        all.put_slice(block);

        let payload = PiecePayload {
            meta_data: meta,
            all_bytes: all.freeze(),
        };

        PeerMessage {
            // <len=0009+X> where X is the length of the block
            len: 9 + payload.block_len() as u32,
            tag: PeerMessagesTag::Piece,
            payload,
        }
    }
}

impl<T> PeerMessage<T>
where
    T: PeerMessagePayload,
{
    /// Calculates the size of the message in bytes.
    ///
    /// The size is determined by:
    /// - 4 bytes for the message length.
    /// - 1 byte for the message tag.
    /// - The size of the payload, calculated via the [`PeerMessagePayload`] trait.
    pub fn size(&self) -> usize {
        4 /*len bytes*/ +  1 /* tag byte*/  + self.payload.size()
    }

    /// Converts the `PeerMessage` into a byte array.
    pub fn to_bytes(&self) -> Bytes {
        let payload_bytes = self.payload.as_bytes();

        let mut bytes = BytesMut::with_capacity(self.size());

        bytes.put_u32(self.len);
        bytes.put_u8(self.tag as u8);
        bytes.put_slice(payload_bytes);

        bytes.freeze()
    }

    /// Constructs a `PeerMessage` from a byte array.
    ///
    /// This method deserializes a `PeerMessage` from its byte representation. It expects:
    ///
    /// - The first 4 bytes to contain the message length in big-endian format.
    /// - The 5th byte to contain the message tag.
    /// - The remaining bytes to contain the payload, if any.
    ///
    /// # Returns
    ///
    /// A `Result` containing the [`PeerMessage`] if deserialization succeeds, or an error if it fails.
    ///
    /// # Errors
    ///
    /// - Returns an error if the `bytes` slice is too short.
    /// - Returns an error if the message tag does not match the expected tag for the payload type.
    /// - Returns an error if deserialization of the payload fails.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let len = u32::from_be_bytes(bytes[0..4].try_into()?);

        ensure!(len as usize <= (bytes.len() - 4));

        let tag = PeerMessagesTag::try_from(bytes[4]).map_err(|e| anyhow!(e))?;
        ensure!(tag == T::message_tag());

        let payload = if bytes.len() > 5 {
            T::from_bytes(&bytes[5..(5 + len - 1) as usize])?
        } else {
            ensure!(
                len == 1,
                "There is no payload. length must be 1 but found {len}"
            );

            T::from_bytes(&[])?
        };

        Ok(Self { len, tag, payload })
    }

    /// Returns a reference to the payload contained in the message.
    pub fn payload(&self) -> &T {
        &self.payload
    }

    /// Returns the `tag` of the peer message
    pub fn tag(&self) -> PeerMessagesTag {
        self.tag
    }

    /// Returns the calculated len of the peer message
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> u32 {
        self.len
    }
}

/// Extension trait for parsing peer messages from byte slices.
///
/// This trait provides methods to parse byte slices into `PeerMessage` structures,
/// which represent messages exchanged between peers in a network. It is implemented
/// for any type that can be referenced as a byte slice (`AsRef<[u8]>`), such as
/// `Vec<u8>`, `&[u8]`, or `Bytes`. This makes it versatile for handling raw byte
/// data, typically received from a network in peer-to-peer protocols.
///
/// # Examples
///
/// Assuming you have a byte slice containing a peer message:
///
/// ```rust
/// use zung_torrent::peers::{PeerMessageExt, PeerMessage, BitfieldPayload};
///
/// fn example(bytes: &[u8]) {
///     let message = bytes.parse_peer_message::<BitfieldPayload>();
///
///     match message {
///         Ok(msg) => println!("Parsed message: {:?}", msg),
///         Err(e) => eprintln!("Failed to parse: {}", e),
///     }
/// }
/// ```
///
/// Note that parsing may fail if the byte slice is malformed or does not match
/// the expected format for the specified payload type.
pub trait PeerMessageExt: AsRef<[u8]> {
    /// Parses the byte slice into a `PeerMessage` with the specified payload type `T`.
    ///
    /// This method attempts to interpret the byte slice as a peer message with a payload
    /// of type `T`, where `T` must implement `PeerMessagePayload`.
    fn parse_peer_message<T>(&self) -> Result<PeerMessage<T>>
    where
        T: PeerMessagePayload,
    {
        PeerMessage::from_bytes(self.as_ref())
    }

    /// Parses the byte slice into a [`PeerMessage`] with a [`ChokePayload`].
    fn parse_choke(&self) -> Result<PeerMessage<ChokePayload>> {
        PeerMessage::from_bytes(self.as_ref())
    }

    /// Parses the byte slice into a [`PeerMessage`] with a [`UnchokePayload`].
    fn parse_unchoke(&self) -> Result<PeerMessage<UnchokePayload>> {
        PeerMessage::from_bytes(self.as_ref())
    }

    /// Parses the byte slice into a [`PeerMessage`] with a [`InterestedPayload`].
    fn parse_interested(&self) -> Result<PeerMessage<InterestedPayload>> {
        PeerMessage::from_bytes(self.as_ref())
    }

    /// Parses the byte slice into a [`PeerMessage`] with a [`NotInterestedPayload`].
    fn parse_not_interested(&self) -> Result<PeerMessage<NotInterestedPayload>> {
        PeerMessage::from_bytes(self.as_ref())
    }

    /// Parses the byte slice into a [`PeerMessage`] with a [`BitfieldPayload`].
    fn parse_bitfield(&self) -> Result<PeerMessage<BitfieldPayload>> {
        PeerMessage::from_bytes(self.as_ref())
    }

    /// Parses the byte slice into a [`PeerMessage`] with a [`PiecePayload`].
    fn parse_piece(&self) -> Result<PeerMessage<PiecePayload>> {
        PeerMessage::from_bytes(self.as_ref())
    }
}

impl PeerMessageExt for &[u8] {}

/////////////////////////////////////////////////////////////////////////////
//                                  Tag
/////////////////////////////////////////////////////////////////////////////

/// Tags of the torrent peer wire protocol
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerMessagesTag {
    //<len=0001><id=0>
    // The choke message is fixed-length and has no payload.
    Choke = 0,

    // <len=0001><id=1>
    // The unchoke message is fixed-length and has no payload.
    Unchoke = 1,

    // <len=0001><id=2>
    // The interested message is fixed-length and has no payload.
    Interested = 2,

    // <len=0001><id=3>
    // The not interested message is fixed-length and has no payload.
    NotInterested = 3,

    // <len=0005><id=4><piece index>
    //
    //The have message is fixed length. The payload is the zero-based index of a piece that has
    //just been successfully downloaded and verified via the hash.
    Have = 4,

    //  <len=0001+X><id=5><bitfield>
    //
    // The bitfield message may only be sent immediately after the handshaking sequence is
    // completed, and before any other messages are sent. It is optional, and need not be sent if a
    // client has no pieces.
    Bitfield = 5,

    // request: <len=0013><id=6><index><begin><length>
    //
    // The request message is fixed length, and is used to request a block. The payload contains
    // the following information:
    //
    //  index: integer specifying the zero-based piece index
    //  begin: integer specifying the zero-based byte offset within the piece
    //  length: integer specifying the requested length.
    Request = 6,

    // piece: <len=0009+X><id=7><index><begin><block>
    //
    // The piece message is variable length, where X is the length of the block. The payload
    // contains the following information:
    //
    // index: integer specifying the zero-based piece index
    // begin: integer specifying the zero-based byte offset within the piece
    // block: block of data, which is a subset of the piece specified by index.
    Piece = 7,
}

impl TryFrom<u8> for PeerMessagesTag {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Choke),
            1 => Ok(Self::Unchoke),
            2 => Ok(Self::Interested),
            3 => Ok(Self::NotInterested),
            4 => Ok(Self::Have),
            5 => Ok(Self::Bitfield),
            6 => Ok(Self::Request),
            7 => Ok(Self::Piece),
            tag => Err(anyhow!("Invalid PeerMessagesTag value: {tag}")),
        }
    }
}

impl Display for PeerMessagesTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PeerMessagesTag::Choke => f.write_str("choke"),
            PeerMessagesTag::Unchoke => f.write_str("unchoke"),
            PeerMessagesTag::Interested => f.write_str("interested"),
            PeerMessagesTag::NotInterested => f.write_str("not interested"),
            PeerMessagesTag::Have => f.write_str("have"),
            PeerMessagesTag::Bitfield => f.write_str("bitfield"),
            PeerMessagesTag::Request => f.write_str("request"),
            PeerMessagesTag::Piece => f.write_str("piece"),
        }
    }
}

/////////////////////////////////////////////////////////////////////////////
//                                  Payload
/////////////////////////////////////////////////////////////////////////////

/// A common trait of all [`PeerMessage`] payloads.
pub trait PeerMessagePayload
where
    Self: Sized,
{
    /// Get the byte reprasentation of the Payload type. The byte conversino is indented to be zero
    /// cost. Therefore, the structs contain the byte values themselves and this method with just
    /// cast the structs into a slice of bytes.
    fn as_bytes(&self) -> &[u8];

    fn from_bytes(bytes: &[u8]) -> Result<Self>;

    fn message_tag() -> PeerMessagesTag;

    fn size(&self) -> usize {
        std::mem::size_of::<Self>()
    }

    fn is_empty_payload(&self) -> bool {
        self.size() == 0
    }
}

/// Payload of the [`PeerMessage::choke`] message.
#[repr(C)]
#[derive(Debug, PartialEq, Eq)]
pub struct ChokePayload;

impl PeerMessagePayload for ChokePayload {
    fn as_bytes(&self) -> &[u8] {
        &[]
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.is_empty());
        Ok(Self)
    }

    fn message_tag() -> PeerMessagesTag {
        PeerMessagesTag::Choke
    }
}

/// Payload of the [`PeerMessage::unchoke`] message.
#[repr(C)]
#[derive(Debug, PartialEq, Eq)]
pub struct UnchokePayload;

impl PeerMessagePayload for UnchokePayload {
    fn as_bytes(&self) -> &[u8] {
        &[]
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.is_empty());
        Ok(Self)
    }

    fn message_tag() -> PeerMessagesTag {
        PeerMessagesTag::Unchoke
    }
}

/// Payload of the [`PeerMessage::interested`] message.
#[repr(C)]
#[derive(Debug, PartialEq, Eq)]
pub struct InterestedPayload;

impl PeerMessagePayload for InterestedPayload {
    fn as_bytes(&self) -> &[u8] {
        &[]
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.is_empty());
        Ok(Self)
    }

    fn message_tag() -> PeerMessagesTag {
        PeerMessagesTag::Interested
    }
}

/// Payload of the [`PeerMessage::not_interested`] message.
#[repr(C)]
#[derive(Debug, PartialEq, Eq)]
pub struct NotInterestedPayload;

impl PeerMessagePayload for NotInterestedPayload {
    fn as_bytes(&self) -> &[u8] {
        &[]
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.is_empty());
        Ok(Self)
    }

    fn message_tag() -> PeerMessagesTag {
        PeerMessagesTag::NotInterested
    }
}

/// Payload of the [`PeerMessage::have`] message.
///
///The 'have' message's payload is a single number, the index which that downloader just completed
///and checked the hash of.
#[repr(C)]
#[derive(Debug, PartialEq, Eq)]
pub struct HavePayload {
    // the zero-based index of a piece that has just been successfully downloaded and verified via
    // the hash.
    index: [u8; 4],
}

impl PeerMessagePayload for HavePayload {
    fn as_bytes(&self) -> &[u8] {
        self.index.as_ref()
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() == 4);

        Ok(Self {
            // Unwrap is fine since len is already ensured.
            index: bytes.try_into().unwrap(),
        })
    }

    fn message_tag() -> PeerMessagesTag {
        PeerMessagesTag::Have
    }
}

/// Payload of the [`PeerMessage::bitfield`] message.
///
///The bitfield message is variable length, where X is the length of the bitfield. The payload is a
///bitfield representing the pieces that have been successfully downloaded. The high bit in the
///first byte corresponds to piece index 0. Bits that are cleared indicated a missing piece, and
///set bits indicate a valid and available piece. Spare bits at the end are set to zero.
#[repr(C)]
#[derive(Debug, PartialEq, Eq)]
pub struct BitfieldPayload(Bytes);

impl PeerMessagePayload for BitfieldPayload {
    fn as_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Ok(Self(Bytes::copy_from_slice(bytes)))
    }

    fn message_tag() -> PeerMessagesTag {
        PeerMessagesTag::Bitfield
    }

    fn size(&self) -> usize {
        self.len()
    }
}

impl BitfieldPayload {
    const fn get_size_const(&self) -> usize {
        self.0.len()
    }
}

impl Deref for BitfieldPayload {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

/// Payload of the [`PeerMessage::request`] message.
///
///`request` messages contain an index, begin, and length. The last two are byte offsets. Length is
///generally a power of two unless it gets truncated by the end of the file. All current
///implementations use 2^14 (16 kiB), and close connections which request an amount greater than
///that.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestPayload {
    // integer specifying the zero-based piece index
    index: [u8; 4],
    // integer specifying the zero-based byte offset within the piece
    begin: [u8; 4],
    // integer specifying the requested length.
    length: [u8; 4],
}

impl PeerMessagePayload for RequestPayload {
    fn as_bytes(&self) -> &[u8] {
        let bytes = self as *const Self as *const [u8; size_of::<Self>()];
        unsafe { &*bytes }
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() == 12);

        Ok(RequestPayload {
            index: bytes[0..4].try_into()?,
            begin: bytes[4..8].try_into()?,
            length: bytes[8..12].try_into()?,
        })
    }

    fn message_tag() -> PeerMessagesTag {
        PeerMessagesTag::Request
    }
}

// TODO: See if there is a better more optimized way for handle piece payload.

/// Payload of the [`PeerMessage::piece`] message.
///
/// In the BitTorrent protocol, a piece message contains the actual data being transferred.
/// Each piece message includes:
/// - An index identifying which piece this block belongs to
/// - A begin offset indicating where in the piece this block starts
/// - The actual block data
#[derive(Debug, PartialEq, Eq)]
pub struct PiecePayload {
    meta_data: PiecePayloadMetaData,
    all_bytes: Bytes,
}

/// Metadata portion of a piece message containing the piece index and byte offset.
#[repr(C)]
#[derive(Debug, PartialEq, Eq)]
struct PiecePayloadMetaData {
    /// The zero-based piece index (4 bytes in big-endian format)
    index: [u8; 4],

    /// The zero-based byte offset within the piece (4 bytes in big-endian format)
    begin: [u8; 4],
}

impl PeerMessagePayload for PiecePayload {
    fn as_bytes(&self) -> &[u8] {
        &self.all_bytes
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() >= 9);

        Ok(Self {
            meta_data: PiecePayloadMetaData {
                index: bytes[0..4].try_into()?,
                begin: bytes[4..8].try_into()?,
            },
            all_bytes: Bytes::copy_from_slice(bytes),
        })
    }

    fn message_tag() -> PeerMessagesTag {
        PeerMessagesTag::Piece
    }
}

impl PiecePayload {
    /// The size in bytes of the metadata portion (index + begin) of a piece message.
    pub const META_DATA_SIZE: usize = std::mem::size_of::<PiecePayloadMetaData>();

    /// Returns the length of the `block` data in bytes.
    ///
    /// This excludes the metadata portion (index and begin) of the message.
    pub const fn block_len(&self) -> usize {
        self.all_bytes.len() - Self::META_DATA_SIZE
    }

    /// Returns the `piece index` as a u32 value.
    ///
    /// The index identifies which piece of the torrent this block belongs to.
    pub const fn index(&self) -> u32 {
        u32::from_be_bytes(self.meta_data.index)
    }

    /// Returns the `begin` offset as a u32 value.
    ///
    /// The begin offset indicates the starting position of this block within the piece.
    pub const fn begin(&self) -> u32 {
        u32::from_be_bytes(self.meta_data.begin)
    }

    /// Returns the actual `block` data as a Bytes object.
    ///
    /// This excludes the metadata portion (index and begin) of the message.
    pub fn block(&self) -> Bytes {
        self.all_bytes.slice(Self::META_DATA_SIZE..)
    }
}

/////////////////////////////////////////////////////////////////////////////
//                                  Frame
/////////////////////////////////////////////////////////////////////////////

/// A trait for sending (encoded) and receiving (devcoded) BitTorrent peer protocol messages over
/// asynchronous streams.
///
/// This trait extends [`AsyncRead`], [`AsyncWrite`] and [`Unpin`] to provide specialized
/// methods for peer message handling.
///
/// ## Example Usage
///
/// ```rust
/// use tokio::net::TcpStream;
/// use bytes::BytesMut;
/// use zung_torrent::peers::{PeerMessage, PeerMessageFrame, BitfieldPayload};
///
/// # async fn handle_peer(stream: TcpStream) -> anyhow::Result<()> {
/// let mut stream = stream;
/// let mut buf = BytesMut::with_capacity(4096);
///
/// // Receive a BitField message
/// let bitfield_msg = stream.recv_peer_message::<BitfieldPayload>(&mut buf).await?;
///
/// // Send an Interested message
/// let interested_msg = PeerMessage::interested();
/// stream.send_peer_message(interested_msg).await?;
///
/// // Receive an Unchoke message using specialized method. Since the size of the unchoke
/// // message always remains the same, this method is more efficient than `recv_peer_message`
/// let unchoke_msg = stream.recv_unchoke_message().await?;
///
/// # Ok(())
/// # }
/// ```
pub trait PeerMessageFrame: AsyncRead + AsyncWrite + Unpin {
    /// Reads and parses a peer message from the stream.
    ///
    /// This method reads bytes from the stream into the provided buffer until a complete peer
    /// message is received.     
    ///
    /// # Returns
    ///
    /// A `Future` that resolves to a `Result<PeerMessage<T>>` where:
    /// - `T` is the type of payload expected in the message
    /// - The message is parsed according to the BitTorrent peer protocol specification
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The stream is closed unexpectedly
    /// - The message length is invalid
    /// - The message tag doesn't match the expected type
    /// - The payload cannot be parsed
    fn recv_peer_message<T>(
        &mut self,
        buf: &mut BytesMut,
    ) -> impl Future<Output = Result<PeerMessage<T>>>
    where
        T: PeerMessagePayload,
    {
        async {
            loop {
                let read = self.read_buf(buf).await?;

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

                ensure!(tag == T::message_tag());

                let payload = T::from_bytes(&buf[5..(5 + len - 1) as usize])?;

                buf.truncate(len as usize + 4);

                let message: PeerMessage<T> = PeerMessage::new(len, tag, payload);

                break Ok(message);
            }
        }
    }

    /// Receives an `[unchoke](PeerMessage::unchoke)` message from the stream.
    ///
    /// This is a specialized method for receiving unchoke messages that is more efficient than
    /// `recv_peer_message` since unchoke messages have a fixed size of 5 bytes (4 bytes length + 1 byte tag).
    ///
    /// # Returns
    ///
    /// A `Future` that resolves to a `Result<PeerMessage<UnchokePayload>>`.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The stream is closed unexpectedly
    /// - The message is not a valid unchoke message
    fn recv_unchoke_message(
        &mut self,
    ) -> impl Future<Output = Result<PeerMessage<UnchokePayload>>> {
        async {
            let mut buf = [0; 5];

            self.read_exact(&mut buf).await?;

            PeerMessage::from_bytes(&buf)
        }
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
    fn send_peer_message<'a, T>(
        &'a mut self,
        message: PeerMessage<T>,
    ) -> impl Future<Output = Result<()>>
    where
        T: 'a + PeerMessagePayload + Send,
    {
        async move {
            let written = self.write(&message.to_bytes()).await?;
            self.flush().await?;

            ensure!(message.size() == written);

            Ok(())
        }
    }
}

impl PeerMessageFrame for TcpStream {}

/////////////////////////////////////////////////////////////////////////////
//                                  Tests
/////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod peer_messages_test {

    use std::sync::LazyLock;

    use super::*;

    const CHOKE: PeerMessage<ChokePayload> = PeerMessage::choke();
    const UNCHOKE: PeerMessage<UnchokePayload> = PeerMessage::unchoke();
    const INTERESTED: PeerMessage<InterestedPayload> = PeerMessage::interested();
    const NOT_INTERESTED: PeerMessage<NotInterestedPayload> = PeerMessage::not_interested();
    const HAVE: PeerMessage<HavePayload> = PeerMessage::have(0);
    const BITFIELD: PeerMessage<BitfieldPayload> = PeerMessage::bitfield(&[1, 2, 3, 4, 5, 6, 7, 8]);
    const REQUEST: PeerMessage<RequestPayload> = PeerMessage::request(1, 2, 3);
    static PIECE: LazyLock<PeerMessage<PiecePayload>> =
        LazyLock::new(|| PeerMessage::piece(1, 2, &[1, 2, 3, 4, 5]));

    #[test]
    fn check_lens() {
        assert_eq!(CHOKE.len, 1);
        assert_eq!(UNCHOKE.len, 1);
        assert_eq!(INTERESTED.len, 1);
        assert_eq!(NOT_INTERESTED.len, 1);
        assert_eq!(HAVE.len, 5);
        assert_eq!(BITFIELD.len, 9);
        assert_eq!(REQUEST.len, 13);
        assert_eq!(PIECE.len, 9 + 5);
    }

    #[test]
    fn check_tag() {
        assert_eq!(CHOKE.tag, PeerMessagesTag::Choke);
        assert_eq!(CHOKE.tag as u8, 0);

        assert_eq!(UNCHOKE.tag, PeerMessagesTag::Unchoke);
        assert_eq!(UNCHOKE.tag as u8, 1);

        assert_eq!(INTERESTED.tag, PeerMessagesTag::Interested);
        assert_eq!(INTERESTED.tag as u8, 2);

        assert_eq!(NOT_INTERESTED.tag, PeerMessagesTag::NotInterested);
        assert_eq!(NOT_INTERESTED.tag as u8, 3);

        assert_eq!(HAVE.tag, PeerMessagesTag::Have);
        assert_eq!(HAVE.tag as u8, 4);

        assert_eq!(BITFIELD.tag, PeerMessagesTag::Bitfield);
        assert_eq!(BITFIELD.tag as u8, 5);

        assert_eq!(REQUEST.tag, PeerMessagesTag::Request);
        assert_eq!(REQUEST.tag as u8, 6);

        assert_eq!(PIECE.tag, PeerMessagesTag::Piece);
        assert_eq!(PIECE.tag as u8, 7);
    }

    #[test]
    fn check_payload() {
        assert_eq!(CHOKE.payload, ChokePayload);
        assert_eq!(UNCHOKE.payload, UnchokePayload);
        assert_eq!(INTERESTED.payload, InterestedPayload);
        assert_eq!(NOT_INTERESTED.payload, NotInterestedPayload);
        assert_eq!(
            HAVE.payload,
            HavePayload {
                index: 0_u32.to_be_bytes()
            }
        );
        assert_eq!(
            BITFIELD.payload,
            BitfieldPayload(Bytes::from_static(&[1, 2, 3, 4, 5, 6, 7, 8]))
        );
        assert_eq!(
            REQUEST.payload,
            RequestPayload {
                index: 1_u32.to_be_bytes(),
                begin: 2_u32.to_be_bytes(),
                length: 3_u32.to_be_bytes()
            }
        );

        assert_eq!(
            PIECE.payload,
            PiecePayload {
                meta_data: PiecePayloadMetaData {
                    index: 1_u32.to_be_bytes(),
                    begin: 2_u32.to_be_bytes(),
                },
                all_bytes: Bytes::copy_from_slice(&[0, 0, 0, 1, 0, 0, 0, 2, 1, 2, 3, 4, 5])
            }
        );
    }

    #[test]
    fn check_payload_as_bytes() {
        assert_eq!(
            REQUEST.payload.as_bytes(),
            [0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3]
        )
    }
}
