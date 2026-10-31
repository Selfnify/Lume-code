use super::*;

fn plain(lines: &[Line<'_>]) -> Vec<String> {
    lines
        .iter()
        .map(|line| line.spans.iter().map(|s| s.content.as_ref()).collect())
        .collect()
}

#[test]
fn paragraphs_lists_and_nested_inline_styles_stay_separate() {
    let lines = render("## What I can do\n\nFirst paragraph.\nSecond line.\n\n- **Coding** with *care*\n- **Bold and _italic_**\n  - Nested item\n\n3. Third\n4. Fourth\n\n> A quote\n\n- [x] Done\n- [ ] Next");
    let text = plain(&lines).join("\n");
    assert!(text.contains("First paragraph.\nSecond line.\n\n• Coding with care\n• Bold and italic\n  • Nested item"), "{text}");
    assert!(text.contains("3. Third\n4. Fourth"));
    assert!(text.contains("│ A quote"));
    assert!(text.contains("[✓] Done"));
    assert!(!text.contains("**"));
    let italic = lines
        .iter()
        .flat_map(|l| &l.spans)
        .find(|s| s.content == "italic")
        .unwrap();
    assert!(italic
        .style
        .add_modifier
        .contains(Modifier::BOLD | Modifier::ITALIC));
    let heading = lines
        .iter()
        .flat_map(|l| &l.spans)
        .find(|s| s.content == "What I can do")
        .unwrap();
    assert!(heading.style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn code_keeps_literal_markdown_indentation_and_blank_lines() {
    let lines = render("Use `main.rs`.\n\n```rust\nfn main() {\n\n\tprintln!(\"**literal**\");\n}\n```\n\n[Docs](https://example.com) and ~~old~~");
    let text = plain(&lines).join("\n");
    assert!(
        text.contains("fn main() {\n\n    println!(\"**literal**\");\n}"),
        "{text}"
    );
    assert!(text.contains("Docs (https://example.com)"));
    let code = lines
        .iter()
        .flat_map(|l| &l.spans)
        .find(|s| s.content == "main.rs")
        .unwrap();
    assert_eq!(code.style.bg, Some(CODE_BG));
    assert!(lines
        .iter()
        .flat_map(|l| &l.spans)
        .all(|s| !s.content.contains('\n')));
}

#[test]
fn streaming_fragments_and_unfinished_fences_never_drop_content_or_controls() {
    let text = "## Café 🦀\n\n**Hello**\n\n- First\n- Second\n\n```js\nconst x = '**text**';\n";
    for end in text
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(text.len()))
    {
        let lines = render(&text[..end]);
        assert!(lines
            .iter()
            .flat_map(|l| &l.spans)
            .all(|s| !s.content.contains('\n')));
    }
    let final_text = plain(&render(text)).join("\n");
    assert!(final_text.contains("const x = '**text**';"));
    let escaped = plain(&render("safe\u{1b}[31m\rtext")).join("\n");
    assert!(!escaped.contains('\u{1b}'));
    assert!(!escaped.contains('\r'));
}

#[test]
fn loose_list_keeps_its_marker_with_the_first_paragraph() {
    let text = plain(&render("- First item\n\n  More detail\n\n- Second item")).join("\n");
    assert!(text.contains("• First item"), "{text}");
    assert!(text.contains("  More detail"), "{text}");
    assert!(text.contains("• Second item"), "{text}");
}
