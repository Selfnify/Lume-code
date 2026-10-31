use super::*;
#[test]
fn completions_only_offer_matching_folders_and_quote_spaces() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("Child folder")).unwrap();
    std::fs::create_dir(dir.path().join("Elsewhere")).unwrap();
    std::fs::write(dir.path().join("Child.txt"), "not a directory").unwrap();
    let rows = complete("/cd Ch", dir.path());
    assert_eq!(rows.len(), 1);
    assert!(rows[0].contains("\"Child folder"));
    assert!(rows[0].ends_with('"'));
    assert!(complete("/usage Ch", dir.path()).is_empty());
    let mut textbox = crate::chat::textbox::textbox::TextBox::new();
    let input = format!(
        "/add-dir \"{}{}Ch",
        dir.path().display(),
        std::path::MAIN_SEPARATOR
    );
    for c in input.chars() {
        textbox.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char(c),
            crossterm::event::KeyModifiers::NONE,
        ));
    }
    textbox.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Tab,
        crossterm::event::KeyModifiers::NONE,
    ));
    assert!(textbox.input().contains("Child folder"));
    let command = textbox
        .handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ))
        .unwrap();
    assert!(command.starts_with("/add-dir \""));
}
