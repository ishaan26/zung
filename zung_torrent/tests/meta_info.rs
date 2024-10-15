use std::path::PathBuf;
use zung_torrent::*;

struct TestMetaInfo {
    arch: Client,
    mit: Client,
    kali: Client,
}

impl TestMetaInfo {
    fn new() -> Self {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests/sample_torrents/archlinux-2024.04.01-x86_64.iso.torrent");

        let arch = Client::new(path).expect("Unable to open the arch torrrent");

        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests/sample_torrents/MIT6.00SCS11_archive.torrent");

        let mit = Client::new(path).expect("Unable to read mit torrent");

        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests/sample_torrents/kali-linux-2024.1-installer-amd64.iso.torrent");

        let kali = Client::new(path).expect("Unable to read kali torrent");

        TestMetaInfo { arch, mit, kali }
    }
}

mod getters {
    use super::*;

    #[test]
    fn title() {
        let tester = TestMetaInfo::new();

        assert_eq!(tester.arch.meta_info().title(), None);
        assert_eq!(
            tester.mit.meta_info().title(),
            Some(&"MIT6.00SCS11".to_string())
        );
        assert_eq!(tester.kali.meta_info().title(), None);
    }

    #[test]
    fn announce() {
        let tester = TestMetaInfo::new();

        assert_eq!(tester.arch.meta_info().announce(), None);
        assert_eq!(
            tester.mit.meta_info().announce(),
            Some(&"http://bt1.archive.org:6969/announce".to_string())
        );
        assert_eq!(
            tester.kali.meta_info().announce(),
            Some(&"http://tracker.kali.org:6969/announce".to_string())
        );
    }
}
