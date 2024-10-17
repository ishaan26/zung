use anyhow::{bail, Result};
use colored::Colorize;
use human_bytes::human_bytes;
use zung_parsers::bencode;

use std::{fmt::Display, path::Path, sync::Arc, thread};

use crate::{meta_info::InfoHash, MetaInfo};

#[derive(Debug)]
pub struct Client {
    meta_info: Arc<MetaInfo>,
    file_name: String,
    info_hash: InfoHash,
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

            let info = thread::spawn(move || {
                let info = value
                    .get_from_dictionary("info")
                    .expect("Invalid Torrent File - No info dictionary provided");

                let info = bencode::to_bytes(info).unwrap();

                InfoHash::new(&info)
            });

            let meta_info = thread::spawn(move || {
                MetaInfo::from_bytes(&file).expect("Invalid torrent file provided")
            });

            let info_hash = info.join().unwrap();
            let meta_info = Arc::new(meta_info.join().unwrap());

            Ok(Client {
                meta_info,
                file_name,
                info_hash,
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

        // created on
        let meta_info = Arc::clone(&self.meta_info);
        thread::spawn(move || {
            print_info("Created on", meta_info.creation_date());
        });

        // created by
        let meta_info = Arc::clone(&self.meta_info);
        thread::spawn(move || {
            print_info("Created by", meta_info.created_by());
        });

        // comment
        let meta_info = Arc::clone(&self.meta_info);

        thread::spawn(move || {
            print_info("Comment", meta_info.comment());
        });

        // Encoded in
        let meta_info = Arc::clone(&self.meta_info);
        thread::spawn(move || {
            print_info("Encoded in", meta_info.encoding());
        });

        // info_hash
        print_info("Info Hash", Some(self.info_hash().to_string()));
    }

    pub fn print_torrent_files(&self) {
        println!("\n{} Files:", "==>".green().bold(),);
        self.meta_info.info().build_file_tree().print_tree(0);
    }

    pub fn info_hash(&self) -> &InfoHash {
        &self.info_hash
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
            "__not present__".italic().dimmed()
        );
    }
}