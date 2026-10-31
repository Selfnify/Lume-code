use super::*;
#[test]
fn picker_catalog_is_account_backend_epoch_scoped_and_includes_other_folders() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let root = first.path().canonicalize().unwrap();
    let other = second.path().canonicalize().unwrap();
    let email = format!(
        "catalog-{}-{}@test.invalid",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap()
    );
    let api = "http://localhost:1";
    let active = sessions::location(api, &email, &root).unwrap();
    let previous = sessions::location(api, &email, &other).unwrap();
    struct Cleanup(Vec<PathBuf>);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            for path in &self.0 {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    let _cleanup = Cleanup(vec![
        active.clone(),
        previous.clone(),
        index(api, &email).unwrap(),
    ]);
    sessions::save(
        &active,
        &[json!({"role":"user","content":"Current task"})],
        &None,
        &Value::Null,
        &json!(0),
    )
    .unwrap();
    sessions::save(
        &previous,
        &[json!({"role":"user","content":"Build a second app"})],
        &None,
        &Value::Null,
        &json!(0),
    )
    .unwrap();
    register(api, &email, &root).unwrap();
    register(api, &email, &other).unwrap();
    let entries = list(api, &email, &root, &json!(0)).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["title"], "Build a second app");
    assert_eq!(entries[0]["root"], json!(other));
    assert!(entries[0]["updated_at"].as_i64().unwrap() > 0);
    assert!(list(api, &email, &root, &json!(1)).unwrap().is_empty());
    assert!(list(api, "different@test.invalid", &root, &json!(0))
        .unwrap()
        .is_empty());
    assert!(list("http://localhost:2", &email, &root, &json!(0))
        .unwrap()
        .is_empty());
}
