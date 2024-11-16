mod peer_id;
pub use peer_id::PeerID;

use anyhow::{bail, Result};
use colored::Colorize;
use human_bytes::human_bytes;
use zung_parsers::bencode;

use std::{cell::OnceCell, fmt::Display, path::Path, sync::Arc, thread};

use crate::{
    meta_info::{FileTree, InfoHash, SortOrd},
    sources::{DownloadSources, HttpSeederList, TrackerList},
    MetaInfo,
};

/// A torrent client providing the methods to interact with a torrent file.
#[derive(Debug)]
pub struct Client {
    meta_info: Arc<MetaInfo>,
    file_name: String,
    info_hash: InfoHash,
    peer_id: PeerID,
    num_files: OnceCell<usize>, // Cache no. of files.
}

/// Main functions
impl Client {
    /// Creates a new [`Client`] by reading and parsing the provided torrent file.
    ///
    /// This method will also calculate and store the `info_hash` of the torrent in memory.
    ///
    /// # Arguments
    ///
    /// * `file` - A reference to the path of the torrent file.
    ///
    /// # Returns
    ///
    /// Returns a `Result<Client>` that contains the initialized client if successful,
    /// or an error otherwise.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use zung_torrent::Client;
    ///
    /// # fn client(path_to_torrent: &str) {
    /// let client = Client::new(path_to_torrent).expect("Failed to create client");
    /// # }
    ///
    /// ```
    pub fn new<P>(file: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        if let Some(file_name) = file.as_ref().file_name() {
            let file_name = file_name.to_string_lossy().to_string();

            let file = std::fs::read(file).expect("Unable to read the provided file");

            let value = bencode::parse(&file)?;

            let meta_info = thread::spawn(move || {
                MetaInfo::from_bytes(&file).expect("Invalid torrent file provided")
            });

            let info = thread::spawn(move || {
                let info = value
                    .get_from_dictionary("info")
                    .expect("Invalid Torrent File - No info dictionary provided");

                let info = bencode::to_bytes(info).expect("Failed to calculate the info hash");

                InfoHash::new(&info)
            });

            let meta_info = Arc::new(
                meta_info
                    .join()
                    .expect("Unable to deserialize the torrent file"),
            );
            let info_hash = info.join().expect("Unable to calculate infohash");

            Ok(Client {
                meta_info,
                file_name,
                info_hash,
                peer_id: PeerID::new(),
                num_files: OnceCell::new(),
            })
        } else {
            bail!("File not found")
        }
    }

    /// Returns a reference to the torrent's [`MetaInfo`].
    ///
    /// # Examples
    ///
    /// ```
    /// use zung_torrent::Client;
    ///
    /// # fn client(path_to_torrent: &str) {
    /// let client = Client::new(path_to_torrent).expect("Failed to create client");
    /// let meta_info = client.meta_info();
    /// # }
    /// ```
    pub fn meta_info(&self) -> &MetaInfo {
        &self.meta_info
    }

    /// Returns the file name of the torrent file.
    ///
    /// # Examples
    ///
    /// ```
    /// use zung_torrent::Client;
    ///
    /// # fn client(path_to_torrent: &str) {
    /// let client = Client::new(path_to_torrent).expect("Failed to create client");
    /// let file_name = client.file_name();
    /// println!("Torrent file: {}", file_name); // Prints the file name as passed in under
    ///                                          // path_to_torrent
    /// # }
    /// ```
    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    /// Returns the info hash of the torrent.
    ///
    /// It is the 20 byte sha1 hash of the bencoded form of the `info` value from the metainfo
    /// file. This purpose of calculating this value is to verify the integrity of contents of the
    /// `info` section in a torrent file (which contains critical information such as file names
    /// and paths).
    ///
    /// Since the info hash of a torrent is a fundamental value for using any torrent, this value
    /// is calculated at initialization of the [`Client`] with [`Client::new`]. This method only
    /// returns a reference to the calculated value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use zung_torrent::Client;
    ///
    /// # fn client(path_to_torrent: &str) {
    /// let client = Client::new(path_to_torrent).expect("Failed to create client");
    /// let info_hash = client.info_hash();
    /// println!("Info Hash: {}", info_hash);
    /// # }
    /// ```
    pub fn info_hash(&self) -> &InfoHash {
        &self.info_hash
    }

