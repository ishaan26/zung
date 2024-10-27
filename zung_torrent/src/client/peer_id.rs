use std::{
    fmt::Display,
    time::{SystemTime, UNIX_EPOCH},
};

use rand::Rng;

const PEERID_SIZE: u8 = 20;

/// A 20-byte URL-encoded identifier used as a unique ID for a BitTorrent client, generated at
/// startup.
///
/// This struct represents the `peer_id` field in the BitTorrent protocol, which serves to uniquely
/// identify a client in a peer network. According to the protocol, the `peer_id` may contain any
/// binary data, though convention suggests that it should be unique for each client instance on a
/// machine.
///
/// # Components
///
/// - `start`: A 1-byte field, typically a dash (`-`), indicating the start of the ID.
/// - `uid`: A 2-byte field for a unique identifier for the client. Here it is set as `"ZG"`.
/// - `pid`: A 4-byte field representing the process ID (PID), used to distinguish instances on the
///    same machine.
/// - `time`: A 12-byte field capturing the system time, ensuring further uniqueness.
/// - `end`: A 1-byte field, typically a dash (`-`), marking the end of the ID.
///
/// Together, these fields sum up to exactly 20 bytes, conforming to the expected size of a
/// `peer_id` in the BitTorrent protocol.
///
/// # Example
///
/// ```
/// use zung_torrent::PeerID;
///
/// let peer_id = PeerID::new();
/// println!("{:?}", peer_id.as_bytes()); // Prints the 20-byte unique peer ID as bytes
/// ```
///
/// # Note
///
/// The generated `peer_id` should be unique per instance to avoid peer collisions on the network.
/// This type uses ["Azureus-style"](https://wiki.theory.org/BitTorrentSpecification#peer_id) of
/// encoding the peer ID to accomplish this.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PeerID {
    start: [u8; 1],
    uid: [u8; 2],
    pid: [u8; 4],
    time: [u8; 12],
    end: [u8; 1],
}

impl PeerID {
    /// Creates a new `PeerID` using the ["Azureus-style"](https://wiki.theory.org/BitTorrentSpecification#peer_id).
    pub fn new() -> Self {
        Self {
            start: *b"-",
            uid: *b"ZG",
            pid: get_pid_bytes(),
            time: get_system_time_bytes(),
            end: *b"-",
        }
    }

    /// Casts the `PeerID` as a 20-byte array.
    ///
    /// # Note
    ///
    /// This function uses an unsafe conversion from `PeerID` to a byte slice to avoid copying each
    /// field, leveraging Rust's `#[repr(C)]` to ensure consistent layout in memory.
    ///
    /// # Safety
    ///
    /// This function performs an unsafe conversion by interpreting the `PeerID` struct
    /// as a 20-byte array directly. This is safe under the following assumptions:
    ///
    /// - [`PeerID`] is marked with `#[repr(C)]`, which guarantees that its fields are laid out in
    ///   memory sequentially and in the declared order.
    /// - The fields within [`PeerID`] (`start`, `uid`, `pid`, `time`, and `end`) are carefully
    ///   sized and combined to exactly match 20 bytes, so the struct and the `[u8; 20]` array always
    ///   have the same memory size.
    /// - The logic ensures that the casting is always done into an array of 20 bytes.
    pub fn as_bytes(&self) -> [u8; PEERID_SIZE as usize] {
        let bytes = self as *const Self as *const [u8; PEERID_SIZE as usize];
        unsafe { *bytes }
    }

    /// Returns a hexadecimal string representation of the `PeerID`.
    ///
    /// This is useful when the `PeerID` needs to be viewed as a UTF-8 string
    /// for logging, debugging, or interfacing with systems that accept hexadecimal.
    ///
    /// # Example
    ///
    /// ```
    /// use zung_torrent::PeerID;
    ///
    /// let peer_id = PeerID::new();
    /// let peer_id_str = peer_id.to_hex_encode();
    /// println!("{}", peer_id_str); // Outputs a hexadecimal string like "2d5a475033313233..."
    /// ```
    pub fn to_hex_encode(&self) -> String {
        hex::encode(self.as_bytes())
    }

    /// Url-encodes the [`PeerID`] value for communication with a [`Tracker`](crate::trackers)
    pub fn to_url_encoded(&self) -> String {
        let bytes = self.as_bytes();
        let mut buff = String::with_capacity(60);
        for byte in bytes {
            buff.push('%');
            buff.push_str(&hex::encode([byte]));
        }
        buff
    }
}

impl Default for PeerID {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for PeerID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // using from_utf8_unchecked because the start, uid and the end fields are all hardcoded
        // and are ascii.
        unsafe {
            write!(
                f,
                "{}{}{}{}{}",
                std::str::from_utf8_unchecked(&self.start),
                std::str::from_utf8_unchecked(&self.uid),
                u32::from_be_bytes(self.pid),
                usize::from_be_bytes(self.time[..8].try_into().unwrap()),
                std::str::from_utf8_unchecked(&self.end),
            )
        }
    }
}

fn get_pid_bytes() -> [u8; 4] {
    std::process::id().to_be_bytes()
}

