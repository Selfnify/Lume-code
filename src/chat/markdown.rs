//! Markdown is presentation only. Original conversation text stays unchanged.
use crate::background::colors::active as theme;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

const CODE_BG: Color = Color::Rgb(233, 225, 213);
const MUTED: Color = Color::Rgb(112, 98, 86);

#[derive(Default)]
struct Renderer {
    lines: Vec<Line<'static>>,
    line: Vec<Span<'static>>,
    styles: Vec<Style>,
    lists: Vec<Option<u64>>,
    items: Vec<usize>,
    quote: usize,
    code: bool,
    code_lang: String,
    code_buf: String,
    links: Vec<String>,
}

impl Renderer {
    fn style(&self) -> Style {
        self.styles
            .last()
            .copied()
            .unwrap_or(Style::default().fg(theme::text()))
    }
    fn prefix(&self) -> String {
        format!(
            "{}{}",
            "│ ".repeat(self.quote),
            " ".repeat(self.items.last().copied().unwrap_or(0))
        )
    }
    fn add(&mut self, text: &str, style: Style) {
        if text.is_empty() {
            return;
        }
        if self.line.is_empty() {
            self.line
                .push(Span::styled(self.prefix(), Style::default().fg(MUTED)));
        }
        // No terminal escapes, carriage returns or control characters from model text.
        let text: String = text
            .chars()
            .filter(|c| !c.is_control() || *c == '\t')
            .collect();
        self.line
            .push(Span::styled(text.replace('\t', "    "), style));
    }
    fn flush(&mut self) {
        if !self.line.is_empty() {
            self.lines.push(Line::from(std::mem::take(&mut self.line)));
        }
    }
    fn gap(&mut self) {
        self.flush();
        if self.lines.last().is_some_and(|line| !line.spans.is_empty()) {
            self.lines.push(Line::raw(""));
        }
    }
    fn newline(&mut self) {
        if self.line.is_empty() {
            self.lines.push(Line::raw(""));
        } else {
            self.flush();
        }
    }
    fn multiline(&mut self, text: &str, style: Style) {
        let mut parts = text.split('\n').peekable();
        while let Some(part) = parts.next() {
            self.add(part, style);
            if parts.peek().is_some() {
                self.newline();
            }
        }
    }
    fn decorated(&mut self, modifier: Modifier) {
        self.styles.push(self.style().add_modifier(modifier));
    }
    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {}
                Tag::Heading { .. } => {
                    self.gap();
                    self.styles
                        .push(self.style().fg(theme::accent()).add_modifier(Modifier::BOLD));
                }
                Tag::Strong => self.decorated(Modifier::BOLD),
                Tag::Emphasis => self.decorated(Modifier::ITALIC),
                Tag::Strikethrough => self.decorated(Modifier::CROSSED_OUT),
                Tag::BlockQuote(_) => {
                    self.gap();
                    self.quote += 1;
                }
                Tag::List(start) => {
                    self.flush();
                    self.lists.push(start);
                }
                Tag::Item => {
                    self.flush();
                    let marker = match self.lists.last_mut() {
                        Some(Some(n)) => {
                            let s = format!("{n}. ");
                            *n = n.saturating_add(1);
                            s
                        }
                        _ => "• ".into(),
                    };
                    let parent = self.items.last().copied().unwrap_or(0);
                    self.add(&marker, Style::default().fg(theme::accent()));
                    self.items.push(parent + marker.chars().count());
                }
                Tag::CodeBlock(kind) => {
                    self.gap();
                    let language = match kind {
                        CodeBlockKind::Fenced(v) => v.into_string(),
                        _ => String::new(),
                    };
                    self.code_lang = language.clone();
                    self.add(
                        &format!(
                            "┌─ {}",
                            if language.is_empty() {
                                "code"
                            } else {
                                &language
                            }
                        ),
                        Style::default().fg(MUTED),
                    );
                    self.flush();
                    self.code = true;
                }
                Tag::Link { dest_url, .. } => {
                    self.links.push(dest_url.into_string());
                    self.styles.push(
                        self.style()
                            .fg(theme::accent())
                            .add_modifier(Modifier::UNDERLINED),
                    );
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Strong | TagEnd::Emphasis | TagEnd::Strikethrough => {
                    self.styles.pop();
                }
                TagEnd::Heading(_) => {
                    self.styles.pop();
                    self.gap();
                }
                TagEnd::Paragraph => {
                    if self.items.is_empty() {
                        self.gap();
                    } else {
                        self.flush();
                    }
                }
                TagEnd::Item => {
                    self.flush();
                    self.items.pop();
                }
                TagEnd::List(_) => {
                    self.flush();
                    self.lists.pop();
                    if self.lists.is_empty() {
                        self.gap();
                    }
                }
                TagEnd::BlockQuote(_) => {
                    self.flush();
                    self.quote = self.quote.saturating_sub(1);
                    self.gap();
                }
                TagEnd::CodeBlock => {
                    self.flush();
                    self.code = false;
                    let code = std::mem::take(&mut self.code_buf);
                    let lang = std::mem::take(&mut self.code_lang);
                    for spans in highlight(&code, &lang) {
                        if spans.iter().all(|(text, _)| text.is_empty()) {
                            self.newline();
                            continue;
                        }
                        for (text, color) in spans {
                            self.add(&text, Style::default().fg(color).bg(CODE_BG));
                        }
                        self.flush();
                    }
                    self.add("└─", Style::default().fg(MUTED));
                    self.gap();
                }
                TagEnd::Link => {
                    self.styles.pop();
                    if let Some(url) = self.links.pop() {
                        self.add(&format!(" ({url})"), Style::default().fg(MUTED));
                    }
                }
                _ => {}
            },
            Event::Text(text) if self.code => self.code_buf.push_str(&text),
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                self.multiline(&text, self.style())
            }
            Event::Code(text) => self.add(&text, self.style().fg(theme::accent()).bg(CODE_BG)),
            Event::SoftBreak | Event::HardBreak => self.newline(),
            Event::Rule => {
                self.gap();
                self.add("────────────────────────", Style::default().fg(MUTED));
                self.gap();
            }
            Event::TaskListMarker(done) => self.add(
                if done { "[✓] " } else { "[ ] " },
                Style::default().fg(theme::accent()),
            ),
            _ => {}
        }
    }
}

