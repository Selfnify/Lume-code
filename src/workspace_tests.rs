use super::*;
use serde_json::json;
fn task(id: &str) -> Task {
    Task {
        id: id.into(),
        name: "create_file".into(),
        label: "Writing file".into(),
        done: false,
        failed: false,
        detail: "LIVE GENERATED CONTENT".into(),
    }
}
#[test]
fn inline_tools_preserve_message_boundaries_and_click_targets_after_wrapping() {
    let mut textbox = TextBox::new();
    let mut messages = vec![Message::User("Create an app".into())];
    let mut pending = Some(Pending {
        text: "Before the first tool. ".repeat(10),
        started: Instant::now(),
        word: "Working",
    });
    let mut tasks = vec![task("one")];
    sync_timeline(&mut textbox, &mut messages, &mut pending, &tasks);
    assert!(matches!(messages[1], Message::Ai { .. }));
    assert!(matches!(messages[2], Message::Tools(0)));
    assert!(pending.as_ref().unwrap().text.is_empty());
    tasks[0].done = true;
    sync_timeline(&mut textbox, &mut messages, &mut pending, &tasks);
    pending.as_mut().unwrap().text = "Now a second step.".into();
    tasks.push(task("two"));
    sync_timeline(&mut textbox, &mut messages, &mut pending, &tasks);
    assert!(matches!(messages[3], Message::Ai { .. }));
    assert!(matches!(messages[4], Message::Tools(1)));
    assert_eq!(textbox.timeline.groups.len(), 2);
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 32)).unwrap();
    let mut info = None;
    terminal
        .draw(|f| {
            info = Some(draw_screen(
                f,
                &textbox,
                &messages,
                pending.as_ref(),
                "✢",
                false,
                0,
                &tasks,
                false,
                None,
            ))
        })
        .unwrap();
    let info = info.unwrap();
    let (rect, id) = info
        .hits
        .iter()
        .find_map(|(r, h)| {
            if let Hit::Group(i) = h {
                Some((*r, *i))
            } else {
                None
            }
        })
        .unwrap();
    assert!(matches!(
        terminal.backend().buffer()[(rect.x, rect.y)].symbol(),
        "❯" | "⌄"
    ));
    textbox.timeline.groups[id].open = true;
    terminal
        .draw(|f| {
            draw_screen(
                f,
                &textbox,
                &messages,
                pending.as_ref(),
                "✢",
                false,
                usize::MAX,
                &tasks,
                false,
                None,
            );
        })
        .unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(text.contains("LIVE GENERATED CONTENT"));
    assert!(!text.contains("Tasker (Ctrl-T"));
    textbox.timeline.latest();
    assert!(textbox.timeline.groups.last().unwrap().open);
}
#[test]
fn restored_tool_groups_split_at_saved_text_offsets() {
    let mut messages = vec![];
    let mut textbox = TextBox::new();
    let mut tasks = vec![];
    restore_reply(
        &mut messages,
        &mut textbox,
        &mut tasks,
        1,
        "First\n\nSecond",
        &json!([{"state":"tool_preparing","name":"read_file","activity_id":"one","at":5},{"state":"tool_done","name":"read_file","activity_id":"one","ok":true,"at":5},{"state":"tool_preparing","name":"web_search","activity_id":"two","at":13},{"state":"tool_done","name":"web_search","activity_id":"two","ok":true,"at":13}]),
    );
    assert_eq!(textbox.timeline.groups.len(), 2);
    assert_eq!(tasks.len(), 2);
    assert!(matches!(messages[0], Message::Ai { .. }));
    assert!(matches!(messages[1], Message::Tools(0)));
    assert!(matches!(messages[2], Message::Ai { .. }));
    assert!(matches!(messages[3], Message::Tools(1)));
    assert!(textbox.timeline.groups.iter().all(|g| !g.open));
}
#[test]
fn usage_recovery_and_folder_access_stay_above_the_textbox_on_small_terminals() {
    let mut textbox = TextBox::new();
    textbox.usage = json!({"five_hour":{"used":30,"limit":30,"resets_at":session_store::now()+3600},"weekly":{"used":50,"limit":150,"resets_at":session_store::now()+86400}});
    textbox.state = json!({"directories":["C:/Projects/helpers"],"recovery":{"status":"interrupted","activities":[]}});
    for width in [48, 120] {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, 32)).unwrap();
        terminal
            .draw(|f| {
                draw_screen(f, &textbox, &[], None, "✢", false, 0, &[], false, None);
            })
            .unwrap();
        let rows = terminal
            .backend()
            .buffer()
            .content
            .chunks(width as usize)
            .map(|r| r.iter().map(|c| c.symbol()).collect::<String>())
            .collect::<Vec<_>>();
        let input = rows
            .iter()
            .position(|r| r.contains("Ask lume anything"))
            .unwrap();
        assert!(rows[..input]
            .iter()
            .any(|r| r.contains("Usage limit reached")));
        assert!(rows[..input].iter().any(|r| r.contains("Ctrl-R")));
        assert!(rows[..input].iter().any(|r| r.contains("dir1=helpers")));
    }
}
