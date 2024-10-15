use std::{borrow::Cow, collections::HashMap};

use human_bytes::human_bytes;
use serde::{Deserialize, Serialize};

use super::pieces::Pieces;

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

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Files {
    SingleFile {
        // length of the file in bytes (integer)
        length: usize,

        // (optional) a 32-character hexadecimal string corresponding to the MD5 sum of the file. This is not used by BitTorrent at all, but it is included by some programs for greater compatibility.
        md5sum: Option<String>,
    },
    MultiFile {
        // a list of dictionaries, one for each file. Each dictionary in this list contains the following keys:
        files: Vec<MultiFiles>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MultiFiles {
    // length of the file in bytes (integer)
    pub(crate) length: usize,

    // (optional) a 32-character hexadecimal string corresponding to the MD5 sum of the file. This
    // is not used by BitTorrent at all, but it is included by some programs for greater
    // compatibility.
    pub(crate) md5sum: Option<String>,

    // a list containing one or more string elements that together represent the path and filename.
    // Each element in the list corresponds to either a directory name or (in the case of the final
    // element) the filename. For example, a the file "dir1/dir2/file.ext" would consist of three
    // string elements: "dir1", "dir2", and "file.ext". This is encoded as a bencoded list of
    // strings such as l4:dir14:dir28:file.exte
    pub(crate) path: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum FileNode<'a> {
    Dir {
        parent: Cow<'a, str>,
        children: HashMap<String, FileNode<'a>>,
        length: usize,
    },
    File {
        name: Cow<'a, str>,
        length: usize,
    },
}

impl<'a> FileNode<'a> {
    fn new_dir(name: &'a str) -> Self {
        FileNode::Dir {
            parent: Cow::from(name),
            children: HashMap::new(),
            length: 0,
        }
    }

    fn new_file(name: &'a str, length: usize) -> Self {
        FileNode::File {
            name: Cow::from(name),
            length,
        }
    }

    fn add_child(&mut self, path: &'a [String], size: usize) {
        if path.is_empty() {
            return;
        }

        match self {
            FileNode::Dir {
                children, length, ..
            } => {
                if path.is_empty() {
                    return;
                }

                let current = &path[0];
                let child = children
                    .entry(current.clone())
                    .or_insert_with(|| FileNode::new_dir(current));

                if path.len() > 1 {
                    *length += size;
                    child.add_child(&path[1..], size);
                } else {
                    *child = FileNode::new_file(current, size);
                    *length += size;
                }
            }
            FileNode::File { .. } => {
                // If we're trying to add a path to a file, something's wrong
                panic!("Attempting to add a path to a file node");
            }
        }
    }
}

impl<'a> Info {
    pub fn torrent_size(&self) -> usize {
        let n_pieces = self.pieces.len();
        let plen = self.piece_length;
        n_pieces * plen
    }

    pub fn build_file_tree(&'a self) -> FileNode<'a> {
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

impl<'a> FileNode<'a> {
    #[cfg(feature = "client")]
    pub fn print_tree(&self, mut indent: usize) {
        use colored::Colorize;

        match self {
            FileNode::Dir {
                parent,
                children,
                length,
            } => {
                if !parent.starts_with(".___") {
                    println!();
                    println!(
                        "{:indent$} - {} ({})",
                        "",
                        parent.bold().underline().green(),
                        human_bytes(*length as f64),
                        indent = indent,
                    );

                    indent += 4;

                    for child in children.values() {
                        child.print_tree(indent);
                    }
                }
            }
            FileNode::File { name, length } => {
                println!(
                    "{:indent$} - {} ({})",
                    "",
                    name.bold(),
                    human_bytes(*length as f64).cyan(),
                    indent = indent
                );
            }
        }
    }
}
