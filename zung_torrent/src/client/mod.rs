use anyhow::{bail, Result};
use colored::Colorize;
use human_bytes::human_bytes;

use std::{fmt::Display, path::Path};

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

        // created on
        print_info("Created on", self.meta_info.creation_date());

        // created by
        print_info("Created by", self.meta_info.created_by());

        // comment
        print_info("Comment", self.meta_info.comment());

        // Encoded in
        print_info("Encoded in", self.meta_info.encoding());
    }

    pub fn print_torrent_files(&self) {
        println!("\n{} Files:", "==>".green().bold(),);
        self.meta_info.info().build_file_tree().print_tree(0);
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
