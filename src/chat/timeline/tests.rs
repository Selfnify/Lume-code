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
        assert!(text.contains("✓ Date and time checked"));
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
        id: id.into(),
        name: "read_file".into(),
        label: if done { "File read" } else { "Reading file" }.into(),
        done,
        failed: false,
        detail: "verified contents".into(),
    }
}
#[test]
fn groups_start_immediately_keep_all_tools_and_only_expand_manually() {
    let mut timeline = Timeline::default();
    let mut tasks = vec![task("one", false)];
    assert_eq!(timeline.sync(&tasks), Some(0));
    assert!(!timeline.groups[0].open);
    let mut lines = vec![];
    timeline.render(0, &mut lines);
    assert!(lines[0].spans.len() > 1, "Running headers must shimmer");
    assert!(!lines
        .iter()
        .any(|l| l.to_string().contains("verified contents")));
    tasks[0].done = true;
    tasks.push(task("two", true));
    timeline.sync(&tasks);
    assert_eq!(timeline.groups[0].tasks.len(), 2);
    timeline.current = None;
    tasks.push(task("three", false));
    assert_eq!(timeline.sync(&tasks), Some(1));
    timeline.latest();
    assert!(!timeline.groups[0].open);
    assert!(timeline.groups[1].open);
    timeline.groups[1].settled = true;
    let mut lines = vec![];
    timeline.render(1, &mut lines);
    assert_eq!(
        lines[0].spans.len(),
        1,
        "Interrupted headers must stop shimmering"
    );
    assert!(lines
        .iter()
        .any(|l| l.to_string().contains("verified contents")));
    assert!(lines
        .iter()
        .any(|l| l.to_string().contains("? Reading file")));
    timeline.latest();
    assert!(!timeline.groups[1].open);
}
