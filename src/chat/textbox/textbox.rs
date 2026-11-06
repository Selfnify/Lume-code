use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::background::colors::active as theme;

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

#[derive(Clone, Copy)]
struct ThemeMenu {
    selected: usize,
    original: usize,
}

pub struct TextBox {
    directory_cache:
        std::cell::RefCell<Option<(String, std::path::PathBuf, std::time::Instant, Vec<String>)>>,
    input: String,
    theme_menu: Option<ThemeMenu>,
    pub queued: Vec<String>,
    chord: Option<std::time::Instant>,
    mention_sel: usize,
    settings_sel: usize,
    pub resume_choice: Option<(String, String, String)>,
    resume_sel: usize,
    history: Vec<String>,
    history_pos: Option<usize>,
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
            theme_menu: None,
            queued: Vec::new(),
            history: Vec::new(),
            history_pos: None,
            chord: None,
            mention_sel: 0,
            settings_sel: 0,
            resume_choice: None,
            resume_sel: 0,
            timeline: Default::default(),
        }
    }

    pub fn plan(&self) -> bool {
        self.mode == Mode::Plan
    }
    pub fn input(&self) -> &str {
        &self.input
    }
    pub fn take_input(&mut self) -> String {
        std::mem::take(&mut self.input)
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
        if self.dismissed || self.theme_menu_open() || self.settings_open() {
            vec![]
        } else {
            crate::chat::commands::matches(&self.input)
        }
    }
    pub fn close_overlay(&mut self) -> bool {
        if self.theme_menu_open() {
            if let Some(menu) = self.theme_menu.take() {
                theme::set_index(menu.original);
            }
            self.input.clear();
            return true;
        }
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

    pub fn mention_query(&self) -> Option<&str> {
        let token = self.input.rsplit(char::is_whitespace).next()?;
        token.strip_prefix('@')
    }

    pub fn settings_open(&self) -> bool {
        self.input.trim() == "/settings"
    }

    pub fn mention_matches(&self) -> Vec<String> {
        if !crate::settings::mentions() {
            return vec![];
        }
        let Some(prefix) = self.mention_query() else {
            return vec![];
        };
        if prefix.contains('/') || prefix.contains('\\') {
            return vec![];
        }
        let root = std::env::current_dir().unwrap_or_default();
        let Ok(read) = std::fs::read_dir(root) else {
            return vec![];
        };
        let needle = prefix.to_lowercase();
        let mut names: Vec<String> = read
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') || !name.to_lowercase().starts_with(&needle) {
                    return None;
                }
                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                Some(if is_dir { format!("{name}/") } else { name })
            })
            .collect();
        names.sort();
        names.truncate(8);
        names
    }

    pub fn mention_selected(&self) -> usize {
        self.mention_sel
    }

    fn complete_mention(&mut self, pick: &str) {
        let start = self
            .input
            .rfind(char::is_whitespace)
            .map(|i| i + self.input[i..].chars().next().map(char::len_utf8).unwrap_or(1))
            .unwrap_or(0);
        self.input.truncate(start);
        self.input.push('@');
        self.input.push_str(pick);
        if !pick.ends_with('/') {
            self.input.push(' ');
        }
    }

    pub fn theme_menu_open(&self) -> bool {
        self.input.trim() == "/theme"
    }

    fn theme_menu_selected(&self) -> usize {
        self.theme_menu
            .map(|menu| menu.selected)
            .unwrap_or_else(theme::index)
    }

    pub fn insert_paste(&mut self, text: &str) {
        self.selected = 0;
        self.dismissed = false;
        self.input.push_str(&text.replace("\r\n", "\n").replace('\r', "\n"));
    }

    pub fn height(&self) -> u16 {
        let lines = self.input.split('\n').count().max(1) as u16;
        (lines + 2).clamp(3, 10)
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<String> {
        if key.kind != KeyEventKind::Press {
            return None;
        }
        if let Some((id, _, _)) = self.resume_choice.clone() {
            match key.code {
                KeyCode::Up | KeyCode::Down => self.resume_sel = 1 - self.resume_sel,
                KeyCode::Enter => {
                    self.resume_choice = None;
                    return Some(if self.resume_sel == 0 {
                        format!("/resume {id}")
                    } else {
                        format!("/resume {id} here")
                    });
                }
                KeyCode::Esc => self.resume_choice = None,
                _ => {}
            }
            return None;
        }
        if self.settings_open() {
            match key.code {
                KeyCode::Up => self.settings_sel = self.settings_sel.saturating_sub(1),
                KeyCode::Down => {
                    self.settings_sel = (self.settings_sel + 1).min(crate::settings::ITEMS.len() - 1)
                }
                KeyCode::Enter | KeyCode::Char(' ') => crate::settings::toggle(self.settings_sel),
                KeyCode::Esc => self.input.clear(),
                _ => {}
            }
            return None;
        }
        if let Some(started) = self.chord.take() {
            if started.elapsed() < std::time::Duration::from_secs(2) {
                match key.code {
                    KeyCode::Char('c') => return Some("/copy".into()),
                    KeyCode::Char('o') => return Some("/tools-full".into()),
                    _ => {}
                }
            }
        }
        if key.code == KeyCode::Char('x') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.chord = Some(std::time::Instant::now());
            return None;
        }
        if matches!(key.code, KeyCode::Char(_) | KeyCode::Backspace) {
            self.mention_sel = 0;
        }
        let mentions = self.mention_matches();
        if !mentions.is_empty() {
            match key.code {
                KeyCode::Up => {
                    self.mention_sel = self.mention_sel.saturating_sub(1);
                    return None;
                }
                KeyCode::Down => {
                    self.mention_sel = (self.mention_sel + 1).min(mentions.len() - 1);
                    return None;
                }
                KeyCode::Tab | KeyCode::Enter => {
                    let pick = mentions[self.mention_sel.min(mentions.len() - 1)].clone();
                    self.complete_mention(&pick);
                    return None;
                }
                _ => {}
            }
        }
        if self.theme_menu_open() {
            if self.theme_menu.is_none() {
                let current = theme::index();
                self.theme_menu = Some(ThemeMenu {
                    selected: current,
                    original: current,
                });
            }
            match key.code {
                KeyCode::Up => {
                    if let Some(menu) = self.theme_menu.as_mut() {
                        menu.selected = menu.selected.saturating_sub(1);
                        theme::set_index(menu.selected);
                    }
                }
                KeyCode::Down => {
                    if let Some(menu) = self.theme_menu.as_mut() {
                        menu.selected = (menu.selected + 1).min(theme::count() - 1);
                        theme::set_index(menu.selected);
                    }
                }
                KeyCode::Enter => {
                    self.theme_menu = None;
                    self.input.clear();
                    theme::save_current();
                }
                KeyCode::Esc => {
                    if let Some(menu) = self.theme_menu.take() {
                        theme::set_index(menu.original);
                    }
                    self.input.clear();
                }
                _ => {}
            }
            return None;
        }
        self.theme_menu = None;
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
                    let entry = picker.filtered().get(picker.selected).map(|v| (*v).clone());
                    self.picker = None;
                    let Some(entry) = entry else {
                        return None;
                    };
                    let id = entry["id"].as_str().unwrap_or("").to_string();
                    let folder = entry["root"].as_str().unwrap_or("").to_string();
                    let here = std::env::current_dir()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default();
                    if id.is_empty() || folder.is_empty() {
                        return None;
                    }
                    self.resume_choice = Some((id, folder, here));
                    self.resume_sel = 0;
                    return None;
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
            KeyCode::Enter if key.modifiers.intersects(KeyModifiers::SHIFT | KeyModifiers::ALT) => {
                self.input.push('\n');
                None
            }
            KeyCode::Enter => {
                let text = self.input.trim().to_string();
                self.input.clear();
                self.history_pos = None;
                if text.is_empty() {
                    None
                } else {
                    if self.history.last() != Some(&text) {
                        self.history.push(text.clone());
                    }
                    Some(text)
                }
            }
            KeyCode::Up if self.input.is_empty() || self.history_pos.is_some() => {
                if !self.history.is_empty() {
                    let pos = match self.history_pos {
                        Some(p) => p.saturating_sub(1),
                        None => self.history.len() - 1,
                    };
                    self.history_pos = Some(pos);
                    self.input = self.history[pos].clone();
                }
                None
            }
            KeyCode::Down if self.history_pos.is_some() => {
                let next = self.history_pos.unwrap_or(0) + 1;
                if next < self.history.len() {
                    self.history_pos = Some(next);
                    self.input = self.history[next].clone();
                } else {
                    self.history_pos = None;
                    self.input.clear();
                }
                None
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
        .border_style(Style::default().fg(theme::accent()))
        .style(Style::default().bg(theme::bg()));

    let content: Vec<Line> = if textbox.input.is_empty() {
        vec![Line::from(vec![
            Span::styled("> ", Style::default().fg(theme::text())),
            Span::styled(PLACEHOLDER, Style::default().fg(PLACEHOLDER_COLOR)),
        ])]
    } else {
        textbox
            .input()
            .split('\n')
            .enumerate()
            .map(|(i, line)| {
                let prefix = if i == 0 { "> " } else { "  " };
                Line::from(Span::styled(
                    format!("{prefix}{line}"),
                    Style::default().fg(theme::text()),
                ))
            })
            .collect()
    };

    let prompt = Paragraph::new(content)
        .style(Style::default().bg(theme::bg()))
        .block(block);
    frame.render_widget(prompt, area);

    let max_x = area.x + area.width.saturating_sub(2);
    let last_line = textbox.input().rsplit('\n').next().unwrap_or("");
    let rows = textbox.input().split('\n').count().max(1) as u16;
    let cursor_x = (area.x + 3 + last_line.chars().count() as u16).min(max_x);
    let cursor_y = (area.y + rows).min(area.bottom().saturating_sub(2));
    frame.set_cursor_position((cursor_x, cursor_y));
}

pub fn draw_status(frame: &mut Frame, area: Rect, textbox: &TextBox, exit_armed: bool) {
    let token_estimate = (textbox.input().chars().count() + 3) / 4;
    let line = if exit_armed {
        Line::from(Span::styled(
            "Press Ctrl-D again to exit",
            Style::default().fg(theme::accent()),
        ))
    } else {
        let hint = if textbox.working {
            "esc to interrupt"
        } else {
            "? for shortcuts"
        };
        let mut spans = vec![
            Span::styled("Mode: ", Style::default().fg(theme::text())),
            Span::styled(textbox.mode.label(), Style::default().fg(theme::accent())),
            Span::styled(
                " (shift+tab to switch) · ",
                Style::default().fg(theme::text()),
            ),
            Span::styled(hint, Style::default().fg(theme::text())),
            Span::styled(" · ◌ ", Style::default().fg(theme::text())),
            Span::styled(format!("~{token_estimate}"), Style::default().fg(theme::text())),
        ];
        let background = crate::local::jobs::running_count();
        if background > 0 {
            let plural = if background == 1 { "" } else { "s" };
            spans.push(Span::styled(
                format!(" · {background} background terminal{plural} running · /ps to view · /stop to close"),
                Style::default().fg(theme::text()),
            ));
        }
        Line::from(spans)
    };

    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(theme::bg())),
        area,
    );
}

