use std::ops::Deref;

use anyhow::{anyhow, ensure, Result};
use bytes::{BufMut, Bytes, BytesMut};

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
            len: payload.get_size_const() as u32 + 1,
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
}

impl<T> PeerMessage<T>
where
    T: PeerMessagePayload + Sized,
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

        let mut bytes = BytesMut::with_capacity(dbg!(self.size()));

        bytes.put_u32(dbg!(self.len));
        bytes.put_u8(dbg!(self.tag as u8));
        bytes.put_slice(dbg!(payload_bytes));

        bytes.freeze()
    }

    /// Constructs a `PeerMessage` from a byte array.
    ///
    /// This method deserializes a `PeerMessage` from its byte representation. It expects:
    /// - The first 4 bytes to contain the message length in big-endian format.
    /// - The 5th byte to contain the message tag.
    /// - The remaining bytes to contain the payload, if any.
    ///
    /// # Returns
    /// A `Result` containing the [`PeerMessage`] if deserialization succeeds, or an error if it fails.
    ///
    /// # Errors
    /// - Returns an error if the `bytes` slice is too short.
    /// - Returns an error if the message tag does not match the expected tag for the payload type.
    /// - Returns an error if deserialization of the payload fails.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let len = u32::from_be_bytes(bytes[0..4].try_into()?);

        let tag = PeerMessagesTag::try_from(bytes[4]).map_err(|e| anyhow!(e))?;
        ensure!(tag == T::message_tag());

        let payload = if bytes.len() > 5 {
            T::from_bytes(&bytes[5..(5 + len) as usize])?
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

/////////////////////////////////////////////////////////////////////////////
//                                  Tests
/////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod peer_messages_test {
    use super::*;

    const CHOKE: PeerMessage<ChokePayload> = PeerMessage::choke();
    const UNCHOKE: PeerMessage<UnchokePayload> = PeerMessage::unchoke();
    const INTERESTED: PeerMessage<InterestedPayload> = PeerMessage::interested();
    const NOT_INTERESTED: PeerMessage<NotInterestedPayload> = PeerMessage::not_interested();
    const HAVE: PeerMessage<HavePayload> = PeerMessage::have(0);
    const BITFIELD: PeerMessage<BitfieldPayload> = PeerMessage::bitfield(&[1, 2, 3, 4, 5, 6, 7, 8]);
    const REQUEST: PeerMessage<RequestPayload> = PeerMessage::request(1, 2, 3);

    #[test]
    fn check_lens() {
        assert_eq!(CHOKE.len, 1);
        assert_eq!(UNCHOKE.len, 1);
        assert_eq!(INTERESTED.len, 1);
        assert_eq!(NOT_INTERESTED.len, 1);
        assert_eq!(HAVE.len, 5);
        assert_eq!(BITFIELD.len, 9);
        assert_eq!(REQUEST.len, 13);
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
    }

    #[test]
    fn check_payload_as_bytes() {
        assert_eq!(
            REQUEST.payload.as_bytes(),
            [0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3]
        )
    }
}
