use std::path::PathBuf;
use zung_torrent::*;

pub struct TestClient {
    pub arch: Client,
    pub mit: Client,
    pub kali: Client,
    pub mc: Client,
}

impl TestClient {
    pub fn new() -> Self {
        // Contains only url-list and no announce field
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("sample_torrents/archlinux-2024.04.01-x86_64.iso.torrent");
        let arch = Client::new(path).expect("Unable to open the arch torrrent");

        // Contains both url-list and announce field
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("sample_torrents/MIT6.00SCS11_archive.torrent");
        let mit = Client::new(path).expect("Unable to read mit torrent");

        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("sample_torrents/kali-linux-2024.1-installer-amd64.iso.torrent");
        let kali = Client::new(path).expect("Unable to read kali torrent");

        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("sample_torrents/MC_GRID-7f06f8280a3b496f2af0f78131ced619df14a0c3.torrent");
        let mc = Client::new(path).expect("Unable to read kali torrent");

        TestClient {
            arch,
            mit,
            kali,
            mc,
        }
    }
}

impl Default for TestClient {
    fn default() -> Self {
        Self::new()
    }
}