    /// Builds and returns the file tree structure of the torrent.
    ///
    /// This method also caches the number of files if not already done.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use zung_torrent::Client;
    ///
    /// # fn client(path_to_torrent: &str) {
    /// let client = Client::new(path_to_torrent).expect("Failed to create client");
    /// let file_tree = client.file_tree();
    /// # }
    /// ```
    pub fn file_tree(&self) -> FileTree<'_> {
        let tree = self.meta_info.info.build_file_tree();
        if self.num_files.get().is_none() {
            self.num_files.set(tree.num_of_files).unwrap(); // num_files is None.
        }
        tree
    }

    /// Returns the total number of files in the torrent.
    ///
    /// This is will build the torrent's  [`FileTree`] if not already built and then store and
    /// return the value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use zung_torrent::Client;
    ///
    /// # fn client(path_to_torrent: &str) {
    /// let client = Client::new(path_to_torrent).expect("Failed to create client");
    /// let num_files = client.number_of_files();
    /// println!("Number of files: {}", num_files);
    /// # }
    /// ```
    pub fn number_of_files(&self) -> usize {
        *self
            .num_files
            .get_or_init(|| self.meta_info.info().build_file_tree().number_of_files())
    }

    /// Returns the [`PeerID`] of this [`Client`].
    pub fn peer_id(&self) -> PeerID {
        self.peer_id
    }

    /// Returns the [`DownloadSources`] generated from the information contained in the
    /// [`MetaInfo`] type.
    ///
    /// See the type documentation for more information on the usage.
    pub fn sources(&self) -> DownloadSources {
        DownloadSources::new(self.meta_info())
    }
}

/// Printer functions.
impl Client {
    /// Prints detailed information about the torrent file, including title, number of pieces,
    /// total size, creation date, and more.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use zung_torrent::Client;
    ///
    /// # fn client(path_to_torrent: &str) {
    /// let client = Client::new(path_to_torrent).expect("Failed to create client");
    /// let num_files = client.number_of_files();
    /// client.print_torrent_info();
    /// # }
    /// ```
    pub fn print_torrent_info(&self) {
        println!("\"{}\" ", self.file_name.magenta().bold().underline(),);

        let info_hash = self.info_hash().to_string();

        let mut handle = Vec::new();

        // Title
        let meta_info = Arc::clone(&self.meta_info);
        handle.push(thread::spawn(move || {
            print_info("Title", meta_info.title());
        }));

        // Length and pieces details
        let meta_info = Arc::clone(&self.meta_info);
        handle.push(thread::spawn(move || {
            let npieces = meta_info.number_of_pieces();
            let plen = meta_info.piece_length();
            let size = (npieces * plen) as f64;

            println!(
                "\n{} Number of pieces: {} each {} in size. Total torrent size: {}",
                "==>".green().bold(),
                npieces.to_string().bold().cyan(),
                human_bytes(plen as f64).bold().cyan(),
                human_bytes(size).bold().cyan()
            );
        }));

        // number of Files
        let meta_info = Arc::clone(&self.meta_info);
        handle.push(thread::spawn(move || {
            print_info(
                "Number of Files",
                Some(meta_info.info().build_file_tree().number_of_files()),
            );
        }));

        // created on
        let meta_info = Arc::clone(&self.meta_info);
        handle.push(thread::spawn(move || {
            print_info("Created on", meta_info.creation_date());
        }));

        // created by
        let meta_info = Arc::clone(&self.meta_info);
        handle.push(thread::spawn(move || {
            print_info("Created by", meta_info.created_by());
        }));

        // comment
        let meta_info = Arc::clone(&self.meta_info);
        handle.push(thread::spawn(move || {
            print_info("Comment", meta_info.comment());
        }));

        // Encoded in
        let meta_info = Arc::clone(&self.meta_info);
        handle.push(thread::spawn(move || {
            print_info("Encoded in", meta_info.encoding());
        }));

        // info_hash
        handle.push(thread::spawn(move || {
            print_info("Info Hash", Some(info_hash));
        }));

        for h in handle {
            h.join().expect("Failed to print information");
        }
    }

