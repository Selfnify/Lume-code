use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::background::colors::cream_terracotta as theme;

const PLACEHOLDER: &str = "Ask lume anything";
const PLACEHOLDER_COLOR: Color = Color::DarkGray;

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Edit,
    Plan,
}

impl Mode {
    fn toggle(self) -> Self {
        match self {
            Mode::Edit => Mode::Plan,
            Mode::Plan => Mode::Edit,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Mode::Edit => "edit",
            Mode::Plan => "plan",
        }
    }
}

pub struct TextBox {
    directory_cache:
        std::cell::RefCell<Option<(String, std::path::PathBuf, std::time::Instant, Vec<String>)>>,
    input: String,
    mode: Mode,
    working: bool,
    pub selected: usize,
    dismissed: bool,
    pub shortcuts: bool,
    pub side_reply: Option<String>,
    pub state: serde_json::Value,
    pub usage: serde_json::Value,
    pub picker: Option<crate::chat::picker::Picker>,
    pub timeline: crate::chat::timeline::Timeline,
}

impl TextBox {
    pub fn new() -> Self {
        Self {
            directory_cache: std::cell::RefCell::new(None),
            input: String::new(),
            mode: Mode::Edit,
            working: false,
            selected: 0,
            dismissed: false,
            shortcuts: false,
            side_reply: None,
            state: serde_json::Value::Null,
            usage: serde_json::Value::Null,
            picker: None,
            timeline: Default::default(),
        }
    }

    pub fn plan(&self) -> bool {
        self.mode == Mode::Plan
    }
    pub fn input(&self) -> &str {
        &self.input
    }
    pub fn set_input(&mut self, text: String) {
        self.input = text;
        self.dismissed = true;
    }
    pub fn directory_choices(&self) -> Vec<String> {
        if self.dismissed {
            return vec![];
        }
        let root = std::env::current_dir().unwrap_or_default();
        if let Some((input, path, time, choices)) = self.directory_cache.borrow().as_ref() {
            if input == &self.input
                && path == &root
                && time.elapsed() < std::time::Duration::from_millis(500)
            {
                return choices.clone();
            }
        }
        let choices = crate::chat::directories::complete(&self.input, &root);
        *self.directory_cache.borrow_mut() = Some((
            self.input.clone(),
            root,
            std::time::Instant::now(),
            choices.clone(),
        ));
        choices
    }
    pub fn commands(&self) -> Vec<&'static crate::chat::commands::Command> {
        if self.dismissed {
            vec![]
        } else {
            crate::chat::commands::matches(&self.input)
        }
    }
    pub fn close_overlay(&mut self) -> bool {
        if self.picker.take().is_some() {
            return true;
        }
        if !self.directory_choices().is_empty() {
            self.dismissed = true;
            return true;
        }
        if self.shortcuts {
            self.shortcuts = false;
            return true;
        }
        if !self.commands().is_empty() {
            self.dismissed = true;
            return true;
        }
        false
    }

    #[allow(dead_code)]
    pub fn set_working(&mut self, working: bool) {
        self.working = working;
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<String> {
        if key.kind != KeyEventKind::Press {
            return None;
        }
        if let Some(picker) = self.picker.as_mut() {
            match key.code {
                KeyCode::Up => picker.selected = picker.selected.saturating_sub(1),
                KeyCode::Down => {
                    picker.selected =
                        (picker.selected + 1).min(picker.filtered().len().saturating_sub(1))
                }
                KeyCode::Backspace => {
                    picker.query.pop();
                    picker.selected = 0;
                }
                KeyCode::Char(c) => {
                    picker.query.push(c);
                    picker.selected = 0;
                }
                KeyCode::Enter => {
                    let command = picker
                        .filtered()
                        .get(picker.selected)
                        .and_then(|v| v["id"].as_str())
                        .map(|id| format!("/resume {id}"));
                    self.picker = None;
                    return command;
                }
                _ => {}
            }
            return None;
        }
        let directories = if key.code == KeyCode::Tab {
            crate::chat::directories::complete(
                &self.input,
                &std::env::current_dir().unwrap_or_default(),
            )
        } else {
            self.directory_choices()
        };
        if !directories.is_empty() {
            match key.code {
                KeyCode::Down => {
                    self.selected = (self.selected + 1).min(directories.len() - 1);
                    return None;
                }
                KeyCode::Up => {
                    self.selected = self.selected.saturating_sub(1);
                    return None;
                }
                KeyCode::Enter | KeyCode::Tab => {
                    self.input = directories[self.selected.min(directories.len() - 1)].clone();
                    self.dismissed = true;
                    return None;
                }
                _ => {}
            }
        }
        if key.code == KeyCode::Char('?') && (self.input.is_empty() || self.shortcuts) {
            self.shortcuts = !self.shortcuts;
            return None;
        }
        if self.shortcuts {
            self.shortcuts = false;
        }
        let commands = self.commands();
        if !commands.is_empty() {
            match key.code {
                KeyCode::Down => {
                    self.selected = (self.selected + 1).min(commands.len() - 1);
                    return None;
                }
                KeyCode::Up => {
                    self.selected = self.selected.saturating_sub(1);
                    return None;
                }
                KeyCode::Tab | KeyCode::Enter => {
                    self.input = commands[self.selected.min(commands.len() - 1)].usage.into();
                    self.dismissed = true;
                    return None;
                }
                _ => {}
            }
        }
        if matches!(key.code, KeyCode::Char(_) | KeyCode::Backspace) {
            self.selected = 0;
            self.dismissed = false;
        }
        match key.code {
            KeyCode::Enter => {
                let text = self.input.trim().to_string();
                self.input.clear();
                if text.is_empty() {
                    None
                } else {
                    Some(text)
                }
            }
            KeyCode::Backspace => {
                self.input.pop();
                None
            }
            KeyCode::BackTab => {
                self.mode = self.mode.toggle();
                None
            }
            KeyCode::Char(c) => {
                self.input.push(c);
                None
            }
            _ => None,
        }
    }
}

