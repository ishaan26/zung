use std::{borrow::Cow, collections::BTreeMap};

use human_bytes::human_bytes;
use serde::{Deserialize, Serialize};

const PADDING_FILE_IDENTIFIER: &str = ".___";

/// Reprasents the the single file or multi file state of the torrent file.
///
/// The both states are reprasented as follows in the enum:
///
/// - `SingleFile`: Represents a single file with its length in bytes and an optional MD5 checksum.
/// - `MultiFile`: Represents multiple files with a vector of [`MultiFiles`] structs.
///
/// As per the [The BitTorrent Protocol
/// Specification](https://www.bittorrent.org/beps/bep_0003.html), in a torrent files there is
/// either a key `length` or a key `files`, but not both or neither. If `length` is present then
/// the download represents a single file, otherwise there is a `files` key which represents a set
/// of files which go in a directory structure.
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

/// Reprasents the multifile state of the torrent.
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
pub struct FileTree<'a> {
    pub(crate) node: FileNode<'a>,
    pub(crate) n: usize,
}

impl<'a> FileTree<'a> {
    pub fn print_file_tree(&self) {
        self.node.print_tree(4);
    }

    pub fn number_of_files(&self) -> usize {
        self.n
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub(crate) enum FileNode<'a> {
    Dir {
        parent: Cow<'a, str>,
        children: BTreeMap<String, FileNode<'a>>,
        length: usize,
    },
    File {
        name: Cow<'a, str>,
        length: usize,
    },
}

/// Type for building a in-memory file structure from a torrent file.
///
/// This is build with the [`build_file_tree`](super::Info::build_file_tree) method.
impl<'a> FileNode<'a> {
    pub(crate) fn new_dir(name: &'a str) -> Self {
        FileNode::Dir {
            parent: Cow::from(name),
            children: BTreeMap::new(),
            length: 0,
        }
    }

    pub(crate) fn new_file(name: &'a str, length: usize) -> Self {
        FileNode::File {
            name: Cow::from(name),
            length,
        }
    }

    pub(crate) fn add_child(&mut self, path: &'a [String], size: usize) {
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

                let current = path.first().unwrap();
                if !current.starts_with(PADDING_FILE_IDENTIFIER) {
                    let child = children
                        .entry(current.clone())
                        .or_insert_with(|| FileNode::new_dir(current));

                    // Add sub directories recursively. The the last entry in the files list is hit,
                    // change FilesNode::Dir entry to FilesNode::Files
                    if path.len() > 1 {
                        *length += size;
                        child.add_child(&path[1..], size);
                    } else {
                        *child = FileNode::new_file(current, size);
                        *length += size;
                    }
                }
            }
            FileNode::File { .. } => {
                // If we're trying to add a path to a file, something's wrong
                panic!("Attempting to add a path to a file node");
            }
        }
    }

    /// Recursively prints the file tree in a human-readable format, using indentation.
    ///
    /// ## Arguments:
    ///
    /// `indent`: Indentation step to use for printing child data in the file structure hirarcy.
    #[cfg(feature = "client")]
    fn print_tree(&self, mut indent: usize) {
        use colored::Colorize;

        match self {
            FileNode::Dir {
                parent,
                children,
                length,
            } => {
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

#[cfg(test)]
mod files_tests {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn test_create_new_directory() {
        let dir_name = "root";
        let dir = FileNode::new_dir(dir_name);

        // Test if the directory is created successfully
        match dir {
            FileNode::Dir {
                parent,
                children,
                length,
            } => {
                assert_eq!(parent, Cow::from(dir_name));
                assert_eq!(children.len(), 0);
                assert_eq!(length, 0);
            }
            _ => panic!("Expected a directory node!"),
        }
    }

    #[test]
    fn test_create_new_file() {
        let file_name = "file.txt";
        let file_size = 1024;
        let file = FileNode::new_file(file_name, file_size);

        // Test if the file is created successfully
        match file {
            FileNode::File { name, length } => {
                assert_eq!(name, Cow::from(file_name));
                assert_eq!(length, file_size);
            }
            _ => panic!("Expected a file node!"),
        }
    }

    #[test]
    fn test_add_file_to_directory() {
        let mut root = FileNode::new_dir("root");

        let path = vec![String::from("file.txt")];
        let size = 512;

        // Add a file to the root directory
        root.add_child(&path, size);

        // Test if the file was added to the directory
        match root {
            FileNode::Dir {
                ref children,
                length,
                ..
            } => {
                assert_eq!(children.len(), 1);
                assert_eq!(length, size);

                let child = children
                    .get("file.txt")
                    .expect("File not found in directory!");
                match child {
                    FileNode::File { name, length } => {
                        assert_eq!(name, "file.txt");
                        assert_eq!(*length, size);
                    }
                    _ => panic!("Expected a file node!"),
                }
            }
            _ => panic!("Expected a directory node!"),
        }
    }

    #[test]
    #[should_panic(expected = "Attempting to add a path to a file node")]
    fn test_add_child_to_file_should_panic() {
        let mut file = FileNode::new_file("file.txt", 1024);
        let path = vec![String::from("new_file.txt")];
        file.add_child(&path, 512); // This should panic as we can't add children to a file node.
    }
}
