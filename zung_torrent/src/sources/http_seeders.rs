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
