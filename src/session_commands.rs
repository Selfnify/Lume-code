use crate::{ai::Event, connection::Connection, sessions};
#[cfg(test)]
mod tests;
use serde_json::{json, Value};
use std::{path::PathBuf, sync::mpsc::Sender};
pub struct Settings {
    pub lock: Option<crate::session_store::Lock>,
    pub root: PathBuf,
    pub directories: Vec<PathBuf>,
    pub effort: String,
    pub threshold: u8,
}
impl Settings {
    pub fn new(root: PathBuf) -> Self {
        Self {
            lock: None,
            root,
            directories: vec![],
            effort: "off".into(),
            threshold: 80,
        }
    }
    pub fn info(&self) -> Value {
        json!({"threshold":self.threshold,"directories":self.directories.iter().enumerate().map(|(i,p)|json!({"alias":format!("dir{}",i+1),"path":p.display().to_string()})).collect::<Vec<_>>()})
    }
    pub fn persisted(&self) -> Value {
        let mut v = self.info();
        v["effort"] = json!(self.effort);
        v["root"] = json!(self.root);
        v
    }
    pub fn restore(&mut self, value: &Value) {
        if let Some(level) = value["effort"]
            .as_str()
            .filter(|s| ["off", "low", "medium", "high", "max"].contains(s))
        {
            self.effort = level.into();
        }
        if let Some(percent) = value["threshold"]
            .as_u64()
            .filter(|v| (10..=95).contains(v))
        {
            self.threshold = percent as u8;
        }
        if let Some(dirs) = value["directories"].as_array() {
            self.directories.clear();
            for entry in dirs.iter().take(8) {
                if let Some(path) = entry["path"]
                    .as_str()
                    .and_then(|p| directory(&self.root, p).ok())
                {
                    if path != self.root && !self.directories.contains(&path) {
                        self.directories.push(path);
                    } else {
                        self.directories.clear();
                        break;
                    }
                } else {
                    self.directories.clear();
                    break;
                }
            }
        }
    }
}
pub fn directory(current: &std::path::Path, arg: &str) -> Result<PathBuf, String> {
    let value = arg.trim().trim_matches('"');
    if value.is_empty() {
        return Err("Provide a directory path, for example /cd \"C:\\Projects\\app\".".into());
    }
    let path = std::path::Path::new(value);
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        current.join(path)
    };
    let path = path
        .canonicalize()
        .map_err(|_| "That directory could not be opened.")?;
    if !path.is_dir() {
        return Err("Choose a directory, not a file.".into());
    }
    Ok(path)
}
pub fn run(
    text: &str,
    s: &mut Settings,
    c: &Connection,
    email: &str,
    path: &mut Option<PathBuf>,
    history: &mut Vec<Value>,
    client: &mut Option<Value>,
    checkpoint: &mut Value,
    epoch: &Value,
    out: &Sender<Event>,
) -> Result<bool, String> {
    let (command, arg) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    let say = |text: String| {
        let _ = out.send(Event::Text(text));
    };
    match command {
        "/effort" | "/reasoning" => {
            let level = arg.trim();
            if level.is_empty() {
                say(format!(
                    "Effort: {}. Choose off, low, medium, high or max.",
                    s.effort
                ));
            } else if ["off", "low", "medium", "high", "max"].contains(&level) {
                s.effort = level.into();
                say(format!("Effort set to {level}."));
            } else {
                return Err("Choose off, low, medium, high or max.".into());
            }
        }
        "/autocompact" => {
            if arg.trim().is_empty() {
                say(format!("Auto-compaction starts at {}% of the estimated working context. Use /autocompact 80.",s.threshold));
            } else {
                let percent = arg
                    .trim()
                    .trim_end_matches('%')
                    .parse::<u8>()
                    .ok()
                    .filter(|v| (10..=95).contains(v))
                    .ok_or("Use a context threshold from 10% to 95%.")?;
                s.threshold = percent;
                say(format!(
                    "Auto-compaction set to {percent}% of the estimated working context."
                ));
            }
        }
        "/add-dir" => {
            let directory = directory(&s.root, arg)?;
            if directory == s.root || s.directories.contains(&directory) {
                return Err("That working directory is already available.".into());
            }
            if s.directories.len() >= 8 {
                return Err("This session already has eight additional directories.".into());
            }
            s.directories.push(directory.clone());
            say(format!(
                "Added dir{}: {}. File tools can select this directory explicitly.",
                s.directories.len(),
                directory.display()
            ));
        }
        "/cd" => {
            let directory = directory(&s.root, arg)?;
            if directory == s.root {
                say(format!("Already working in {}.", directory.display()));
                return Ok(true);
            }
            let previous=path.as_ref().map(|p|json!({"side":crate::session_store::extra(p,epoch),"recovery":crate::session_store::journal(p,epoch)})).unwrap_or(Value::Null);
            if let Some(p) = path.as_ref() {
                sessions::save(p, history, client, checkpoint, epoch)?;
                sessions::archive(p)?;
            }
            let next = sessions::location(&c.api, email, &directory)?;
            let lock = crate::session_store::Lock::acquire(&next)?;
            sessions::archive(&next)?;
            crate::session_store::import(&next, &previous, epoch)?;
            s.root = directory.clone();
            s.lock = Some(lock);
            s.directories.retain(|p| p != &directory);
            *path = Some(next);
            *client = None;
            let _ = out.send(Event::Directory(directory.display().to_string()));
            say(format!(
                "Working directory changed to {}. This conversation is preserved.",
                directory.display()
            ));
        }
        "/clear" | "/new" => {
            let mut archived = None;
            if let Some(p) = path.as_ref() {
                sessions::save(p, history, client, checkpoint, epoch)?;
                archived = sessions::archive(p)?;
            }
            history.clear();
            if let Some(p) = path.as_ref() {
                crate::session_store::clear(p, epoch)?;
            }
            *client = None;
            *checkpoint = Value::Null;
            let _ = out.send(Event::History(vec![]));
            say(archived
                .map(|id| {
                    format!("Started a fresh session. Resume the previous one with /resume {id}.")
                })
                .unwrap_or("Started a fresh session.".into()));
        }
        "/resume" => {
            let current = path
                .as_ref()
                .ok_or("Sign in to list saved sessions.")?
                .clone();
            let catalog = crate::session_catalog::list(&c.api, email, &s.root, epoch)?;
            if arg.trim().is_empty() {
                let _ = out.send(Event::Picker(catalog));
            } else {
                let entry = if arg.trim() == "latest" {
                    catalog.first()
                } else {
                    catalog
                        .iter()
                        .find(|v| v["id"].as_str() == Some(arg.trim()))
                };
                let (mut saved, root) = if let Some(entry) = entry {
                    let file =
                        PathBuf::from(entry["file"].as_str().ok_or("Invalid saved session")?);
                    let mut saved = sessions::load(&file)?;
                    if entry["archive"] == "" {
                        saved["side"] = crate::session_store::extra(&file, epoch);
                        saved["recovery"] = crate::session_store::journal(&file, epoch);
                    }
                    (
                        saved,
                        directory(&s.root, entry["root"].as_str().unwrap_or(""))?,
                    )
                } else {
                    (sessions::resume(&current, arg.trim())?, s.root.clone())
                };
                if saved["epoch"] != *epoch {
                    return Err("This saved session belongs to an earlier account-data version and cannot be restored.".into());
                }
                sessions::save(&current, history, client, checkpoint, epoch)?;
                sessions::archive(&current)?;
                let next = if root == s.root {
                    current.clone()
                } else {
                    sessions::location(&c.api, email, &root)?
                };
                let next_lock = if next != current {
                    Some(crate::session_store::Lock::acquire(&next)?)
                } else {
                    None
                };
                if let Some(entry) = entry {
                    if entry["archive"] == "" && next != current {
                        let file =
                            PathBuf::from(entry["file"].as_str().ok_or("Invalid saved session")?);
                        saved = sessions::load(&file)?;
                        if saved["epoch"] != *epoch {
                            return Err("This session changed account-data versions before it could be resumed.".into());
                        }
                        saved["side"] = crate::session_store::extra(&file, epoch);
                        saved["recovery"] = crate::session_store::journal(&file, epoch);
                    }
                }
                if next != current {
                    sessions::archive(&next)?;
                }
                crate::session_store::import(&next, &saved, epoch)?;
                *history = saved["messages"]
                    .as_array_mut()
                    .map(std::mem::take)
                    .unwrap_or_default();
                crate::session_store::restore(&next, epoch, history)?;
                *checkpoint = saved["checkpoint"].clone();
                *client = saved["binding"]
                    .as_object()
                    .map(|_| saved["binding"].clone());
                s.root = root;
                if let Some(lock) = next_lock {
                    s.lock = Some(lock);
                }
                s.restore(&saved["settings"]);
                *path = Some(next);
                let _ = out.send(Event::Directory(s.root.display().to_string()));
                let _ = out.send(Event::History(history.clone()));
                say("Resumed the saved conversation.".into());
            }
        }
        "/dirs" => {
            say(format!(
                "Working directories\n\nmain: {}\n{}",
                s.root.display(),
                s.directories
                    .iter()
                    .enumerate()
                    .map(|(i, p)| format!("dir{}: {}", i + 1, p.display()))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }
        "/recovery" => {
            let p = path.as_ref().ok_or("Load a session first.")?;
            let run = crate::session_store::journal(p, epoch);
            let mut tasks = vec![];
            for event in run["activities"].as_array().into_iter().flatten() {
                crate::chat::tasker::apply(&mut tasks, 0, event);
            }
            say(format!(
                "Recovery details\n\nRequest: {}\n\n{}\n\n{}",
                run["request"].as_str().unwrap_or("No interrupted request"),
                tasks
                    .iter()
                    .map(|t| format!(
                        "- {} {}",
                        if !t.done {
                            "Unconfirmed:"
                        } else if t.failed {
                            "Failed:"
                        } else {
                            "Completed:"
                        },
                        t.label
                    ))
                    .collect::<Vec<_>>()
                    .join("\n"),
                crate::session_store::CONTINUE
            ));
        }
        _ => return Ok(false),
    }
    Ok(true)
}
