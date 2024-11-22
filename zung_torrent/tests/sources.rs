use futures::StreamExt;
use utilities::torrent::CLIENT;
use zung_torrent::sources::DownloadSources;

#[test]
fn source_types() {
    let arch = CLIENT.arch.sources();
    let mit = CLIENT.mit.sources();
    let mc = CLIENT.mc.sources();
    let kali = CLIENT.kali.sources();

    matches!(arch, DownloadSources::HttpSeeders { .. });
    matches!(mit, DownloadSources::Hybrid { .. });
    matches!(mc, DownloadSources::Trackers { .. });
    matches!(kali, DownloadSources::Trackers { .. });
}

#[test]
fn arch_source() {
    let arch = &CLIENT.arch;

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
    let mit = &CLIENT.mit;

    let sources = mit.sources();

    assert!(sources.trackers().is_some());

    let http_sources = sources.http_seeders().expect("This should be some");

    for s in http_sources {
        for u in &s.1 {
            assert!(u.contains(mit.meta_info().info().name()))
        }
    }
}

#[tokio::test]
async fn kali_source() {
    let kali = &CLIENT.kali;

    let mut list = kali
        .sources()
        .tracker_requests(kali.info_hash().as_encoded(), kali.peer_id())
        .unwrap();

    // Waits for ALL futures to complete
    while let Some(result) = list.next().await {
        let Ok(a) = result else { continue };
        if let Ok(a) = a {
            if a.is_http() {
                assert!(a
                    .to_url()
                    .unwrap()
                    .contains(&kali.info_hash().to_url_encoded()))
            } else if a.is_udp() {
                assert!(a.connection_id().is_some())
            }
        }
    }
}