pub fn draw_theme_menu(frame: &mut Frame, area: Rect, textbox: &TextBox) {
    let selected = textbox.theme_menu_selected();
    let lines: Vec<Line> = theme::names()
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let style = if i == selected {
                Style::default()
                    .fg(Color::Rgb(35, 95, 190))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme::text())
            };
            Line::from(Span::styled(
                format!(
                    " {} {:<18} {}",
                    if i == selected { ">" } else { " " },
                    name,
                    theme::description(i)
                ),
                style,
            ))
        })
        .collect();
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::accent()))
        .title(" Themes ")
        .style(Style::default().bg(theme::bg()));
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .style(Style::default().bg(theme::bg())),
        area,
    );
}

pub fn draw_mention_menu(frame: &mut Frame, area: Rect, textbox: &TextBox) {
    let names = textbox.mention_matches();
    let selected = textbox.mention_selected().min(names.len().saturating_sub(1));
    let lines: Vec<Line> = names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let style = if i == selected {
                Style::default()
                    .fg(Color::Rgb(35, 95, 190))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme::text())
            };
            Line::from(Span::styled(
                format!(" {} {name}", if i == selected { ">" } else { " " }),
                style,
            ))
        })
        .collect();
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::accent()))
        .title(" Files ")
        .style(Style::default().bg(theme::bg()));
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .style(Style::default().bg(theme::bg())),
        area,
    );
}

