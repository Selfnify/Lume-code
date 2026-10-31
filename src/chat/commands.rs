use crate::background::colors::cream_terracotta as theme;
#[cfg(test)]
mod tests;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
    Frame,
};

pub struct Command {
    pub name: &'static str,
    pub usage: &'static str,
    pub description: &'static str,
}
pub const COMMANDS: &[Command] = &[
    Command {
        name: "/add-dir",
        usage: "/add-dir ",
        description: "Add another working directory",
    },
    Command {
        name: "/autocompact",
        usage: "/autocompact ",
        description: "Set context threshold (10–95%)",
    },
    Command {
        name: "/btw",
        usage: "/btw ",
        description: "Ask a separate quick question",
    },
    Command {
        name: "/clear",
        usage: "/clear",
        description: "Archive this session and start fresh",
    },
    Command {
        name: "/cd",
        usage: "/cd ",
        description: "Move this session to another directory",
    },
    Command {
        name: "/compact",
        usage: "/compact",
        description: "Summarize earlier conversation context",
    },
    Command {
        name: "/effort",
        usage: "/effort ",
        description: "Set off, low, medium, high or max",
    },
    Command {
        name: "/logout",
        usage: "/logout",
        description: "Sign out of your Selfnify account",
    },
    Command {
        name: "/resume",
        usage: "/resume ",
        description: "Search saved sessions and working folders",
    },
    Command {
        name: "/usage",
        usage: "/usage",
        description: "Show usage and allowance reset times",
    },
    Command {
        name: "/dirs",
        usage: "/dirs",
        description: "Show every accessible working folder",
    },
    Command {
        name: "/recovery",
        usage: "/recovery",
        description: "Inspect completed and unfinished work",
    },
    Command {
        name: "/side-history",
        usage: "/side-history",
        description: "Read saved side questions separately",
    },
    Command {
        name: "/tasks",
        usage: "/tasks",
        description: "Toggle tool activity",
    },
    Command {
        name: "/help",
        usage: "/help",
        description: "Show keyboard shortcuts",
    },
    Command {
        name: "/attach",
        usage: "/attach ",
        description: "Attach a project file",
    },
    Command {
        name: "/deep",
        usage: "/deep ",
        description: "Propose deep research",
    },
    Command {
        name: "/research",
        usage: "/research ",
        description: "Manage a research plan or report",
    },
    Command {
        name: "/coworker",
        usage: "/coworker ",
        description: "Manage background programs",
    },
    Command {
        name: "/reasoning",
        usage: "/reasoning ",
        description: "Alias for /effort",
    },
    Command {
        name: "/new",
        usage: "/new",
        description: "Alias for /clear",
    },
];
pub const SHORTCUTS: &str = "Keyboard shortcuts\n\n/                 Find commands\n↑ / ↓             Select a command\nTab / Enter       Insert selected command\nEnter             Send message or run command\nEsc               Close menu or stop reply/side question\nShift-Tab         Switch Plan / Edit\nCtrl-T            Expand latest inline tool group\nCtrl-R            Prepare Continue for interrupted work\nPageUp / PageDown  Scroll conversation\nEnd               Return to newest message\nCtrl-D twice      Exit Lume Code\n? (empty textbox) Show / hide this guide\n\nPress Esc or ? to close.";

pub fn matches(input: &str) -> Vec<&'static Command> {
    if !input.starts_with('/') || input.chars().any(char::is_whitespace) {
        return vec![];
    }
    COMMANDS
        .iter()
        .filter(|c| c.name.starts_with(input))
        .collect()
}
pub fn draw(frame: &mut Frame, area: Rect, commands: &[&Command], selected: usize) {
    frame.render_widget(Clear, area);
    let start = selected.saturating_sub(4);
    let lines: Vec<Line> = commands
        .iter()
        .enumerate()
        .skip(start)
        .take(5)
        .map(|(i, c)| {
            let style = if i == selected {
                Style::default()
                    .fg(Color::Rgb(35, 95, 190))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme::TEXT)
            };
            Line::from(vec![Span::styled(
                format!(
                    " {} {:<15} {}",
                    if i == selected { "❯" } else { " " },
                    c.name,
                    c.description
                ),
                style,
            )])
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(theme::BG)),
        area,
    );
}
