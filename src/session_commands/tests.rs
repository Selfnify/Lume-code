use super::*;
#[test]
fn clear_resume_and_directory_switch_preserve_history_and_scope() {
    let dir = tempfile::tempdir().unwrap();
    let next = tempfile::tempdir().unwrap();
    let extra = tempfile::tempdir().unwrap();
    let c = Connection {
        http: reqwest::Client::new(),
        api: "http://localhost:1".into(),
        token: "synthetic".into(),
    };
    let mut settings = Settings::new(dir.path().canonicalize().unwrap());
    let mut path = Some(dir.path().join("fixture.json"));
    let mut history = vec![json!({"role":"user","content":"Keep this task"})];
    let mut client = None;
    let mut checkpoint = Value::Null;
    let (tx, _rx) = std::sync::mpsc::channel();
    let mut run = |text: &str, s: &mut Settings, h: &mut Vec<Value>| {
        super::run(
            text,
            s,
            &c,
            "fixture@test.invalid",
            &mut path,
            h,
            &mut client,
            &mut checkpoint,
            &json!(0),
            &tx,
        )
    };
    run("/clear", &mut settings, &mut history).unwrap();
    assert!(history.is_empty());
    run("/resume latest", &mut settings, &mut history).unwrap();
    assert_eq!(history[0]["content"], "Keep this task");
    run(
        &format!("/add-dir {}", extra.path().display()),
        &mut settings,
        &mut history,
    )
    .unwrap();
    assert_eq!(settings.directories.len(), 1);
    assert!(run("/autocompact 100", &mut settings, &mut history).is_err());
    run("/autocompact 60%", &mut settings, &mut history).unwrap();
    assert_eq!(settings.threshold, 60);
    run("/effort high", &mut settings, &mut history).unwrap();
    assert_eq!(settings.effort, "high");
    run(
        &format!("/cd {}", next.path().display()),
        &mut settings,
        &mut history,
    )
    .unwrap();
    drop(run);
    assert_eq!(settings.root, next.path().canonicalize().unwrap());
    assert_eq!(history[0]["content"], "Keep this task");
    assert_eq!(
        path,
        Some(sessions::location(&c.api, "fixture@test.invalid", &settings.root).unwrap())
    );
    assert!(sessions::resume(&dir.path().join("other-account.json"), "latest").is_err());
    assert!(sessions::resume(&dir.path().join("fixture.json"), "../../secret").is_err());
    if let Some(path) = path {
        settings.lock = None;
        for extension in ["side.json", "run.json", "lock"] {
            let _ = std::fs::remove_file(path.with_extension(extension));
        }
    }
}

#[test]
fn archived_settings_restore_and_stale_epochs_do_not_replace_active_history() {
    let dir = tempfile::tempdir().unwrap();
    let extra = tempfile::tempdir().unwrap();
    let p = dir.path().join("active.json");
    let mut settings = Settings::new(dir.path().canonicalize().unwrap());
    settings.effort = "high".into();
    settings.threshold = 55;
    settings
        .directories
        .push(extra.path().canonicalize().unwrap());
    let history = vec![json!({"role":"user","content":"Saved request"})];
    sessions::save(
        &p,
        &history,
        &Some(json!({"id":"fixture_binding"})),
        &Value::Null,
        &json!(1),
    )
    .unwrap();
    sessions::save_options(&p, &settings.persisted()).unwrap();
    let id = sessions::archive(&p).unwrap().unwrap();
    let archived = sessions::resume(&p, &id).unwrap();
    let mut restored = Settings::new(settings.root.clone());
    restored.restore(&archived["settings"]);
    assert_eq!(restored.effort, "high");
    assert_eq!(restored.threshold, 55);
    assert_eq!(restored.directories, settings.directories);
    let c = Connection {
        http: reqwest::Client::new(),
        api: "http://localhost:1".into(),
        token: "synthetic".into(),
    };
    let mut current = vec![json!({"role":"user","content":"Current request"})];
    let (tx, _) = std::sync::mpsc::channel();
    assert!(super::run(
        &format!("/resume {id}"),
        &mut restored,
        &c,
        "fixture@test.invalid",
        &mut Some(p),
        &mut current,
        &mut None,
        &mut Value::Null,
        &json!(0),
        &tx
    )
    .is_err());
    assert_eq!(current[0]["content"], "Current request");
}
#[tokio::test]
async fn extra_directory_tools_are_explicit_and_cannot_escape() {
    let primary = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    std::fs::write(other.path().join("hello.txt"), "extra directory").unwrap();
    let root = primary.path().canonicalize().unwrap();
    let dirs = vec![other.path().canonicalize().unwrap()];
    let value = crate::local::execute_in(
        &root,
        &dirs,
        "read_file",
        &json!({"directory":"dir1","path":"hello.txt"}),
    )
    .await;
    assert!(value.to_string().contains("extra directory"));
    for directory in ["dir0", "dir2", "C:/Windows"] {
        assert!(crate::local::execute_in(
            &root,
            &dirs,
            "read_file",
            &json!({"directory":directory,"path":"hello.txt"})
        )
        .await["error"]
            .is_string());
    }
    assert!(crate::local::execute_in(
        &root,
        &dirs,
        "read_file",
        &json!({"directory":"dir1","path":"../secret"})
    )
    .await["error"]
        .is_string());
}