pub fn draw_settings_menu(frame: &mut Frame, area: Rect, textbox: &TextBox) {
    let selected = textbox.settings_sel;
    let lines: Vec<Line> = crate::settings::ITEMS
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let style = if i == selected {
                Style::default()
                    .fg(Color::Rgb(35, 95, 190))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme::text())
            };
            let mark = if crate::settings::get(i) { "[x]" } else { "[ ]" };
            Line::from(Span::styled(
                format!(" {} {mark} {name}", if i == selected { ">" } else { " " }),
                style,
            ))
        })
        .collect();
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::accent()))
        .title(" Settings ")
        .style(Style::default().bg(theme::bg()));
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .style(Style::default().bg(theme::bg())),
        area,
    );
}

pub fn draw_resume_choice(frame: &mut Frame, area: Rect, textbox: &TextBox) {
    let Some((_, folder, here)) = textbox.resume_choice.clone() else {
        return;
    };
    let rows = [
        format!("Open in the session folder: {folder}"),
        format!("Stay in the current folder: {here}"),
    ];
    let lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let style = if i == textbox.resume_sel {
                Style::default()
                    .fg(Color::Rgb(35, 95, 190))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme::text())
            };
            Line::from(Span::styled(
                format!(" {} {text}", if i == textbox.resume_sel { ">" } else { " " }),
                style,
            ))
        })
        .collect();
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::accent()))
        .title(" Resume in which folder? ")
        .style(Style::default().bg(theme::bg()));
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .style(Style::default().bg(theme::bg())),
        area,
    );
}