//! Account and backend scoped continuity. Credentials never enter these files.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
pub fn location(api: &str, identity: &str, root: &Path) -> Result<PathBuf, String> {
    let home = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("HOME"))
        .ok_or("Could not find your private application-data folder.")?;
    let hash = Sha256::digest(format!("{api}\0{identity}\0{}", root.display()).as_bytes());
    Ok(PathBuf::from(home)
        .join("LumeCode/sessions")
        .join(format!("{hash:x}.json")))
}
pub fn load(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(Value::Null);
    }
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 32 * 1024 * 1024 {
        return Err("Saved conversation exceeds the local storage limit. Start with /new.".into());
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Could not read the saved conversation: {e}"))
}
pub fn save(
    path: &Path,
    history: &[Value],
    binding: &Option<Value>,
    checkpoint: &Value,
    epoch: &Value,
) -> Result<(), String> {
    let parent = path.parent().ok_or("Missing session folder.")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let settings = load(path)?["settings"].clone();
    let bytes = serde_json::to_vec(
        &json!({"messages":history,"binding":binding,"checkpoint":checkpoint,"epoch":epoch,"settings":settings,"updated_at":crate::session_store::now()}),
    )
    .map_err(|e| e.to_string())?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err(
            "Conversation storage reached 32 MiB. Start /new after finishing this task.".into(),
        );
    }
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temp.write_all(&bytes).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
pub fn save_options(path: &Path, settings: &Value) -> Result<(), String> {
    let mut value = load(path)?;
    value["settings"] = settings.clone();
    let bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("Conversation storage reached 32 MiB.".into());
    }
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().ok_or("Missing session folder.")?)
        .map_err(|e| e.to_string())?;
    temp.write_all(&bytes).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

fn archive_dir(path: &Path) -> PathBuf {
    path.with_extension("")
}
pub fn archive(path: &Path) -> Result<Option<String>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let mut value = load(path)?;
    if value["messages"].as_array().is_none_or(|v| v.is_empty()) {
        return Ok(None);
    }
    value["side"] = crate::session_store::extra(path, &value["epoch"]);
    value["recovery"] = crate::session_store::journal(path, &value["epoch"]);
    let bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let id = format!("{:032x}", now);
    let directory = archive_dir(path);
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join(format!("{id}.json")))
        .map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    Ok(Some(id))
}
pub fn archived(path: &Path) -> Result<Vec<(String, String)>, String> {
    let directory = archive_dir(path);
    if !directory.exists() {
        return Ok(vec![]);
    }
    let mut files = std::fs::read_dir(directory)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter(|name| name.ends_with(".json") && valid_id(name.trim_end_matches(".json")))
        .collect::<Vec<_>>();
    files.sort();
    files.reverse();
    files
        .into_iter()
        .take(30)
        .map(|name| {
            let value = load(&archive_dir(path).join(&name))?;
            let title = value["messages"]
                .as_array()
                .and_then(|m| m.iter().find(|v| v["role"] == "user"))
                .and_then(|m| m["content"].as_str())
                .unwrap_or("Untitled session")
                .chars()
                .filter(|c| !c.is_control())
                .take(65)
                .collect();
            Ok((name.trim_end_matches(".json").into(), title))
        })
        .collect()
}
fn valid_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit())
}
pub fn resume(path: &Path, id: &str) -> Result<Value, String> {
    let id = if id == "latest" {
        archived(path)?
            .first()
            .map(|v| v.0.clone())
            .ok_or("No archived sessions for this project.")?
    } else {
        id.into()
    };
    if !valid_id(&id) {
        return Err("Choose an ID from /resume, or use /resume latest.".into());
    }
    let target = archive_dir(path).join(format!("{id}.json"));
    if !target.is_file() {
        return Err("This session was not found for this account and project.".into());
    }
    load(&target)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn continuity_is_scoped_and_saves_atomically_without_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("session.json");
        save(
            &p,
            &[json!({"role":"user","content":"hello"})],
            &None,
            &Value::Null,
            &json!(0),
        )
        .unwrap();
        save(
            &p,
            &[json!({"role":"assistant","content":"saved"})],
            &None,
            &Value::Null,
            &json!(0),
        )
        .unwrap();
        assert_eq!(load(&p).unwrap()["messages"][0]["content"], "saved");
        assert!(!std::fs::read_to_string(&p).unwrap().contains("token"));
        assert_ne!(
            location("https://a", "alice", dir.path()).unwrap(),
            location("https://a", "bob", dir.path()).unwrap()
        );
    }
}
