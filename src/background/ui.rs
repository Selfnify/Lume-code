use ratatui::{style::Style, widgets::Block, Frame};

use super::colors::cream_terracotta as theme;

pub fn draw(frame: &mut Frame) {
    let area = frame.area();
    frame.render_widget(Block::default().style(Style::default().bg(theme::BG)), area);
}
