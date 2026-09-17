#[test]
fn dbg_seed_validate() {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::config_slices::ConfigSlicesStore::new(dir.path().to_path_buf());
    let slices = store.load().unwrap();
    for item in &slices.outbounds.items {
        eprintln!("{} {} builtin={}", item.id, item.name, item.builtin);
    }
    eprintln!("validate: {:?}", slices.validate());
    panic!("debug");
}