pub fn render(text: &str) -> Vec<Line<'static>> {
    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut renderer = Renderer::default();
    for event in Parser::new_ext(text, options) {
        renderer.event(event);
    }
    renderer.flush();
    while renderer
        .lines
        .last()
        .is_some_and(|line| line.spans.is_empty())
    {
        renderer.lines.pop();
    }
    renderer.lines
}

#[cfg(test)]
mod tests;

fn highlight(code: &str, lang: &str) -> Vec<Vec<(String, Color)>> {
    use std::sync::OnceLock;
    use syntect::{
        easy::HighlightLines,
        highlighting::ThemeSet,
        parsing::SyntaxSet,
        util::LinesWithEndings,
    };
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    static THEMES: OnceLock<ThemeSet> = OnceLock::new();
    let syntaxes = SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines);
    let themes = THEMES.get_or_init(ThemeSet::load_defaults);
    let theme = &themes.themes["InspiredGitHub"];
    let syntax = syntaxes
        .find_syntax_by_token(lang)
        .unwrap_or_else(|| syntaxes.find_syntax_plain_text());
    let mut highlighter = HighlightLines::new(syntax, theme);
    LinesWithEndings::from(code)
        .map(|line| {
            match highlighter.highlight_line(line, syntaxes) {
                Ok(ranges) => ranges
                    .into_iter()
                    .map(|(style, text)| {
                        let c = style.foreground;
                        (
                            text.trim_end_matches(['\r', '\n']).to_string(),
                            Color::Rgb(c.r, c.g, c.b),
                        )
                    })
                    .collect(),
                Err(_) => vec![(line.trim_end_matches(['\r', '\n']).to_string(), theme::text())],
            }
        })
        .collect()
}