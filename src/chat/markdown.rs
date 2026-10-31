//! Markdown is presentation only. Original conversation text stays unchanged.
use crate::background::colors::cream_terracotta as theme;
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
    links: Vec<String>,
}

impl Renderer {
    fn style(&self) -> Style {
        self.styles
            .last()
            .copied()
            .unwrap_or(Style::default().fg(theme::TEXT))
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
                        .push(self.style().fg(theme::ACCENT).add_modifier(Modifier::BOLD));
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
                    self.add(&marker, Style::default().fg(theme::ACCENT));
                    self.items.push(parent + marker.chars().count());
                }
                Tag::CodeBlock(kind) => {
                    self.gap();
                    let language = match kind {
                        CodeBlockKind::Fenced(v) => v.into_string(),
                        _ => String::new(),
                    };
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
                            .fg(theme::ACCENT)
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
            Event::Text(text) if self.code => {
                self.multiline(&text, Style::default().fg(theme::TEXT).bg(CODE_BG))
            }
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                self.multiline(&text, self.style())
            }
            Event::Code(text) => self.add(&text, self.style().fg(theme::ACCENT).bg(CODE_BG)),
            Event::SoftBreak | Event::HardBreak => self.newline(),
            Event::Rule => {
                self.gap();
                self.add("────────────────────────", Style::default().fg(MUTED));
                self.gap();
            }
            Event::TaskListMarker(done) => self.add(
                if done { "[✓] " } else { "[ ] " },
                Style::default().fg(theme::ACCENT),
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
