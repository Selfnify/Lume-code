use super::*;
use crate::chat::textbox::textbox::TextBox;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}
#[test]
fn picker_filters_scrolls_inserts_and_closes() {
    let mut box_ = TextBox::new();
    box_.handle_key(key(KeyCode::Char('/')));
    assert_eq!(box_.commands().len(), COMMANDS.len());
    for _ in 0..9 {
        box_.handle_key(key(KeyCode::Down));
    }
    assert_eq!(box_.commands()[box_.selected].name, "/usage");
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(90, 5)).unwrap();
    terminal
        .draw(|frame| {
            draw(frame, frame.area(), &box_.commands(), box_.selected);
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let rows = buffer
        .content
        .chunks(90)
        .map(|r| r.iter().map(|c| c.symbol()).collect::<String>())
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 5);
    assert!(rows[4].contains("❯ /usage"));
    assert!(!rows.join("").contains("/add-dir"));
    assert_eq!(buffer[(1, 4)].fg, Color::Rgb(35, 95, 190));
    assert!(box_.handle_key(key(KeyCode::Enter)).is_none());
    assert_eq!(box_.input(), "/usage");
    assert_eq!(
        box_.handle_key(key(KeyCode::Enter)).as_deref(),
        Some("/usage")
    );
    for c in "/comp".chars() {
        box_.handle_key(key(KeyCode::Char(c)));
    }
    assert_eq!(box_.commands().len(), 1);
    assert_eq!(box_.commands()[0].name, "/compact");
    assert!(box_.close_overlay());
    assert!(box_.commands().is_empty());
}
#[test]
fn question_mark_is_a_real_shortcut_and_normal_question_text_is_preserved() {
    let mut b = TextBox::new();
    b.handle_key(key(KeyCode::Char('?')));
    assert!(b.shortcuts);
    assert_eq!(b.input(), "");
    assert!(b.close_overlay());
    for c in "Why?".chars() {
        b.handle_key(key(KeyCode::Char(c)));
    }
    assert!(!b.shortcuts);
    assert_eq!(b.input(), "Why?");
}
