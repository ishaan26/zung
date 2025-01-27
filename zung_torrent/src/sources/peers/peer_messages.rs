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
    /// Creates a `PeerMessage` representing the `choke` message in the Peer Wire Protocol.
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

    /// Creates a `PeerMessage` representing the `unchoke` message in the Peer Wire Protocol.
    ///
    /// This message, as the name suggests, opens the chocked connection.
    pub const fn unchoke() -> PeerMessage<UnchokePayload> {
        PeerMessage {
            len: (size_of::<UnchokePayload>() + 1) as u32,
            tag: PeerMessagesTag::Unchoke,
            payload: UnchokePayload,
        }
    }

    /// Creates a `PeerMessage` representing the `interested` message in the Peer Wire Protocol.
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

    /// Creates a `PeerMessage` representing the `not interested` message in the Peer Wire Protocol.
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

    /// Creates a `PeerMessage` representing the `have` message in the Peer Wire Protocol.
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
    pub const fn size(&self) -> usize {
        4 /*len bytes*/ +  1 /* tag byte*/  + size_of::<T>()
    }

    pub fn as_bytes(&self) -> Bytes {
        let payload_bytes = self.payload.as_bytes();

        let mut bytes = BytesMut::with_capacity(self.size());

        bytes.put_u32(self.len);
        bytes.put_u8(self.tag as u8);
        bytes.put_slice(payload_bytes);

        bytes.freeze()
    }
}

/////////////////////////////////////////////////////////////////////////////
//                                  Tag
/////////////////////////////////////////////////////////////////////////////

/// Tags of the torrent peer wire protocol
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
enum PeerMessagesTag {
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

    // request: <len=0013><id=6><index><begin><length>
    //
    // The request message is fixed length, and is used to request a block. The payload contains
    // the following information:
    //
    //  index: integer specifying the zero-based piece index
    //  begin: integer specifying the zero-based byte offset within the piece
    //  length: integer specifying the requested length.
    Request = 6,
}

/////////////////////////////////////////////////////////////////////////////
//                                  Payload
/////////////////////////////////////////////////////////////////////////////

/// A common trait of all [`PeerMessage`] payloads.
pub trait PeerMessagePayload
where
    Self: Sized,
{
    fn as_bytes(&self) -> &[u8];

    fn size(&self) -> usize {
        std::mem::size_of::<Self>()
    }

    fn is_empty_payload(&self) -> bool {
        self.size() == 0
    }
}

/// Payload of the [`PeerMessage::choke`] message.
#[repr(C)]
pub struct ChokePayload;

impl PeerMessagePayload for ChokePayload {
    fn as_bytes(&self) -> &[u8] {
        &[]
    }
}

/// Payload of the [`PeerMessage::unchoke`] message.
#[repr(C)]
pub struct UnchokePayload;

impl PeerMessagePayload for UnchokePayload {
    fn as_bytes(&self) -> &[u8] {
        &[]
    }
}

/// Payload of the [`PeerMessage::interested`] message.
#[repr(C)]
pub struct InterestedPayload;

impl PeerMessagePayload for InterestedPayload {
    fn as_bytes(&self) -> &[u8] {
        &[]
    }
}

/// Payload of the [`PeerMessage::not_interested`] message.
#[repr(C)]
pub struct NotInterestedPayload;

impl PeerMessagePayload for NotInterestedPayload {
    fn as_bytes(&self) -> &[u8] {
        &[]
    }
}

/// Payload of the [`PeerMessage::have`] message.
///
///The 'have' message's payload is a single number, the index which that downloader just completed
///and checked the hash of.
#[repr(C)]
pub struct HavePayload {
    // the zero-based index of a piece that has just been successfully downloaded and verified via
    // the hash.
    index: [u8; 4],
}

impl PeerMessagePayload for HavePayload {
    fn as_bytes(&self) -> &[u8] {
        self.index.as_ref()
    }
}

/// Payload of the [`PeerMessage::request`] message.
#[repr(C)]
#[derive(Clone, Copy)]
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
}

#[cfg(test)]
mod peer_messages_test {

    use super::PeerMessage;

    #[test]
    fn check_sizes() {
        let choke = PeerMessage::choke();
        assert_eq!(choke.len, 1);

        let unchoke = PeerMessage::unchoke();
        assert_eq!(unchoke.len, 1);

        let interested = PeerMessage::interested();
        assert_eq!(interested.len, 1);

        let not_interested = PeerMessage::not_interested();
        assert_eq!(not_interested.len, 1);

        let request = PeerMessage::request(234, 234, 243);
        assert_eq!(request.len, 13);

        dbg!(choke.size());
        dbg!(request.size());
    }
}
