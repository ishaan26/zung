use std::sync::atomic::{AtomicBool, Ordering};

use rayon::{
    iter::{IndexedParallelIterator, ParallelIterator},
    slice::ParallelSlice,
};
use serde::{de::Visitor, Deserialize, Serialize};

/////////////////////////////////////////////////////////////////////////////
//                               Single Piece
/////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct Piece {
    downloaded: AtomicBool,
    index: usize,
    hash: [u8; 20],
}

impl PartialEq for Piece {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.hash == other.hash
    }
}

impl Piece {
    pub fn index(&self) -> usize {
        self.index
    }

    pub fn is_downloaded(&self) -> bool {
        self.downloaded.load(Ordering::Relaxed)
    }

    pub fn set_downloaded(&self) {
        self.downloaded.store(true, Ordering::Relaxed)
    }

    pub fn hash(&self) -> [u8; 20] {
        self.hash
    }
}

/////////////////////////////////////////////////////////////////////////////
//                               List of Pieces
/////////////////////////////////////////////////////////////////////////////

/// This is a string consisting of the concatenation of all 20-byte sha1 hash values, one per piece
/// (byte string, i.e. not urlencoded)
#[derive(Debug)]
pub struct PiecesList {
    list: Vec<Piece>,
}

impl PiecesList {
    pub fn len(&self) -> usize {
        self.list.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn iter(&self) -> PiecesListIter {
        PiecesListIter::new(self)
    }
}

pub struct PiecesListIter<'a> {
    iter: std::slice::Iter<'a, Piece>,
}

impl<'a> PiecesListIter<'a> {
    fn new(list: &'a PiecesList) -> Self {
        Self {
            iter: list.list.iter(),
        }
    }
}

impl<'a> Iterator for PiecesListIter<'a> {
    type Item = &'a Piece;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<'a> IntoIterator for &'a PiecesList {
    type Item = &'a Piece;

    type IntoIter = PiecesListIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

struct PiecesListVisitor;

impl Visitor<'_> for PiecesListVisitor {
    type Value = PiecesList;

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
            .enumerate()
            .map(|(i, c)| Piece {
                downloaded: AtomicBool::new(false),
                index: i,
                hash: c
                    .try_into()
                    .expect("Unable to divide pieces into 20 byte chunks"),
            })
            .collect_into_vec(&mut chunks);

        Ok(PiecesList { list: chunks })
    }
}

impl Serialize for PiecesList {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(&self.list.iter().flat_map(|p| p.hash).collect::<Vec<_>>())
    }
}

impl<'de> Deserialize<'de> for PiecesList {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_bytes(PiecesListVisitor)
    }
}

///////////////////////////////
// For testing only
///////////////////////////////

impl PiecesList {
    pub(super) fn __test_build() -> Self {
        let bytes = [[1; 20], [2; 20], [3; 20]].as_flattened();

        #[derive(Debug)]
        struct PieceError {}

        impl std::fmt::Display for PieceError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("Piece Error")
            }
        }

        impl std::error::Error for PieceError {}

        impl serde::de::Error for PieceError {
            fn custom<T>(_msg: T) -> Self
            where
                T: std::fmt::Display,
            {
                PieceError {}
            }
        }

        let visitor = PiecesListVisitor;
        let parsed = visitor.visit_bytes::<PieceError>(bytes).unwrap();

        parsed
    }
}

#[cfg(test)]
mod pieces_tests {
    use super::*;

    #[test]
    fn test_formation() {
        let pieces = PiecesList::__test_build();
        let mut iter = pieces.iter();

        assert_eq!(
            iter.next().unwrap(),
            &Piece {
                downloaded: AtomicBool::new(false),
                index: 0,
                hash: [1; 20]
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            &Piece {
                downloaded: AtomicBool::new(false),
                index: 1,
                hash: [2; 20]
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            &Piece {
                downloaded: AtomicBool::new(false),
                index: 2,
                hash: [3; 20]
            }
        );
        assert_eq!(iter.next(), None);
    }
}

// #[cfg(test)]
// mod pieces_tests {
//     use super::*;
//     use zung_parsers::bencode;
//
//     const TEST_BYTES: [[u8; 20]; 3] = [[1; 20], [2; 20], [3; 20]];
//     const SERIALIZED_BYTES: &[u8; 63] =  b"60:\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x02\x03\x03\x03\x03\x03\x03\x03\x03\x03\x03\x03\x03\x03\x03\x03\x03\x03\x03\x03\x03";
//
//     #[test]
//     fn test_pieces_serialization() {
//         let pieces = PiecesList::__test_build();
//         let serialized = bencode::to_bytes(&pieces).unwrap();
//         assert_eq!(serialized, SERIALIZED_BYTES);
//     }
//
//     #[test]
//     fn test_pieces_deserialization() {
//         let pieces: PiecesList = bencode::from_bytes(SERIALIZED_BYTES).unwrap();
//         assert_eq!(pieces.list, vec![[1; 20], [2; 20], [3; 20]]);
//     }
//
//     #[test]
//     fn test_pieces_roundtrip() {
//         let original = PiecesList {
//             list: vec![[1; 20], [2; 20], [3; 20], [4; 20]],
//         };
//         let serialized = bencode::to_bytes(&original).unwrap();
//         let deserialized: PiecesList = bencode::from_bytes(&serialized).unwrap();
//         assert_eq!(original.list, deserialized.list);
//     }
//
//     #[test]
//     fn test_pieces_invalid_length() {
//         let input = b"61:\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x02";
//         let result: Result<PiecesList, _> = bencode::from_bytes(input);
//         assert!(result.is_err());
//     }
//
//     #[test]
//     fn test_pieces_empty() {
//         let pieces = PiecesList { list: vec![] };
//         let serialized = bencode::to_bytes(&pieces).unwrap();
//         assert_eq!(serialized, b"0:");
//         let deserialized: PiecesList = bencode::from_bytes(&serialized).unwrap();
//         assert!(deserialized.is_empty())
//     }
//
//     #[test]
//     fn test_pieces_deref() {
//         let pieces = PiecesList {
//             list: vec![[1; 20], [2; 20]],
//         };
//         assert_eq!(pieces.len(), 2);
//         assert_eq!(pieces[0], [1; 20]);
//         assert_eq!(pieces[1], [2; 20]);
//     }
// }
