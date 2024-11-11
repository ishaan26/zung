//! For using HTTP or FTP servers as seeds for BitTorrent downloads.

/// Rerasents a HTTP Seeder request url.
#[derive(Debug)]
pub struct HttpSeederRequest {
    url: String,
}

impl HttpSeederRequest {
    pub fn new(base_url: &str, name: Option<&str>, file_path: &str) -> Self {
        let mut url = String::new();
        url.push_str(base_url);
        if let Some(name) = name {
            url.push_str(name);
            url.push('/');
        }
        url.push_str(file_path);
        Self { url }
    }

    pub fn to_url(&self) -> &str {
        &self.url
    }
}
