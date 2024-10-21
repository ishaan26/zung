use std::{borrow::Cow, fmt::Display};

use human_bytes::human_bytes;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

const PADDING_ATTR: &str = "p";
const SYMLINK_ATTR: &str = "l";
const EXECUTABLE_ATTR: &str = "x";
const HIDDEN_ATTR: &str = "h";

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

        // (optional) a 32-character hexadecimal string corresponding to the MD5 sum of the file.
        // This is not used by BitTorrent at all, but it is included by some programs for greater
        // compatibility.
        md5sum: Option<String>,

        // A variable-length string. When present the characters each represent a file attribute. l
        // = symlink, x = executable, h = hidden, p = padding file. Characters appear in no
        // particular order and unknown characters should be ignored.
        attr: Option<FileAttr>,
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

    // A variable-length string. When present the characters each represent a file attribute. l
    // = symlink, x = executable, h = hidden, p = padding file. Characters appear in no
    // particular order and unknown characters should be ignored.
    pub(crate) attr: Option<FileAttr>,
}

/// Reprasents the various values of a attr field within files of the torrent.
///
/// From [BEP 47](https://www.bittorrent.org/beps/bep_0047.html)
#[derive(Debug)]
pub enum FileAttr {
    /// Padding files are synthetic files inserted into the file list to let the following file
    /// start at a piece boundary. That means their length should fill up the remainder of the
    /// piece length of the file that is supposed to be padded. For the calculation of piece hashes
    /// the content of padding file is all zeros.
    ///
    /// Clients aware of this extension don't need to write the padding files to disk and should
    /// also avoid requesting byte-ranges covering their contents, e.g. via request messages. But
    /// for backwards-compatibility they must service such requests.
    ///
    /// While clients implementing this extensions will have no use for the path of a padding file
    /// it should be included for backwards compatibility since it is a mandatory field in BEP 3
    /// [1]. The recommended path is [".pad", "N"] where N is the length of the padding file in
    /// base10. This way clients not aware of this extension will write the padding files into a
    /// single directory, potentially re-using padding files from other torrents also stored in
    /// that directory.
    ///
    /// To eventually allow the path field to be omitted clients implementing this BEP should not
    /// require it to be present on padding files.
    ///
    /// Piece-aligned files simplify deduplication [2] and the operations on mutable torrents [3].
    ///
    /// The presence of padding files does not imply that all files are piece-aligned.
    Padding,

    /// When the l attribute flag is present then the symlink path represents the target of the symlink
    /// while path indicates the location of the symlink itself.
    ///
    /// The length of a symlink is always zero since it just points to another file already present
    /// in the piece space. For backwards compatibility the length=0 should be included when
    /// creating a torrent but clients implementing with this BEP should not require their presence
    /// on symlink files when parsing it so it can be omitted at some point in the future.
    ///
    /// Just like the regular path the symlink path is relative to the torrent root and must not
    /// contain .. elements. It should also target another file within the torrent, otherwise a
    /// dangling symlink will be created.
    Symlink,

    Executable,

    Hidden,

    Other(String),
}

impl Display for FileAttr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileAttr::Padding => write!(f, "{PADDING_ATTR}"),
            FileAttr::Symlink => write!(f, "{SYMLINK_ATTR}"),
            FileAttr::Executable => write!(f, "{EXECUTABLE_ATTR}"),
            FileAttr::Hidden => write!(f, "{HIDDEN_ATTR}"),
            FileAttr::Other(s) => write!(f, "{s}"),
        }
    }
}

impl Serialize for FileAttr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for FileAttr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            PADDING_ATTR => Ok(FileAttr::Padding),
            SYMLINK_ATTR => Ok(FileAttr::Symlink),
            EXECUTABLE_ATTR => Ok(FileAttr::Executable),
            HIDDEN_ATTR => Ok(FileAttr::Hidden),
            // NOTE: Rejecting a torrent because of unsupported file attr seems to strict.
            // TODO: Revisit this.
            t => Ok(FileAttr::Other(t.to_string())),
        }
    }
}

