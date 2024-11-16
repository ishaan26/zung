use utilities::torrent::TestClient;
use zung_torrent::sources::DownloadSources;

#[test]
fn source_types() {
    let clients = TestClient::new();

    let arch = clients.arch.sources();
    let mit = clients.mit.sources();
    let mc = clients.mc.sources();
    let kali = clients.kali.sources();

    matches!(arch, DownloadSources::HttpSeeders { .. });
    matches!(mit, DownloadSources::Hybrid { .. });
    matches!(mc, DownloadSources::Trackers { .. });
    matches!(kali, DownloadSources::Trackers { .. });
}

#[test]
fn arch_source() {
    let clients = TestClient::new();

    let arch = clients.arch;

    let sources = arch.sources();
    let sources = sources.http_seeders().expect("This should be some");

    for s in sources {
        for u in &s.1 {
            assert!(u.contains(arch.meta_info().info().name()))
        }
    }
}

#[test]
fn mit_source() {
    let clients = TestClient::new();

    let mit = clients.mit;

    let sources = mit.sources();

    assert!(sources.trackers().is_some());

    let http_sources = sources.http_seeders().expect("This should be some");

    for s in http_sources {
        for u in &s.1 {
            assert!(u.contains(mit.meta_info().info().name()))
        }
    }
}
