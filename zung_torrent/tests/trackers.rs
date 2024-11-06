use utilities::torrent::TestClient;

#[test]
fn tracker_request_url() {
    let clients = TestClient::new();
    let arch = clients.arch.tracker_request().unwrap().to_url().unwrap();
    let mit = clients.mit.tracker_request().unwrap().to_url().unwrap();
    let mc = clients.mc.tracker_request().unwrap().to_url().unwrap();
    let kali = clients.kali.tracker_request().unwrap().to_url().unwrap();

    // TODO: Test for urls as well when generation procedure is final.

    assert!(arch.contains(&clients.arch.info_hash().to_url_encoded()));
    assert!(mit.contains(&clients.mit.info_hash().to_url_encoded()));
    assert!(mc.contains(&clients.mc.info_hash().to_url_encoded()));
    assert!(kali.contains(&clients.kali.info_hash().to_url_encoded()));

    assert!(arch.contains(&clients.arch.peer_id().to_url_encoded()));
    assert!(mit.contains(&clients.mit.peer_id().to_url_encoded()));
    assert!(mc.contains(&clients.mc.peer_id().to_url_encoded()));
    assert!(kali.contains(&clients.kali.peer_id().to_url_encoded()));
}