/// Constructed files tree from a torrent file.
#[derive(Debug, Clone)]
pub struct FileTree<'a> {
    pub(crate) node: FileNode<'a>,
    pub(crate) num_of_files: usize,
}

/// Value enum to be passed as an argument to [`FileTree::sort_by_name`] or
/// [`FileTree::sort_by_size`]
pub enum SortOrd {
    Ascending,
    Desending,
}

impl<'a> FileTree<'a> {
    pub fn sort_by_name(&mut self, ord: SortOrd) {
        match ord {
            SortOrd::Ascending => self.node.sort_by_name_ascending(),
            SortOrd::Desending => self.node.sort_by_name_desending(),
        }
    }

    pub fn sort_by_size(&mut self, ord: SortOrd) {
        match ord {
            SortOrd::Ascending => self.node.sort_by_size_ascending(),
            SortOrd::Desending => self.node.sort_by_size_desending(),
        }
    }

    /// Recursively prints the file tree to stdout in a custom human-readable format, using
    /// indentation.
    #[cfg(feature = "client")]
    pub fn print(&self) {
        self.node.print_tree(4);
    }

    pub fn number_of_files(&self) -> usize {
        self.num_of_files
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FileNode<'a> {
    Dir {
        parent: Cow<'a, str>,
        children: IndexMap<String, FileNode<'a>>,
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
    #[inline]
    pub(crate) fn new_dir(name: &'a str) -> Self {
        FileNode::Dir {
            parent: Cow::from(name),
            children: IndexMap::new(),
            length: 0,
        }
    }

    #[inline]
    pub(crate) fn new_file(name: &'a str, length: usize) -> Self {
        FileNode::File {
            name: Cow::from(name),
            length,
        }
    }

    #[inline]
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
            FileNode::File { .. } => {
                // If we're trying to add a path to a file, something's wrong
                panic!("Attempting to add a path to a file node");
            }
        }
    }

    #[inline]
    fn len(&self) -> usize {
        match self {
            FileNode::Dir { length, .. } => *length,
            FileNode::File { length, .. } => *length,
        }
    }

    #[inline]
    fn sort_by_name_ascending(&mut self) {
        match self {
            FileNode::Dir { children, .. } => {
                children.sort_by(|k1, _, k2, _| k1.to_lowercase().cmp(&k2.to_lowercase()));

                for child in children.values_mut() {
                    child.sort_by_name_ascending();
                }
            }
            FileNode::File { .. } => (),
        }
    }

    #[inline]
    fn sort_by_name_desending(&mut self) {
        match self {
            FileNode::Dir { children, .. } => {
                children.sort_by(|k1, _, k2, _| k2.to_lowercase().cmp(&k1.to_lowercase()));

                for child in children.values_mut() {
                    child.sort_by_name_ascending();
                }
            }
            FileNode::File { .. } => (),
        }
    }

    #[inline]
    fn sort_by_size_ascending(&mut self) {
        match self {
            FileNode::Dir { children, .. } => {
                children.sort_by(|_, v1, _, v2| v1.len().cmp(&v2.len()));

                for child in children.values_mut() {
                    child.sort_by_size_ascending();
                }
            }
            FileNode::File { .. } => (),
        }
    }

    #[inline]
    fn sort_by_size_desending(&mut self) {
        match self {
            FileNode::Dir { children, .. } => {
                children.sort_by(|_, v1, _, v2| v2.len().cmp(&v1.len()));

                for child in children.values_mut() {
                    child.sort_by_size_ascending();
                }
            }
            FileNode::File { .. } => (),
        }
    }

    /// Recursively prints the file tree in a human-readable format, using indentation.
    ///
    /// ## Arguments:
    ///
    /// `indent`: Indentation step to use for printing child data in the file structure hirarcy.
    #[cfg(feature = "client")]
    #[inline]
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
