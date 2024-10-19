mod files;
mod info;
mod pieces;

use anyhow::Result;
use chrono::{DateTime, Utc};

pub use files::{FileTree, Files, SortOrd};
pub use info::{Info, InfoHash};

use serde::{Deserialize, Serialize};
use zung_parsers::bencode;

/// A type reprasenting deserialized [Metainfo files](https://en.wikipedia.org/wiki/Torrent_file)
/// (also known as .torrent files)
///
/// Meteinfo files are bencoded dictionaries that contains metadata about files and folders to be
/// distributed, and usually also: a list of the network locations of
/// [trackers](https://en.wikipedia.org/wiki/BitTorrent_tracker)
#[derive(Debug, Serialize, Deserialize)]
pub struct MetaInfo {
    // A dictionary that describes the file(s) of the torrent. There are two possible forms: one for
    // the case of a 'single-file' torrent with no directory structure, and one for the case of a
    // 'multi-file' torrent (see below for details)
    pub(crate) info: Info,

    // The announce URL of the tracker (string)
    pub(crate) announce: Option<String>,

    // (BEP: 19) For using HTTP or FTP servers as seeds for BitTorrent downloads.
    //
    // This key refers to a one or more URLs, and contains a list of web addresses where torrent
    // data can be retrieved directly from a server instead of from a peer.
    #[serde(rename = "url-list")]
    pub(crate) url_list: Option<Vec<String>>,

    // (BEP: 12) This is an extension to the official specification, offering
    // backwards-compatibility. (list of lists of strings).
    #[serde(rename = "announce-list")]
    pub(crate) announce_list: Option<Vec<Vec<String>>>,

    // Title of the torrent file
    pub(crate) title: Option<String>,

    // The creation time of the torrent, in standard UNIX epoch format (integer, seconds since 1-Jan-1970 00:00:00 UTC)
    #[serde(rename = "creation date")]
    pub(crate) creation_date: Option<i64>,

    // Free-form textual comments of the author (string)
    pub(crate) comment: Option<String>,

    // Name and version of the program used to create the .torrent (string)
    #[serde(rename = "created by")]
    pub(crate) created_by: Option<String>,

    // The string encoding format used to generate the pieces part of the info dictionary in
    // the .torrent metafile (string)
    pub(crate) encoding: Option<String>,
}

impl MetaInfo {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let meta_info: Self = bencode::from_bytes(bytes)?;
        Ok(meta_info)
    }

    /// Title of the torrent file (if any)
    pub fn title(&self) -> Option<&String> {
        self.title.as_ref()
    }

    /// Returns the number of piece sha1 hashes contained in a torrent file.
    pub fn number_of_pieces(&self) -> usize {
        self.info.pieces.len()
    }

    /// Returns the creation time of the torrent, in [RFC
    /// 2822](https://www.rfc-editor.org/rfc/rfc2822) format
    pub fn creation_date(&self) -> Option<String> {
        self.creation_date
            .and_then(|datetime| DateTime::<Utc>::from_timestamp(datetime, 0))
            .map(|datetime| datetime.to_rfc2822())
    }

    /// Returns the creation the creation time of the torrent, in standard UNIX epoch format.
    pub fn creation_date_raw(&self) -> Option<i64> {
        self.creation_date
    }

    /// Returns comments of the author contained in the torrent file (if any).
    pub fn comment(&self) -> Option<&String> {
        self.comment.as_ref()
    }

    pub fn created_by(&self) -> Option<&String> {
        self.created_by.as_ref()
    }

    pub fn encoding(&self) -> Option<&String> {
        self.encoding.as_ref()
    }

    pub fn files(&self) -> &Files {
        &self.info.files
    }

    pub fn info(&self) -> &Info {
        &self.info
    }

    pub fn announce(&self) -> Option<&String> {
        self.announce.as_ref()
    }

    pub fn url_list(&self) -> Option<&Vec<String>> {
        self.url_list.as_ref()
    }

    pub fn announce_list(&self) -> Option<&Vec<Vec<String>>> {
        self.announce_list.as_ref()
    }

    pub fn piece_length(&self) -> usize {
        self.info.piece_length
    }

    pub fn build_file_tree(&self) -> FileTree<'_> {
        self.info.build_file_tree()
    }

    pub fn torrent_size(&self) -> usize {
        self.info.torrent_size()
    }
}
