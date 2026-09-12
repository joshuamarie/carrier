use carrier_native::cache::clear_module_cache;

#[test]
fn clear_module_cache_behavior() {
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("CARRIER_CACHE_DIR", tmp.path());

    // Removes an existing module's cache entries, all triples/versions/hashes included.
    let module_dir = tmp.path().join("mystats").join("x86_64-pc-linux-gnu").join("4.4");
    std::fs::create_dir_all(&module_dir).unwrap();
    std::fs::write(module_dir.join("mystats.so"), b"fake artifact").unwrap();

    clear_module_cache("mystats").unwrap();
    assert!(!tmp.path().join("mystats").exists());

    // A module with nothing cached yet is a no-op, not an error.
    let result = clear_module_cache("never_built");
    assert!(result.is_ok());

    // Only the named module is touched, siblings are left alone.
    std::fs::create_dir_all(tmp.path().join("mystats")).unwrap();
    std::fs::create_dir_all(tmp.path().join("otherstats")).unwrap();

    clear_module_cache("mystats").unwrap();
    assert!(!tmp.path().join("mystats").exists());
    assert!(tmp.path().join("otherstats").exists());

    std::env::remove_var("CARRIER_CACHE_DIR");
}
