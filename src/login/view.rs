use super::{Model, Phase};
use crate::background::{colors::cream_terracotta as theme, ui};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};
#[derive(Default)]
pub struct Buttons {
    pub login: Rect,
    pub copy: Option<Rect>,
    pub cancel: Rect,
}
pub fn draw(frame: &mut Frame, model: &Model) -> Buttons {
    ui::draw(frame);
    let area = frame.area();
    let w = area.width.saturating_sub(4).min(96);
    let h = area.height.saturating_sub(2).min(30);
    let outer = Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::ACCENT));
    let inner = block.inner(outer);
    frame.render_widget(block, outer);
    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(3),
        Constraint::Length(2),
    ])
    .margin(1)
    .split(inner);
    frame.render_widget(
        Paragraph::new("Lume Code").style(
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new("Sign in with your Lume account to start working on your project.")
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(theme::TEXT)),
        chunks[1],
    );
    let mut lines = vec![
        Line::styled(
            model.status.clone(),
            Style::default().fg(if matches!(model.phase, Phase::Error) {
                Color::Red
            } else {
                theme::TEXT
            }),
        ),
        Line::raw(""),
    ];
    if let Some(link) = &model.link {
        lines.extend([
            Line::styled(
                format!("Confirmation code: {}", link.code),
                Style::default().fg(theme::ACCENT),
            ),
            Line::raw("Check this code in your browser before approving."),
            Line::raw(""),
            Line::raw("Public sign-in token"),
            Line::raw(link.token.clone()),
            Line::raw(""),
            Line::raw("If the browser did not open, copy this URL:"),
            Line::styled(link.url.clone(), Style::default().fg(theme::ACCENT)),
            Line::raw(""),
        ]);
        let remaining = link.expires_in.saturating_sub(
            model
                .link_started
                .map(|t| t.elapsed().as_secs())
                .unwrap_or(0),
        );
        lines.push(Line::raw(format!(
            "Link expires in {}:{:02}",
            remaining / 60,
            remaining % 60
        )));
    } else {
        lines.push(Line::raw(
            "Your browser handles sign-in. Your session is stored securely on this PC.",
        ));
    }
    let info = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((model.scroll, 0))
        .style(Style::default().fg(theme::TEXT));
    frame.render_widget(info, chunks[2]);
    let buttons = Layout::horizontal([
        Constraint::Length(32),
        Constraint::Length(15),
        Constraint::Length(13),
        Constraint::Min(0),
    ])
    .spacing(2)
    .split(chunks[3]);
    let label = match model.phase {
        Phase::Waiting => "Open browser [Enter]",
        Phase::Starting => "Creating sign-in link…",
        Phase::Error => "Try again [Enter]",
        Phase::Connected => "Signed in",
        Phase::Idle => "Sign in with browser [Enter]",
    };
    for (rect, text) in [(buttons[0], label), (buttons[2], "Cancel [Esc]")] {
        frame.render_widget(
            Paragraph::new(text)
                .centered()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                )
                .style(Style::default().fg(theme::TEXT)),
            rect,
        );
    }
    let copy = if model.link.is_some() {
        frame.render_widget(
            Paragraph::new("Copy URL [C]")
                .centered()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                )
                .style(Style::default().fg(theme::TEXT)),
            buttons[1],
        );
        Some(buttons[1])
    } else {
        None
    };
    frame.render_widget(
        Paragraph::new("Esc/Ctrl-C to cancel · Page Up/Down to scroll")
            .style(Style::default().fg(Color::DarkGray)),
        chunks[4],
    );
    Buttons {
        login: buttons[0],
        copy,
        cancel: buttons[2],
    }
}
use ratatui::style::Color;
