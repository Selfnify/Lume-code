use std::env;

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::background::colors::cream_terracotta as theme;

const DOTS_W: usize = 20;
const DOTS_H: usize = 16;
const LOGO_X0: f64 = 24.0;
const LOGO_X1: f64 = 76.0;
const LOGO_Y0: f64 = 28.0;
const LOGO_Y1: f64 = 68.0;
const LOGO_PAD: &str = "   ";

const BRAILLE_BITS: [[u32; 4]; 2] = [[0x01, 0x02, 0x04, 0x40], [0x08, 0x10, 0x20, 0x80]];

pub fn draw(frame: &mut Frame, area: Rect) {
    let cwd = env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| String::from("?"));

    let logo = logo_rows();
    let lines: Vec<Line> = logo
        .into_iter()
        .enumerate()
        .map(|(i, row)| {
            let info = match i {
                0 => Span::styled(
                    "Lume Code",
                    Style::default()
                        .fg(theme::ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                1 => Span::styled(cwd.clone(), Style::default().fg(theme::TEXT)),
                _ => Span::raw(""),
            };
            Line::from(vec![
                Span::raw(LOGO_PAD),
                Span::styled(row, Style::default().fg(theme::ACCENT)),
                Span::raw("  "),
                info,
            ])
        })
        .collect();

    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(theme::BG)),
        area,
    );
}

fn logo_rows() -> Vec<String> {
    let mut dots = [[false; DOTS_W]; DOTS_H];
    for (py, row) in dots.iter_mut().enumerate() {
        for (px, dot) in row.iter_mut().enumerate() {
            let x = LOGO_X0 + (px as f64 + 0.5) * (LOGO_X1 - LOGO_X0) / DOTS_W as f64;
            let y = LOGO_Y0 + (py as f64 + 0.5) * (LOGO_Y1 - LOGO_Y0) / DOTS_H as f64;
            *dot = in_logo(x, y);
        }
    }

    (0..DOTS_H / 4)
        .map(|cy| {
            (0..DOTS_W / 2)
                .map(|cx| {
                    let mut bits = 0u32;
                    for dx in 0..2 {
                        for dy in 0..4 {
                            if dots[cy * 4 + dy][cx * 2 + dx] {
                                bits |= BRAILLE_BITS[dx][dy];
                            }
                        }
                    }
                    char::from_u32(0x2800 + bits).unwrap_or(' ')
                })
                .collect()
        })
        .collect()
}

fn in_logo(x: f64, y: f64) -> bool {
    let rect = |x0: f64, y0: f64, x1: f64, y1: f64| x >= x0 && x <= x1 && y >= y0 && y <= y1;

    let outer = rect(22.5, 26.5, 77.5, 69.5);
    let inner = rect(25.5, 29.5, 74.5, 66.5);
    let outline = outer && !inner;

    outline
        || rect(34.5, 37.0, 40.5, 43.0)
        || rect(59.5, 37.0, 65.5, 43.0)
        || rect(30.0, 54.0, 34.0, 58.0)
        || rect(66.0, 54.0, 70.0, 58.0)
        || rect(34.0, 58.0, 66.0, 62.0)
}
