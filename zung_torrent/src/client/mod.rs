use anyhow::{bail, Result};
use colored::Colorize;
use human_bytes::human_bytes;
use zung_parsers::bencode;

use std::{cell::OnceCell, fmt::Display, path::Path, sync::Arc, thread};

use crate::{
    meta_info::{FileTree, InfoHash},
    MetaInfo,
};

#[derive(Debug)]
pub struct Client {
    meta_info: Arc<MetaInfo>,
    file_name: String,
    info_hash: InfoHash,
    num_files: OnceCell<usize>, // Cache no. of files when calling either file_tree or
                                // number_of_files methods.
}

impl Client {
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

                let info = bencode::to_bytes(info).unwrap();

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
                num_files: OnceCell::new(),
            })
        } else {
            bail!("File not found")
        }
    }

    pub fn meta_info(&self) -> &MetaInfo {
        &self.meta_info
    }

    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    pub fn print_torrent_info(&self) {
        println!("\"{}\" ", self.file_name.magenta().bold().underline(),);

        print_info("Title", self.meta_info.title());

        // Length and pieces details
        let npieces = self.meta_info.number_of_pieces();
        let plen = self.meta_info.piece_length();
        let size = (npieces * plen) as f64;

        println!(
            "\n{} Number of pieces: {} each {} in size. Total torrent size: {}",
            "==>".green().bold(),
            npieces.to_string().bold().cyan(),
            human_bytes(plen as f64).bold().cyan(),
            human_bytes(size).bold().cyan()
        );

        let mut handle = Vec::new();

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

        for h in handle {
            h.join().expect("Failed to print information");
        }

        // info_hash
        print_info("Info Hash", Some(self.info_hash().to_string()));

        print_info("Number of Files", Some(self.number_of_files()));
    }

    pub fn print_torrent_files(&self) {
        println!("\n{} Files:", "==>".green().bold());
        self.file_tree().print_file_tree();
    }

    pub fn info_hash(&self) -> &InfoHash {
        &self.info_hash
    }

    pub fn file_tree(&self) -> FileTree<'_> {
        let tree = self.meta_info.info.build_file_tree();
        if self.num_files.get().is_none() {
            self.num_files.set(tree.n).unwrap(); // num_files is None.
        }
        tree
    }

    pub fn number_of_files(&self) -> usize {
        *self
            .num_files
            .get_or_init(|| self.meta_info.info().build_file_tree().number_of_files())
    }
}

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
