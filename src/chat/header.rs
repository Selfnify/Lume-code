use std::env;

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::background::colors::active as theme;

const LOGO_W: usize = 13;
const LOGO_H: usize = 10;
const LOGO_BITMAP: [&str; LOGO_H] = [
    "1111111111111",
    "1000000000001",
    "1001100011001",
    "1001100011001",
    "1000000000001",
    "1010000000101",
    "1001111111001",
    "1000000000001",
    "1111111111111",
    "0000000000000",
];
const LOGO_COL_W: u16 = (LOGO_W + 2) as u16;
const MIN_TEXT_W: u16 = 24;

pub fn height(width: u16) -> u16 {
    if width >= LOGO_COL_W + 1 + MIN_TEXT_W + 2 {
        (LOGO_H / 2) as u16 + 2
    } else {
        3 + 2
    }
}

const BOX_MAX_W: u16 = 60;

pub fn draw(frame: &mut Frame, area: Rect) {
    let area = Rect::new(area.x, area.y, area.width.min(BOX_MAX_W), area.height);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::accent()))
        .style(Style::default().bg(theme::bg()));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let cwd = display_directory(
        &env::current_dir()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|_| String::from("?")),
    );
    let show_logo = area.width >= LOGO_COL_W + 1 + MIN_TEXT_W + 2;

    if show_logo {
        let cols = Layout::horizontal([
            Constraint::Length(LOGO_COL_W),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(inner);
        let logo: Vec<Line> = logo_rows()
            .into_iter()
            .map(|row| {
                Line::from(vec![
                    Span::raw(" "),
                    Span::styled(row, Style::default().fg(theme::accent())),
                ])
            })
            .collect();
        frame.render_widget(
            Paragraph::new(logo).style(Style::default().bg(theme::bg())),
            cols[0],
        );
        let divider: Vec<Line> = (0..cols[1].height)
            .map(|_| Line::styled("│", Style::default().fg(theme::accent())))
            .collect();
        frame.render_widget(
            Paragraph::new(divider).style(Style::default().bg(theme::bg())),
            cols[1],
        );
        let text_area = Rect::new(cols[2].x + 1, cols[2].y, cols[2].width.saturating_sub(1), cols[2].height);
        let width = text_area.width as usize;
        let info = vec![
            title_line(),
            Line::styled(truncate_left(&cwd, width), Style::default().fg(theme::text())),
            Line::raw(""),
            hint_line(width),
        ];
        frame.render_widget(
            Paragraph::new(info).style(Style::default().bg(theme::bg())),
            text_area,
        );
    } else {
        let text_area = Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(1), inner.height);
        let width = text_area.width as usize;
        let info = vec![
            title_line(),
            Line::styled(truncate_left(&cwd, width), Style::default().fg(theme::text())),
            hint_line(width),
        ];
        frame.render_widget(
            Paragraph::new(info).style(Style::default().bg(theme::bg())),
            text_area,
        );
    }
}

fn title_line() -> Line<'static> {
    Line::from(vec![
        Span::styled(
            ">_ ",
            Style::default()
                .fg(theme::accent())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Lume Code",
            Style::default()
                .fg(theme::text())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" (v{})", env!("CARGO_PKG_VERSION")),
            Style::default()
                .fg(theme::text())
                .add_modifier(Modifier::DIM),
        ),
    ])
}

fn hint_line(width: usize) -> Line<'static> {
    Line::styled(
        truncate_left("/ for commands · ? for shortcuts", width),
        Style::default()
            .fg(theme::text())
            .add_modifier(Modifier::DIM),
    )
}

fn logo_rows() -> Vec<String> {
    let on = |py: usize, px: usize| LOGO_BITMAP[py].as_bytes()[px] == b'1';
    (0..LOGO_H / 2)
        .map(|r| {
            (0..LOGO_W)
                .map(|c| match (on(r * 2, c), on(r * 2 + 1, c)) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    _ => ' ',
                })
                .collect()
        })
        .collect()
}

fn display_directory(path: &str) -> String {
    let home = env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .unwrap_or_default();
    if !home.is_empty() {
        if let Some(rest) = path.strip_prefix(&home) {
            return if rest.is_empty() {
                "~".to_string()
            } else {
                format!("~{rest}")
            };
        }
    }
    path.to_string()
}

fn truncate_left(text: &str, width: usize) -> String {
    let count = text.chars().count();
    if width == 0 {
        return String::new();
    }
    if count <= width {
        return text.to_string();
    }
    let keep = width.saturating_sub(1);
    let tail: String = text.chars().skip(count - keep).collect();
    format!("…{tail}")
}