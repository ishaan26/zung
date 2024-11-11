use utilities::torrent::TestClient;
use zung_torrent::sources::DownloadSources;

#[test]
fn source_types() {
    let clients = TestClient::new();

    let arch = clients.arch.sources();
    let mit = clients.mit.sources();
    let mc = clients.mc.sources();
    let kali = clients.kali.sources();

    matches!(arch, DownloadSources::HttpSeeder { .. });
    matches!(mit, DownloadSources::Hybrid { .. });
    matches!(mc, DownloadSources::Tracker { .. });
    matches!(kali, DownloadSources::Tracker { .. });
}

#[test]
fn arch_source() {
    let clients = TestClient::new();

    let arch = clients.arch;

    let sources = arch.sources();
    let sources = sources
        .get_http_seeders_requests()
        .expect("This should be some");

    for s in sources {
        assert!(s.to_url().contains(arch.meta_info().info().name()))
    }
}

#[test]
fn mit_source() {
    let clients = TestClient::new();

    let mit = clients.mit;

    let sources = mit.sources();

    let tracker_sources = sources
        .get_tracker_requests()
        .expect("There should be some");

    let http_sources = sources
        .get_http_seeders_requests()
        .expect("This should be some");

    for s in tracker_sources {
        assert!(s
            .to_url()
            .unwrap()
            .contains(&mit.info_hash().to_url_encoded()));

        assert!(s
            .to_url()
            .unwrap()
            .contains(&mit.peer_id().to_url_encoded()));
    }

    for s in http_sources {
        assert!(s.to_url().contains(mit.meta_info().info().name()))
    }
}
