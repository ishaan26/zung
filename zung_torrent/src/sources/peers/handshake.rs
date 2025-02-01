use crate::{client::PEER_ID, meta_info::InfoHashEncoded};

/// Represents the BitTorrent handshake message.
///
/// The handshake is the first message transmitted by a client to a peer in the BitTorrent
/// protocol.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Handshake {
    // string length of <pstr>, as a single raw byte
    pstrlen: u8,

    // string identifier of the protocol
    pstr: [u8; 19],

    // eight (8) reserved bytes. All current implementations use all zeroes. Each bit in these
    // bytes can be used to change the behavior of the protocol. An email from Bram suggests that
    // trailing bits should be used first, so that leading bits may be used to change the meaning
    // of trailing bits.
    reserved: [u8; 8],

    // 20-byte SHA1 hash of the info key in the metainfo file. This is the same info_hash that is
    // transmitted in tracker requests.
    info_hash: [u8; 20],

    // 20-byte string used as a unique ID for the client. This is usually the same peer_id that is
    // transmitted in tracker requests (but not always e.g. an anonymity option in Azureus).
    peer_id: [u8; 20],
}

impl Handshake {
    /// The v1 protocol identifier for BitTorrent which is always "BitTorrent protocol".    
    pub const PROTOCOL_V1: [u8; 19] = *b"BitTorrent protocol";

    /// The total size (in bytes) of a handshake message which is 68.
    pub const SIZE: usize = size_of::<Self>();

    /// Constructs a new handshake with default protocol version and peer ID
    pub fn new(info_hash: InfoHashEncoded) -> Self {
        Self {
            pstrlen: 19,
            pstr: Self::PROTOCOL_V1,
            reserved: [0; 8],
            info_hash: info_hash.as_bytes(),
            peer_id: PEER_ID.as_bytes(),
        }
    }

    /// Returns the handshake message as an array of bytes.
    pub const fn as_bytes(&self) -> [u8; Self::SIZE] {
        let bytes = self as *const Self as *const [u8; Self::SIZE];

        // SAFETY:
        // - The struct is marked `#[repr(C)]`, ensuring a well-defined, packed layout without padding.
        // - All fields are contiguous arrays of `u8` (1-byte alignment), guaranteeing the struct's
        //   in-memory representation is exactly the concatenation of its fields' bytes.
        // - `size_of::<Handshake>()` matches the sum of all field sizes (1 + 19 + 8 + 20 + 20 = 68
        //   bytes),
        //   confirming there's no implicit padding.
        unsafe { *bytes }
    }

    /// Constructs a `Handshake` instance from an array of bytes. This intended to be used when
    /// reading the reply message of a initiated Handshake Message.
    pub const fn from_bytes(bytes: [u8; Self::SIZE]) -> Self {
        let handshake = &bytes as *const [u8; Self::SIZE] as *const Self;

        unsafe { *handshake }
    }

    /// Returns the protocol string length (`pstrlen`).
    pub const fn pstrlen(&self) -> u8 {
        self.pstrlen
    }

    /// Returns the protocol string (`pstr`).
    pub const fn pstr(&self) -> [u8; 19] {
        self.pstr
    }

    /// Returns the reserved bytes.
    pub const fn reserved(&self) -> [u8; 8] {
        self.reserved
    }

    /// Returns the info hash.
    pub const fn info_hash(&self) -> [u8; 20] {
        self.info_hash
    }

    /// Returns the peer identifier.
    pub const fn peer_id(&self) -> [u8; 20] {
        self.peer_id
    }
}
