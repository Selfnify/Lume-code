use super::*;
#[test]
fn clock_activity_shows_status_without_raw_output_in_live_or_saved_groups() {
    for restored in [false, true] {
        let mut timeline = Timeline::default();
        let mut clock = task("clock", true);
        clock.name = "get_datetime".into();
        clock.label = "Date and time checked".into();
        clock.detail =
            r#"{"datetime":"2026-10-29T18:36:54Z","timezone":"GMT+00:00","ok":true}"#.into();
        timeline.sync(&[clock, task("file", true)]);
        timeline.groups[0].open = true;
        timeline.groups[0].restored = restored;
        let mut lines = vec![];
        timeline.render(0, &mut lines);
        let text = lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Ran a tool"));
        assert!(text.contains("└ "));
        assert!(!text.contains("datetime"));
        assert!(!text.contains("GMT+00:00"));
        assert!(text.contains("verified contents"));
    }
}
#[test]
fn live_file_preview_keeps_the_newest_lines_visible() {
    let mut timeline = Timeline::default();
    let mut t = task("source", false);
    t.detail = (0..40)
        .map(|i| format!("source_line_{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    timeline.sync(&[t]);
    timeline.groups[0].open = true;
    let mut lines = vec![];
    timeline.render(0, &mut lines);
    let text = lines
        .iter()
        .map(|l| l.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("source_line_39"));
    assert!(!text.contains("source_line_0\n"));
}
fn task(id: &str, done: bool) -> Task {
    Task {
        command: String::new(),
        id: id.into(),
        name: "read_file".into(),
        label: if done { "File read" } else { "Reading file" }.into(),
        done,
        failed: false,
        detail: "verified contents".into(),
    }
}
#[test]
fn commands_render_as_ungrouped_rows_that_pulse_then_turn_green() {
    let mut timeline = Timeline::default();
    let mut command = task("one", false);
    command.name = "run_command".into();
    command.command = "cargo test".into();
    command.detail = "running tests".into();
    let mut tasks = vec![command];
    assert_eq!(timeline.sync(&tasks), Some(0));
    let mut lines = vec![];
    timeline.render(0, &mut lines);
    assert_eq!(lines[0].to_string(), "● Running cargo test");
    assert!(matches!(
        lines[0].spans[0].style.fg,
        Some(Color::White) | Some(Color::DarkGray)
    ));
    assert!(lines.iter().any(|l| l.to_string().contains("└ running tests")));
    tasks[0].done = true;
    tasks[0].detail = "ok".into();
    timeline.sync(&tasks);
    let mut lines = vec![];
    timeline.render(0, &mut lines);
    assert_eq!(lines[0].spans[0].style.fg, Some(Color::Green));
    assert!(lines.iter().any(|l| l.to_string().contains("└ ok")));
}
#[test]
fn command_rows_show_stdout_instead_of_raw_json() {
    let mut timeline = Timeline::default();
    let mut command = task("one", true);
    command.name = "run_command".into();
    command.command = "echo hi".into();
    command.detail = r#"{"exit_code":0,"output_limit":65536,"stderr":"","stdout":"hi\r\n"}"#.into();
    timeline.sync(&[command]);
    let mut lines = vec![];
    timeline.render(0, &mut lines);
    let text = lines.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n");
    assert!(text.contains("└ hi"));
    assert!(!text.contains("output_limit"));
}