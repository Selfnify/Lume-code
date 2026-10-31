use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use serde_json::Value;
#[cfg(test)]
mod tests;
pub struct Picker {
    pub entries: Vec<Value>,
    pub query: String,
    pub selected: usize,
}
impl Picker {
    pub fn new(entries: Vec<Value>) -> Self {
        Self {
            entries,
            query: String::new(),
            selected: 0,
        }
    }
    pub fn filtered(&self) -> Vec<&Value> {
        let q = self.query.to_lowercase();
        self.entries
            .iter()
            .filter(|v| {
                format!(
                    "{} {}",
                    v["title"].as_str().unwrap_or(""),
                    v["root"].as_str().unwrap_or("")
                )
                .to_lowercase()
                .contains(&q)
            })
            .collect()
    }
    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Clear, area);
        let rows = self.filtered();
        let mut lines = vec![Line::raw(format!("Search: {}", self.query)), Line::raw("")];
        let count = (area.height.saturating_sub(5) / 2).max(1) as usize;
        let start = self.selected.saturating_sub(count - 1);
        for (i, v) in rows.iter().enumerate().skip(start).take(count) {
            let date = chrono::DateTime::from_timestamp(v["updated_at"].as_i64().unwrap_or(0), 0)
                .map(|v| {
                    v.with_timezone(&chrono::Local)
                        .format("%b %d %H:%M")
                        .to_string()
                })
                .unwrap_or_default();
            let style = if i == self.selected {
                Style::default()
                    .fg(Color::Rgb(35, 95, 190))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(crate::background::colors::cream_terracotta::TEXT)
            };
            lines.push(Line::styled(
                format!(
                    "{} {}",
                    if i == self.selected { "❯" } else { " " },
                    v["title"].as_str().unwrap_or("Untitled")
                ),
                style,
            ));
            lines.push(Line::styled(
                format!("  {date} · {}", v["root"].as_str().unwrap_or("")),
                style,
            ));
        }
        if rows.is_empty() {
            lines.push(Line::raw("No matching saved sessions."));
        }
        frame.render_widget(
            Paragraph::new(lines)
                .style(Style::default().bg(crate::background::colors::cream_terracotta::BG))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Resume · ↑↓ select · Enter open · Esc close "),
                ),
            area,
        );
    }
}
