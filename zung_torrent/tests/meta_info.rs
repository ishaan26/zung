use utilities::torrent::CLIENT;

// Values directly parsed and extracted from a torrent file.
mod getters {
    use super::*;

    #[test]
    fn title() {
        assert_eq!(CLIENT.arch.meta_info().title(), None);
        assert_eq!(
            CLIENT.mit.meta_info().title(),
            Some(&"MIT6.00SCS11".to_string())
        );
        assert_eq!(CLIENT.kali.meta_info().title(), None);
    }

    #[test]
    fn announce() {
        assert_eq!(CLIENT.arch.meta_info().announce(), None);
        assert_eq!(
            CLIENT.mit.meta_info().announce(),
            Some(&"http://bt1.archive.org:6969/announce".to_string())
        );
        assert_eq!(
            CLIENT.kali.meta_info().announce(),
            Some(&"http://tracker.kali.org:6969/announce".to_string())
        );
    }

    #[test]
    fn number_of_pieces() {
        assert_eq!(CLIENT.arch.meta_info().number_of_pieces(), 1911);
        assert_eq!(CLIENT.mit.meta_info().number_of_pieces(), 3259);
        assert_eq!(CLIENT.kali.meta_info().number_of_pieces(), 15650);
    }

    #[test]
    fn creation_date() {
        assert_eq!(
            CLIENT.arch.meta_info().creation_date().unwrap(),
            "Mon, 1 Apr 2024 18:00:29 +0000"
        );
        assert_eq!(
            CLIENT.mit.meta_info().creation_date().unwrap(),
            "Wed, 12 Sep 2012 22:35:35 +0000"
        );
        assert_eq!(
            CLIENT.kali.meta_info().creation_date().unwrap(),
            "Tue, 27 Feb 2024 13:12:54 +0000"
        );
    }

    #[test]
    fn creation_date_raw() {
        assert_eq!(
            CLIENT.arch.meta_info().creation_date_raw().unwrap(),
            1711994429
        );
        assert_eq!(
            CLIENT.mit.meta_info().creation_date_raw().unwrap(),
            1347489335
        );
        assert_eq!(
            CLIENT.kali.meta_info().creation_date_raw().unwrap(),
            1709039574
        );
    }

    #[test]
    fn comment() {
        assert_eq!(
            CLIENT.arch.meta_info().comment(),
            Some(&"Arch Linux 2024.04.01 <https://archlinux.org>".to_string())
        );

        assert_eq!(
            CLIENT.mit.meta_info().comment(),
            Some(&"This content hosted at the Internet Archive at http://archive.org/details/MIT6.00SCS11\nFiles may have changed, which prevents torrents from downloading correctly or completely; please check for an updated torrent at http://archive.org/download/MIT6.00SCS11/MIT6.00SCS11_archive.torrent\nNote: retrieval usually requires a client that supports webseeding (GetRight style).\nNote: many Internet Archive torrents contain a 'pad file' directory. This directory and the files within it may be erased once retrieval completes.\nNote: the file MIT6.00SCS11_meta.xml contains metadata about this torrent's contents.".to_string())
        );

        assert_eq!(
            CLIENT.kali.meta_info().comment().unwrap(),
            "kali-linux-2024.1-installer-amd64.iso from https://www.kali.org/get-kali/"
        );
    }

    #[test]
    fn created_by() {
        assert_eq!(
            CLIENT.arch.meta_info().created_by().unwrap(),
            "mktorrent 1.1"
        );
        assert_eq!(
            CLIENT.mit.meta_info().created_by(),
            Some(&"ia_make_torrent".to_string())
        );
        assert_eq!(
            CLIENT.kali.meta_info().created_by().unwrap(),
            "mktorrent 1.1"
        );
    }

    #[test]
    fn encoding() {
        assert!(CLIENT.arch.meta_info().encoding().is_none());
        assert!(CLIENT.mit.meta_info().encoding().is_none());
        assert!(CLIENT.kali.meta_info().encoding().is_none());
    }