fn get_system_time_bytes() -> [u8; 12] {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    let millis = duration.as_millis().to_be_bytes();
    let mut rng = rand::thread_rng();
    let random: u32 = rng.gen();

    // Combine 8 bytes of millis with 4 bytes of randomness
    let mut result = [0u8; 12];
    result[..8].copy_from_slice(&millis[0..8]);
    result[8..].copy_from_slice(&random.to_be_bytes());
    result
}

#[cfg(test)]
mod peer_id_tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_peer_id_new() {
        let peer_id = PeerID::new();

        // Test start and end markers
        assert_eq!(&peer_id.start, b"-");
        assert_eq!(&peer_id.end, b"-");

        // Test client identifier
        assert_eq!(&peer_id.uid, b"ZG");

        // Test PID bytes match current process
        let expected_pid = std::process::id().to_be_bytes();
        assert_eq!(&peer_id.pid, &expected_pid);
    }

    #[test]
    fn test_peer_id_uniqueness() {
        // Create multiple peer IDs and ensure they're different
        let peer_id1 = PeerID::new();
        thread::sleep(Duration::from_millis(100)); // Ensure different timestamp
        let peer_id2 = PeerID::new();

        assert_ne!(peer_id1.time, peer_id2.time);
    }

    #[test]
    fn ensure_length() {
        let peer_id = PeerID::new();
        let bytes = peer_id.as_bytes();

        // Test total length
        assert_eq!(
            bytes.len(),
            PEERID_SIZE as usize,
            "peer id len must be == {PEERID_SIZE}"
        );
    }

    #[test]
    fn test_peer_id_as_bytes() {
        let peer_id = PeerID::new();
        let bytes = peer_id.as_bytes();

        // Test total length
        assert_eq!(bytes.len(), 20);

        // Test start and end markers in byte array
        assert_eq!(bytes[0], b'-');
        assert_eq!(bytes[19], b'-');

        // Test client identifier in byte array
        assert_eq!(&bytes[1..3], b"ZG");
    }

    #[test]
    fn test_system_time_bytes() {
        let bytes1 = get_system_time_bytes();
        thread::sleep(Duration::from_millis(1));
        let bytes2 = get_system_time_bytes();

        // Test length
        assert_eq!(bytes1.len(), 12);

        // Test that different calls produce different values
        assert_ne!(bytes1, bytes2);
    }

    #[test]
    fn test_pid_bytes() {
        let pid_bytes = get_pid_bytes();
        let current_pid = std::process::id();

        // Test length
        assert_eq!(pid_bytes.len(), 4);

        // Test value matches current process ID
        assert_eq!(pid_bytes, current_pid.to_be_bytes());
    }

    #[test]
    fn test_default_implementation() {
        let default_peer_id = PeerID::default();
        let new_peer_id = PeerID::new();

        // Test that default and new have same structure
        assert_eq!(default_peer_id.start, new_peer_id.start);
        assert_eq!(default_peer_id.uid, new_peer_id.uid);
        assert_eq!(default_peer_id.pid, new_peer_id.pid);
        assert_eq!(default_peer_id.end, new_peer_id.end);
    }

    #[test]
    fn test_peer_id_serialization() {
        let peer_id = PeerID::new();
        let bytes = peer_id.as_bytes();

        // Verify structure preservation
        assert_eq!(&bytes[0..1], &peer_id.start);
        assert_eq!(&bytes[1..3], &peer_id.uid);
        assert_eq!(&bytes[3..7], &peer_id.pid);
        assert_eq!(&bytes[7..19], &peer_id.time);
        assert_eq!(&bytes[19..20], &peer_id.end);
    }

    #[test]
    fn test_time_bytes_format() {
        let bytes = get_system_time_bytes();

        // Create a new duration from the bytes
        let secs_bytes = &bytes[..6];
        let nanos_bytes = &bytes[6..];

        // Verify we can reconstruct a valid timestamp
        let mut secs_arr = [0u8; 8];
        secs_arr[..6].copy_from_slice(secs_bytes);

        let mut nanos_arr = [0u8; 4];
        nanos_arr[..3].copy_from_slice(&nanos_bytes[..3]);

        // These shouldn't panic if the bytes are valid
        let _secs = u64::from_le_bytes(secs_arr);
        let _nanos = u32::from_le_bytes(nanos_arr);
    }

    #[test]
    fn test_to_url_encoded() {
        // Initialize a PeerID with a specific set of values to ensure consistency in the test.
        let peer_id = PeerID {
            start: *b"-",
            uid: *b"ZG",
            pid: [49, 50, 51, 52], // '1234' in ASCII
            time: [53, 54, 55, 56, 57, 65, 66, 67, 68, 69, 70, 71], // '56789ABCDEFG' in ASCII
            end: *b"-",
        };

        // Expected URL-encoded representation
        let expected = "%2d%5a%47%31%32%33%34%35%36%37%38%39%41%42%43%44%45%46%47%2d";

        // Run the `to_url_encoded` method
        let result = peer_id.to_url_encoded();

        // Assert that the result matches the expected string
        assert_eq!(result, expected);
    }
}
