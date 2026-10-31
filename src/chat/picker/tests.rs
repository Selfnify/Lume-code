use super::*;
use serde_json::json;
#[test]
fn session_picker_searches_titles_and_folders_and_submits_selection() {
    let entries = vec![
        json!({"id":"first","title":"First app","root":"C:/Projects/one","updated_at":1700000000}),
        json!({"id":"second","title":"Second app","root":"C:/Projects/two","updated_at":1700000010}),
    ];
    let mut box_ = crate::chat::textbox::textbox::TextBox::new();
    box_.picker = Some(Picker::new(entries));
    let key = |code| crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE);
    for c in "two".chars() {
        box_.handle_key(key(crossterm::event::KeyCode::Char(c)));
    }
    assert_eq!(box_.picker.as_ref().unwrap().filtered().len(), 1);
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 12)).unwrap();
    terminal
        .draw(|f| box_.picker.as_ref().unwrap().draw(f, f.area()))
        .unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(text.contains("Second app"));
    assert!(text.contains("C:/Projects/two"));
    assert_eq!(
        box_.handle_key(key(crossterm::event::KeyCode::Enter))
            .as_deref(),
        Some("/resume second")
    );
    assert!(box_.picker.is_none());
}
