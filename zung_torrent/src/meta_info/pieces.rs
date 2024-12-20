use std::ops::Deref;

use rayon::{
    iter::{IndexedParallelIterator, ParallelIterator},
    slice::ParallelSlice,
};
use serde::{de::Visitor, Deserialize, Serialize};

/// This is a string consisting of the concatenation of all 20-byte sha1 hash values, one per piece
/// (byte string, i.e. not urlencoded)
#[derive(Debug)]
pub struct Pieces {
    bytes: Vec<[u8; 20]>,
}

struct PiecesVisitor;

impl Visitor<'_> for PiecesVisitor {
    type Value = Pieces;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            formatter,
            "Metainfo pieces - A byte encoded string of 20byte sha1 hash values"
        )
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if v.len() % 20 != 0 {
            return Err(E::custom(
                "Invalid Torrent File - Pieces should be in 20 byte chunks always",
            ));
        }
        let len = v.len() / 20;
        let mut chunks = Vec::with_capacity(len);

        v.par_chunks_exact(20)
            .map(|c| {
                c.try_into()
                    .expect("Unable to divide pieces into 20 byte chunks")
            })
            .collect_into_vec(&mut chunks);

        Ok(Pieces { bytes: chunks })
    }
}

impl Serialize for Pieces {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(self.bytes.as_flattened())
    }
}

impl<'de> Deserialize<'de> for Pieces {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_bytes(PiecesVisitor)
    }
}

impl Deref for Pieces {
    type Target = Vec<[u8; 20]>;

    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

impl Pieces {
    pub(crate) fn __test_build() -> Self {
        Self {
            bytes: [[1; 20], [2; 20], [3; 20]].to_vec(),
        }
    }
}

#[cfg(test)]
mod pieces_tests {
    use super::*;
    use zung_parsers::bencode;

    const TEST_BYTES: [[u8; 20]; 3] = [[1; 20], [2; 20], [3; 20]];
    const SERIALIZED_BYTES: &[u8; 63] =  b"60:\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x03\x03\x03\x03\x03\x03\x03\x03\x03\x03\x03\x03\x03\x03\x03\x03\x03\x03\x03\x03";

    #[test]
    fn test_pieces_serialization() {
        let pieces = Pieces {
            bytes: TEST_BYTES.to_vec(),
        };
        let serialized = bencode::to_bytes(&pieces).unwrap();
        assert_eq!(serialized, SERIALIZED_BYTES);
    }

    #[test]
    fn test_pieces_deserialization() {
        let pieces: Pieces = bencode::from_bytes(SERIALIZED_BYTES).unwrap();
        assert_eq!(pieces.bytes, vec![[1; 20], [2; 20], [3; 20]]);
    }

    #[test]
    fn test_pieces_roundtrip() {
        let original = Pieces {
            bytes: vec![[1; 20], [2; 20], [3; 20], [4; 20]],
        };
        let serialized = bencode::to_bytes(&original).unwrap();
        let deserialized: Pieces = bencode::from_bytes(&serialized).unwrap();
        assert_eq!(original.bytes, deserialized.bytes);
    }

    #[test]
    fn test_pieces_invalid_length() {
        let input = b"61:\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x02";
        let result: Result<Pieces, _> = bencode::from_bytes(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_pieces_empty() {
        let pieces = Pieces { bytes: vec![] };
        let serialized = bencode::to_bytes(&pieces).unwrap();
        assert_eq!(serialized, b"0:");
        let deserialized: Pieces = bencode::from_bytes(&serialized).unwrap();
        assert!(deserialized.is_empty())
    }

    #[test]
    fn test_pieces_deref() {
        let pieces = Pieces {
            bytes: vec![[1; 20], [2; 20]],
        };
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0], [1; 20]);
        assert_eq!(pieces[1], [2; 20]);
    }
}
