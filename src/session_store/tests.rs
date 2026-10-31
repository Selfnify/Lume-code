use super::*;
#[test]
fn project_session_lock_releases_when_a_window_exits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.json");
    let first = Lock::acquire(&path).unwrap();
    assert!(Lock::acquire(&path).is_err());
    drop(first);
    assert!(Lock::acquire(&path).is_ok());
}
#[test]
fn crash_journals_restore_unicode_partial_results_once_and_reject_old_epochs() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("session.json");
    let epoch = json!(3);
    let mut history = vec![json!({"role":"user","content":"Build the app"})];
    crate::sessions::save(&p, &history, &None, &Value::Null, &epoch).unwrap();
    let run = json!({"id":"run_one","epoch":3,"status":"running","request":"Build the app","text":"Saved café 🦀","activities":[{"state":"tool_preparing","activity_id":"one","name":"create_file","at":0},{"state":"tool_done","activity_id":"one","name":"create_file","ok":true,"output":"saved main.js offset 240","at":0},{"state":"tool_preparing","activity_id":"two","name":"run_command","at":5}]});
    save_run(&p, &run).unwrap();
    restore(&p, &epoch, &mut history).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[1]["content"], "Saved café 🦀");
    restore(&p, &epoch, &mut history).unwrap();
    assert_eq!(history.len(), 2);
    assert!(recovery_text(&run)
        .unwrap()
        .contains("1 completed, 1 unconfirmed"));
    assert!(resume_request(&run).contains("offset 240"));
    let mut fresh = vec![];
    restore(&p, &json!(4), &mut fresh).unwrap();
    assert!(fresh.is_empty());
    let mut complete = run;
    complete["status"] = json!("complete");
    save_run(&p, &complete).unwrap();
    assert!(recovery_text(&complete).is_none());
    let mut recovered = vec![];
    restore(&p, &epoch, &mut recovered).unwrap();
    assert_eq!(
        recovered.len(),
        1,
        "A completed stream must survive a crash before the main save"
    );
}
#[test]
fn side_questions_archive_and_resume_without_entering_model_messages() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("main.json");
    let epoch = json!(0);
    crate::sessions::save(
        &p,
        &[json!({"role":"user","content":"Main task"})],
        &None,
        &Value::Null,
        &epoch,
    )
    .unwrap();
    let item = json!({"id":"side_1","question":"A side question?","answer":"A separate answer","status":"complete"});
    save_side(&p, &epoch, item.clone()).unwrap();
    save_side(&p, &epoch, item.clone()).unwrap();
    assert_eq!(extra(&p, &epoch)["items"].as_array().unwrap().len(), 1);
    assert!(!crate::sessions::load(&p)
        .unwrap()
        .to_string()
        .contains("A separate answer"));
    let id = crate::sessions::archive(&p).unwrap().unwrap();
    let saved = crate::sessions::resume(&p, &id).unwrap();
    clear(&p, &epoch).unwrap();
    assert_eq!(extra(&p, &epoch)["items"], json!([]));
    import(&p, &saved, &epoch).unwrap();
    assert_eq!(extra(&p, &epoch)["items"][0]["answer"], "A separate answer");
    assert_eq!(extra(&p, &json!(1))["items"], json!([]));
    assert!(side_display(&item).contains("A separate answer"));
    assert_eq!(side_answer("**Q**\n\nAnswer", "Q"), "Answer");
}
