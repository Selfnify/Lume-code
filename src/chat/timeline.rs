use super::tasker::Task;
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
}
pub struct Group {
    pub tasks: Vec<Task>,
    pub open: bool,
    pub started: std::time::Instant,
    pub finished: Option<u64>,
    pub settled: bool,
    pub restored: bool,
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
        let running = !group.settled && group.tasks.iter().any(|t| !t.done);
        let label = group
            .tasks
            .iter()
            .rev()
            .find(|t| !t.done)
            .or(group.tasks.last())
            .map(|t| t.label.as_str())
            .unwrap_or("Tool activity");
        let text = format!(
            "{} {} · {} tools{}",
            if group.open { "⌄" } else { "❯" },
            label,
            group.tasks.len(),
            if group.restored {
                String::new()
            } else {
                format!(
                    " · {}s",
                    group
                        .finished
                        .unwrap_or_else(|| group.started.elapsed().as_secs())
                )
            }
        );
        let phase = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            / 90) as usize;
        let spans = if running {
            text.chars()
                .enumerate()
                .map(|(i, c)| {
                    let tone = [85, 100, 120, 140, 120, 100][(i + phase) % 6];
                    Span::styled(
                        c.to_string(),
                        Style::default().fg(Color::Rgb(tone, tone - 5, tone - 12)),
                    )
                })
                .collect()
        } else {
            vec![Span::styled(text, Style::default().fg(Color::DarkGray))]
        };
        lines.push(Line::from(spans));
        if group.open {
            for task in &group.tasks {
                lines.push(Line::raw(format!(
                    "  {} {}",
                    if !task.done {
                        if group.settled {
                            "?"
                        } else {
                            "…"
                        }
                    } else if task.failed {
                        "!"
                    } else {
                        "✓"
                    },
                    task.label
                )));
                if task.name == "get_datetime" {
                    continue;
                }
                let detail = task.detail.lines().collect::<Vec<_>>();
                let visible = if task.done {
                    &detail[..detail.len().min(30)]
                } else {
                    &detail[detail.len().saturating_sub(30)..]
                };
                for text in visible {
                    lines.push(Line::styled(
                        format!("    {}", crate::clean(text)),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
            }
        }
        lines.push(Line::raw(""));
    }
}
