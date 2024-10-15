use anyhow::{bail, Result};
use colored::Colorize;
use human_bytes::human_bytes;

use std::path::Path;

use zung_parsers::bencode;

use crate::MetaInfo;

pub struct Client {
    meta_info: MetaInfo,
    file_name: String,
}

impl Client {
    pub fn new<P>(file: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        if let Some(file_name) = file.as_ref().file_name() {
            let file_name = file_name.to_string_lossy().to_string();

            let file = std::fs::read(file).expect("Unable to read the provided file");

            let meta_info =
                bencode::from_bytes(&file).expect("The file provided is not a valid torrent file");

            Ok(Client {
                meta_info,
                file_name,
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

        // created by
        if let Some(s) = self.meta_info.created_by() {
            println!("\n{} Created by: {}", "==>".green().bold(), s.bold().cyan());
        }

        // comment
        if let Some(s) = self.meta_info.comment() {
            println!("\n{} Comment: {}", "==>".green().bold(), s.bold().cyan());
        }

        // Files
        println!("\n{} Files:", "==>".green().bold(),);
        self.meta_info.info().build_file_tree().print_tree(0);

        // Files::SingleFile { length, .. } => {
        //     let length = human_bytes(*length as f64);
        //     if let Some(name) = &self.meta_info.info().name {
        //         println!("     1. {}: {}", name.bold().magenta(), length.cyan())
        //     } else {
        //         println!("__No name__ : {length}")
        //     }
        // }

        // Files::MultiFile { files } => {}
        // if let FileState::SingleFile { name, length } = files_info {
        //     let length = human_bytes(length as u32);
        //     if let Some(name) = name {
        //         println!("     1. {}: {length}", name.bold())
        //     } else {
        //         println!("__No name__ : {length}")
        //     }
        // } else if let FileState::MultiFile(map) = torrent.files() {
        //     for (i, (name, length)) in map.iter().enumerate() {
        //         let length = human_bytes(*length as u32);
        //         println!("     {i}. {}: {length}", name.bold())
        //     }
        // }

        // let info_hash = torrent.info_hash();
        // println!(
        //     "\n{} Info Hash: {} ",
        //     "==>".green().bold(),
        //     info_hash.encode_to_hex_string().bold().cyan()
        // );
        //
        // if verbose {
        //     println!("Tracker URL: {}", torrent.announce());
        //
        //     if let Some(al) = torrent.announce_list() {
        //         al.iter().for_each(|a| println!("{a:?}"));
        //     }
        //
        //     println!("Piece Length: {}", torrent.piece_length());
        //
        //     println!("Piece Hashes:");
        //     for hash in torrent.piece_hashes_hex() {
        //         println!("{hash}");
        //     }
        // }
    }
}
