use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use super::{
    files::{FileNode, Files},
    pieces::Pieces,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Info {
    // number of bytes in each piece (integer).
    //
    // The piece length specifies the nominal piece size, and is usually a power of 2. The piece
    // size is typically chosen based on the total amount of file data in the torrent, and is
    // constrained by the fact that too-large piece sizes cause inefficiency, and too-small piece
    // sizes cause large .torrent metadata file. Historically, piece size was chosen to result in a
    // .torrent file no greater than approx. 50 - 75 kB (presumably to ease the load on the server
    // hosting the torrent files).
    //
    // Current best-practice is to keep the piece size to 512KB or less, for torrents around
    // 8-10GB, even if that results in a larger .torrent file. This results in a more efficient
    // swarm for sharing files. The most common sizes are 256 kB, 512 kB, and 1 MB. Every piece is
    // of equal length except for the final piece, which is irregular. The number of pieces is thus
    // determined by 'ceil( total length / piece size )'.
    //
    // For the purposes of piece boundaries in the multi-file case, consider the file data as
    // one long continuous stream, composed of the concatenation of each file in the order
    // listed in the files list. The number of pieces and their boundaries are then determined
    // in the same manner as the case of a single file. Pieces may overlap file boundaries.
    //
    // Each piece has a corresponding SHA1 hash of the data contained within that piece. These
    // hashes are concatenated to form the pieces value in the above info dictionary. Note that
    // this is not a list but rather a single string. The length of the string must be a multiple
    // of 20.
    #[serde(rename = "piece length")]
    pub(crate) piece_length: usize,

    // string consisting of the concatenation of all 20-byte SHA1 hash values, one per piece (byte
    // string, i.e. not urlencoded)
    pub(crate) pieces: Pieces,

    // (optional) this field is an integer. If it is set to "1", the client MUST publish its
    // presence to get other peers ONLY via the trackers explicitly described in the metainfo file.
    // If this field is set to "0" or is not present, the client may obtain peer from other means,
    // e.g. PEX peer exchange, dht. Here, "private" may be read as "no external peer source".
    //
    // NOTE:
    // There is much debate surrounding private trackers. The official request for a specification
    // change is here. Azureus was the first client to respect private trackers, see their wiki for
    // more details.
    pub(crate) private: Option<u8>,

    // A torrent can be a `Single-File` or a 'MultiFile'. This key reprasents that state
    #[serde(flatten)]
    pub(crate) files: Files,

    // In the single file state this is the filename. In the multifile state this is the the name
    // of the directory in which to store all the files. This is purely advisory. (string)
    pub(crate) name: Option<String>,
}

impl<'a> Info {
    pub(crate) fn torrent_size(&self) -> usize {
        let n_pieces = self.pieces.len();
        let plen = self.piece_length;
        n_pieces * plen
    }

    pub(crate) fn build_file_tree(&'a self) -> FileNode<'a> {
        match &self.files {
            Files::SingleFile { length, .. } => {
                if let Some(name) = &self.name {
                    FileNode::File {
                        name: Cow::from(name),
                        length: *length,
                    }
                } else {
                    FileNode::File {
                        name: Cow::from("__No Name__"),
                        length: *length,
                    }
                }
            }
            Files::MultiFile { files } => {
                if let Some(name) = &self.name {
                    let mut root = FileNode::new_dir(name);
                    for file in files {
                        let path = &file.path;
                        let file_path = &path[..path.len()];
                        root.add_child(file_path, file.length);
                    }
                    root
                } else {
                    panic!("The torrent has no root folder")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta_info::files::{Files, MultiFiles};
    use crate::meta_info::pieces::Pieces;

    #[test]
    fn test_torrent_size() {
        // Setup: Creating an instance of `Info` with mocked piece length and pieces.
        let piece_length = 1024; // each piece is 1024 bytes
        let pieces = Pieces::__test_build();

        let info = Info {
            piece_length,
            pieces,
            private: None,
            files: Files::SingleFile {
                length: 4096,
                md5sum: None,
            },
            name: Some("test_file.txt".to_string()),
        };

        // We expect 4 pieces, each of size 1024 bytes
        assert_eq!(info.torrent_size(), 3 * 1024);
    }

    #[test]
    fn test_build_file_tree_single_file() {
        // Setup: Creating a single-file torrent info
        let info = Info {
            piece_length: 1024,
            pieces: Pieces::__test_build(),
            private: None,
            files: Files::SingleFile {
                length: 4096,
                md5sum: None,
            },
            name: Some("test_file.txt".to_string()),
        };

        let file_tree = info.build_file_tree();

        // Check if the file tree is built correctly for a single file
        match file_tree {
            FileNode::File { name, length } => {
                assert_eq!(name, Cow::from("test_file.txt"));
                assert_eq!(length, 4096);
            }
            _ => panic!("Expected a file node"),
        }
    }

    #[test]
    fn test_build_file_tree_multi_file() {
        // Setup: Creating a multi-file torrent info
        let files = vec![
            MultiFiles {
                length: 1024,
                md5sum: None,
                path: vec!["folder".to_string(), "file1.txt".to_string()],
            },
            MultiFiles {
                length: 2048,
                md5sum: None,
                path: vec!["folder".to_string(), "file2.txt".to_string()],
            },
        ];

        let info = Info {
            piece_length: 1024,
            pieces: Pieces::__test_build(), // Mocked 4 pieces
            private: None,
            files: Files::MultiFile { files },
            name: Some("root_folder".to_string()),
        };

        let file_tree = info.build_file_tree();

        // Check if the file tree is built correctly for multi-file torrents
        match file_tree {
            FileNode::Dir {
                parent, children, ..
            } => {
                assert_eq!(parent, Cow::from("root_folder"));
                assert!(children.contains_key("folder"));

                match children.get("folder").unwrap() {
                    FileNode::Dir { children, .. } => {
                        assert_eq!(children.len(), 2);
                        let file1 = children.get("file1.txt").expect("File1 not found");
                        match file1 {
                            FileNode::File { name, length } => {
                                assert_eq!(name, "file1.txt");
                                assert_eq!(*length, 1024);
                            }
                            _ => panic!("Expected a file node"),
                        }

                        let file2 = children.get("file2.txt").expect("File2 not found");
                        match file2 {
                            FileNode::File { name, length } => {
                                assert_eq!(name, "file2.txt");
                                assert_eq!(*length, 2048);
                            }
                            _ => panic!("Expected a file node"),
                        }
                    }
                    _ => panic!("Expected a directory node for 'folder'"),
                }
            }
            _ => panic!("Expected a directory node for 'root_folder'"),
        }
    }

    #[test]
    #[should_panic(expected = "The torrent has no root folder")]
    fn test_build_file_tree_no_root_folder() {
        // Setup: Creating a multi-file torrent info without a root folder (should panic)
        let files = vec![MultiFiles {
            length: 1024,
            md5sum: None,
            path: vec!["folder".to_string(), "file1.txt".to_string()],
        }];

        let info = Info {
            piece_length: 1024,
            pieces: Pieces::__test_build(), // Mocked 4 pieces
            private: None,
            files: Files::MultiFile { files },
            name: None, // No root folder
        };

        info.build_file_tree(); // This should panic because the root folder is missing
    }
}
