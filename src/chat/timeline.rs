use super::tasker::Task;
use crate::background::colors::active as theme;
#[cfg(test)]
mod tests;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
#[derive(Default)]
pub struct Timeline {
    pub groups: Vec<Group>,
    pub current: Option<usize>,
    pub full: bool,
}
pub struct Group {
    pub tasks: Vec<Task>,
    pub open: bool,
    pub started: std::time::Instant,
    pub finished: Option<u64>,
    pub settled: bool,
    pub restored: bool,
}
fn command_lines(detail: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(detail) else {
        return detail.lines().map(str::to_owned).collect();
    };
    if v.get("stdout").is_none() && v.get("error").is_none() {
        return detail.lines().map(str::to_owned).collect();
    }
    let mut out: Vec<String> = v["stdout"]
        .as_str()
        .unwrap_or("")
        .lines()
        .map(str::to_owned)
        .collect();
    out.extend(v["stderr"].as_str().unwrap_or("").lines().map(str::to_owned));
    if let Some(code) = v["exit_code"].as_i64().filter(|c| *c != 0) {
        out.push(format!("exit {code}"));
    }
    if let Some(error) = v["error"].as_str() {
        out.push(error.to_owned());
    }
    out
}
impl Timeline {
    pub fn latest(&mut self) {
        if let Some(g) = self.groups.last_mut() {
            g.open = !g.open;
        }
    }
    pub fn sync(&mut self, tasks: &[Task]) -> Option<usize> {
        let unseen = tasks
            .iter()
            .filter(|t| {
                !self
                    .groups
                    .iter()
                    .any(|g| g.tasks.iter().any(|v| v.id == t.id))
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut created = None;
        if !unseen.is_empty() {
            let index = match self.current {
                Some(i) => i,
                None => {
                    let i = self.groups.len();
                    self.groups.push(Group {
                        tasks: vec![],
                        open: false,
                        started: std::time::Instant::now(),
                        finished: None,
                        settled: false,
                        restored: false,
                    });
                    self.current = Some(i);
                    created = Some(i);
                    i
                }
            };
            self.groups[index].tasks.extend(unseen);
        }
        for g in &mut self.groups {
            if g.settled {
                continue;
            }
            for task in &mut g.tasks {
                if let Some(v) = tasks.iter().find(|v| v.id == task.id) {
                    *task = v.clone();
                }
            }
            if g.tasks.iter().any(|t| !t.done) {
                g.finished = None;
            }
            if g.tasks.iter().all(|t| t.done) && g.finished.is_none() {
                g.finished = Some(g.started.elapsed().as_secs());
            }
        }
        created
    }
    pub fn render(&self, index: usize, lines: &mut Vec<Line<'static>>) {
        let Some(group) = self.groups.get(index) else {
            return;
        };
        let phase = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            / 400) as usize;
        for task in &group.tasks {
            let color = if !task.done {
                if phase % 2 == 0 {
                    Color::White
                } else {
                    Color::DarkGray
                }
            } else if task.failed {
                Color::Red
            } else {
                Color::Green
            };
            let verb = if task.done { "Ran" } else { "Running" };
            let title = if !task.command.is_empty() {
                format!("{verb} {}", task.command)
            } else {
                format!("{verb} a tool")
            };
            lines.push(Line::from(vec![
                Span::styled("● ", Style::default().fg(color)),
                Span::raw(title),
            ]));
            let detail: Vec<String> = if task.name == "get_datetime" {
                vec![]
            } else if task.name == "run_command" {
                command_lines(&task.detail)
            } else {
                task.detail.lines().map(str::to_owned).collect()
            };
            let keep = if self.full { 30 } else { 3 };
            let visible = &detail[detail.len().saturating_sub(keep)..];
            let is_diff = detail.iter().any(|l| l.starts_with("@@"));
            if visible.is_empty() {
                lines.push(Line::styled(
                    "  └ (no output)",
                    Style::default().fg(theme::text()),
                ));
            }
            for (i, text) in visible.iter().enumerate() {
                let branch = if i == 0 { "└ " } else { "  " };
                let clean_text = crate::clean(text.as_str()).chars().take(160).collect::<String>();
                let color = if is_diff {
                    if clean_text.starts_with("@@") {
                        theme::accent()
                    } else if clean_text.starts_with('+') && !clean_text.starts_with("+++") {
                        Color::Green
                    } else if clean_text.starts_with('-') && !clean_text.starts_with("---") {
                        Color::Red
                    } else {
                        theme::text()
                    }
                } else {
                    theme::text()
                };
                lines.push(Line::styled(
                    format!("  {branch}{clean_text}"),
                    Style::default().fg(color),
                ));
            }
            lines.push(Line::raw(""));
        }
        lines.push(Line::raw(""));
    }
}