    /// Prints a list of all files in the torrent, sorted by size.
    ///
    /// # Arguments
    ///
    /// * `ord` - Sorting order, either ascending or descending.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use zung_torrent::Client;
    /// use zung_torrent::meta_info::SortOrd;
    ///
    /// # fn client(path_to_torrent: &str) {
    /// let client = Client::new(path_to_torrent).expect("Failed to create client");
    ///
    /// client.print_files_by_size(SortOrd::Ascending);
    /// # }
    /// ```
    pub fn print_files_by_size(&self, ord: SortOrd) {
        println!("\n{} Files:", "==>".green().bold());
        let mut filetree = self.file_tree();
        filetree.sort_by_size(ord);
        filetree.print();
    }

    /// Prints a list of all files in the torrent, sorted by name.
    ///
    /// # Arguments
    ///
    /// * `ord` - Sorting order, either ascending or descending.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use zung_torrent::Client;
    /// use zung_torrent::meta_info::SortOrd;
    ///
    /// # fn client(path_to_torrent: &str) {
    /// let client = Client::new(path_to_torrent).expect("Failed to create client");
    ///
    /// client.print_files_by_name(SortOrd::Ascending);
    /// # }
    pub fn print_files_by_name(&self, ord: SortOrd) {
        println!("\n{} Files:", "==>".green().bold());
        let mut filetree = self.file_tree();
        filetree.sort_by_size(ord);
        filetree.print();
    }

    /// Prints the download sources generated from the [`MetaInfo`] file to stdout.
    pub fn print_download_sources(&self) {
        #[inline]
        fn print_trackers(tracker_list: TrackerList) {
            print_header("Trackers");
            for (mut i, tracker) in tracker_list.iter().enumerate() {
                i += 1;
                println!("\t{i}. {}", tracker.url().bold().cyan())
            }
        }

        #[inline]
        fn print_http_seeders(http_seeder_list: HttpSeederList<'_>) {
            print_header("HTTP Seeders");
            for (mut i, http) in http_seeder_list.iter().enumerate() {
                i += 1;
                println!("\t{i} : {}", http.0.bold().cyan());
                for (mut j, url) in http.1.urls().iter().enumerate() {
                    j += 1;
                    println!("\t\t{j}. {url}")
                }
            }
        }

        match self.sources() {
            DownloadSources::Trackers { tracker_list } => {
                print_trackers(tracker_list);
            }
            DownloadSources::HttpSeeders { http_seeder_list } => {
                print_http_seeders(http_seeder_list);
            }
            DownloadSources::Hybrid {
                tracker_list,
                http_seeder_list,
            } => {
                print_trackers(tracker_list);
                print_http_seeders(http_seeder_list);
            }
        }
    }
}

// helper function
fn print_info<T: Display>(header: &str, value: Option<T>) {
    if let Some(value) = value {
        println!(
            "\n{} {header}: {}",
            "==>".green().bold(),
            value.to_string().bold().cyan()
        );
    } else {
        println!(
            "\n{} {header}: {}",
            "==>".green().bold(),
            "not present".italic().dimmed()
        );
    }
}

fn print_header(header: &str) {
    println!("\n{} {header}: ", "==>".green().bold(),);
}
