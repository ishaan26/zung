//! Provides functionality to interact with HTTP seeders (also known as "web seeds").
//!
//! HTTP seeders (also known as "web seeds") in BitTorrent are alternative sources for downloading
//! torrent data directly from web servers using HTTP/HTTPS, rather than from P2P peers. They were
//! introduced to ensure availability of torrents even when there are few or no regular peers.
//!
//! HTTP seeding comes in several variations, each suited to different use cases. The original
//! GetRight-style web seeding allows standard web servers to serve complete files. The more
//! sophisticated HTTP/FTP seeding specification (BEP 19) enables servers to serve individual
//! pieces of files, matching the granular nature of BitTorrent's peer-to-peer transfers. A third
//! variant, Metalink (BEP 49), provides a way to specify multiple HTTP sources for the same
//! content.
//!
//! HTTP seeders are defined in the torrent metadata using either the "url-list" or "httpseeds"
//! keys. Unlike regular peers who must maintain complex BitTorrent protocol states and participate
//! in piece selection and trading algorithms, HTTP seeders simply respond to standard HTTP
//! requests. This simplicity makes them easier to implement and maintain, though it comes at the
//! cost of the bandwidth efficiency that makes peer-to-peer networks so powerful.

use std::ops::Deref;

use crate::meta_info::{FileAttr, Files, MetaInfo};

#[derive(Debug, Clone)]
pub struct HttpSeederList {
    http_seeder_list: Vec<(String, HttpSeeder)>,
}

impl HttpSeederList {
    pub fn new(http_seeder_list: Vec<(String, HttpSeeder)>) -> Self {
        Self { http_seeder_list }
    }

    pub fn http_seeder_list(&self) -> &[(String, HttpSeeder)] {
        &self.http_seeder_list
    }
}

impl Deref for HttpSeederList {
    type Target = [(String, HttpSeeder)];

    fn deref(&self) -> &Self::Target {
        self.http_seeder_list()
    }
}

#[derive(Debug, Clone)]
pub struct HttpSeeder {
    urls: Vec<String>,
}

impl Deref for HttpSeeder {
    type Target = [String];

    fn deref(&self) -> &Self::Target {
        self.urls()
    }
}

impl<'a> IntoIterator for &'a HttpSeeder {
    type Item = &'a String;

    type IntoIter = std::slice::Iter<'a, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.urls.iter()
    }
}

impl HttpSeeder {
    pub fn new(base_url: &str, meta_info: &MetaInfo) -> Self {
        let name = meta_info.info().name();
        match &meta_info.info().files {
            Files::SingleFile { attr, .. } => {
                if let Some(FileAttr::Padding) = attr {
                    HttpSeeder { urls: Vec::new() }
                } else {
                    let mut url = base_url.to_string();
                    url.push_str(name);
                    HttpSeeder { urls: vec![url] }
                }
            }
            Files::MultiFile { files } => {
                let mut urls = Vec::with_capacity(files.len());
                for file in files {
                    if let Some(attr) = &file.attr {
                        if attr.is_padding_file() {
                            continue;
                        }
                    }
                    for path in &file.path {
                        let mut url = base_url.to_string();

                        if &url[url.len() - 1..] != "/" {
                            url.push('/');
                        }

                        url.push_str(name);
                        url.push('/');
                        url.push_str(path);
                        urls.push(url);
                    }
                }
                HttpSeeder { urls }
            }
        }
    }

    pub fn urls(&self) -> &[String] {
        &self.urls
    }
}