pub fn draw(frame: &mut Frame, area: Rect, textbox: &TextBox) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::ACCENT))
        .style(Style::default().bg(theme::BG));

    let content = if textbox.input.is_empty() {
        Line::from(vec![
            Span::styled("> ", Style::default().fg(theme::TEXT)),
            Span::styled(PLACEHOLDER, Style::default().fg(PLACEHOLDER_COLOR)),
        ])
    } else {
        Line::from(Span::styled(
            format!("> {}", textbox.input()),
            Style::default().fg(theme::TEXT),
        ))
    };

    let prompt = Paragraph::new(content)
        .style(Style::default().bg(theme::BG))
        .block(block);
    frame.render_widget(prompt, area);

    let max_x = area.x + area.width.saturating_sub(2);
    let cursor_x = (area.x + 3 + textbox.input().chars().count() as u16).min(max_x);
    frame.set_cursor_position((cursor_x, area.y + 1));
}

pub fn draw_status(frame: &mut Frame, area: Rect, textbox: &TextBox, exit_armed: bool) {
    let token_estimate = (textbox.input().chars().count() + 3) / 4;
    let line = if exit_armed {
        Line::from(Span::styled(
            "Press Ctrl-D again to exit",
            Style::default().fg(theme::ACCENT),
        ))
    } else {
        let hint = if textbox.working {
            "esc to interrupt"
        } else {
            "? for shortcuts"
        };
        Line::from(vec![
            Span::styled("Mode: ", Style::default().fg(theme::TEXT)),
            Span::styled(textbox.mode.label(), Style::default().fg(theme::ACCENT)),
            Span::styled(
                " (shift+tab to switch) · ",
                Style::default().fg(theme::TEXT),
            ),
            Span::styled(hint, Style::default().fg(theme::TEXT)),
            Span::styled(" · ◌ ", Style::default().fg(theme::TEXT)),
            Span::styled(format!("~{token_estimate}"), Style::default().fg(theme::TEXT)),
        ])
    };

    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(theme::BG)),
        area,
    );
}