    #[test]
    fn piece_length() {
        assert_eq!(CLIENT.arch.meta_info().piece_length(), 524288);
        assert_eq!(CLIENT.mit.meta_info().piece_length(), 4194304);
        assert_eq!(CLIENT.kali.meta_info().piece_length(), 262144);
    }

    #[test]
    fn torrent_size() {
        assert_eq!(CLIENT.arch.meta_info().size(), 1001914368);
        assert_eq!(CLIENT.mit.meta_info().size(), 13669236736);
        assert_eq!(CLIENT.kali.meta_info().size(), 4102553600);
    }

    #[test]
    fn url_list() {
        assert!(CLIENT.arch.meta_info().url_list().is_some());
        assert!(CLIENT.mit.meta_info().url_list().is_some());
        assert!(CLIENT.kali.meta_info().url_list().is_some());
        assert!(CLIENT.mc.meta_info().url_list().is_some()); // in this the url-list is [""].
                                                             // However, this should still return Some there is a entry of url list present in the
                                                             // torrent file.
    }

    #[test]
    fn announce_list() {
        assert!(CLIENT.arch.meta_info().announce_list().is_none());
        assert!(CLIENT.mit.meta_info().announce_list().is_some());
        assert!(CLIENT.kali.meta_info().announce_list().is_some());
        assert!(CLIENT.mc.meta_info().announce_list().is_some());
    }
}

// Value calculated by the program from a torrent file.
mod calculators {
    use super::*;

    #[test]
    fn info_hash_to_string() {
        let arch = CLIENT.arch.info_hash().to_string();
        let mit = CLIENT.mit.info_hash().to_string();
        let kali = CLIENT.kali.info_hash().to_string();

        // compared with info hashes as generated by qbittorrent.
        assert_eq!(arch, "6853ab2b86b2cb6a3c778b8aafe3dffd94242321");
        assert_eq!(mit, "c5f1f7e86c5f18636a4b64502299c3880d9085a8");
        assert_eq!(kali, "f24f4f54df51118b03f99c74416e4554ab88d22b");
    }

    #[test]
    fn info_hash_as_bytes() {
        let arch = hex::encode(CLIENT.arch.info_hash().as_bytes());
        let mit = hex::encode(CLIENT.mit.info_hash().as_bytes());
        let kali = hex::encode(CLIENT.kali.info_hash().as_bytes());

        // compared with info hashes as generated by qbittorrent.
        assert_eq!(arch, "6853ab2b86b2cb6a3c778b8aafe3dffd94242321");
        assert_eq!(mit, "c5f1f7e86c5f18636a4b64502299c3880d9085a8");
        assert_eq!(kali, "f24f4f54df51118b03f99c74416e4554ab88d22b");
    }

    #[test]
    fn info_hash_url_encode() {
        let arch = CLIENT.arch.info_hash().to_url_encoded();
        let mit = CLIENT.mit.info_hash().to_url_encoded();
        let kali = CLIENT.kali.info_hash().to_url_encoded();

        // compared with info hashes as generated by qbittorrent.
        assert_eq!(
            arch,
            "%68%53%ab%2b%86%b2%cb%6a%3c%77%8b%8a%af%e3%df%fd%94%24%23%21"
        );
        assert_eq!(
            mit,
            "%c5%f1%f7%e8%6c%5f%18%63%6a%4b%64%50%22%99%c3%88%0d%90%85%a8"
        );
        assert_eq!(
            kali,
            "%f2%4f%4f%54%df%51%11%8b%03%f9%9c%74%41%6e%45%54%ab%88%d2%2b"
        );
    }

    #[test]
    fn number_of_files() {
        assert_eq!(CLIENT.arch.number_of_files(), 1);
        assert_eq!(CLIENT.kali.number_of_files(), 1);
        assert_eq!(CLIENT.mit.number_of_files(), 154);
        assert_eq!(CLIENT.mc.number_of_files(), 131934);
    }
}
