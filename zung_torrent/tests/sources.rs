use utilities::torrent::CLIENT;

#[test]
fn source_types() {
    let arch = CLIENT.arch.sources();
    let mit = CLIENT.mit.sources();
    let mc = CLIENT.mc.sources();
    let kali = CLIENT.kali.sources();

    assert!(arch.is_http_seeders());
    assert!(mit.is_hybrid());
    assert!(mc.is_trackers());
    assert!(kali.is_hybrid());
}

#[test]
fn arch_source() {
    let arch = &CLIENT.arch;

    let sources = arch.sources();
    let sources = sources.http_seeders().expect("This should be some");

    for s in sources.iter() {
        for u in &s.1 {
            assert!(u.contains(arch.meta_info().info().name()))
        }
    }
}

#[test]
fn mit_source() {
    let mit = &CLIENT.mit;

    let sources = mit.sources();

    assert!(sources.tracker_list().is_some());

    let http_sources = sources.http_seeders().expect("This should be some");

    for s in http_sources.iter() {
        for u in &s.1 {
            assert!(u.contains(mit.meta_info().info().name()))
        }
    }
}
