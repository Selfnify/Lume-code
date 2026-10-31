use crate::{session_store, sessions};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
fn index(api: &str, email: &str) -> Result<PathBuf, String> {
    let location = sessions::location(api, email, Path::new("catalog"))?;
    Ok(location.with_extension("catalog.json"))
}
pub fn register(api: &str, email: &str, root: &Path) -> Result<(), String> {
    let path = index(api, email)?;
    let mut value = sessions::load(&path)?;
    if !value.is_array() {
        value = json!([]);
    }
    let roots = value.as_array_mut().unwrap();
    let root = json!(root);
    if let Some(index) = roots.iter().position(|v| v == &root) {
        roots.remove(index);
    }
    if !roots.contains(&root) {
        roots.push(root);
        if roots.len() > 100 {
            roots.remove(0);
        }
    }
    session_store::write(&path, &value)
}
pub fn list(api: &str, email: &str, current: &Path, epoch: &Value) -> Result<Vec<Value>, String> {
    let mut roots = sessions::load(&index(api, email)?)?
        .as_array()
        .cloned()
        .unwrap_or_default();
    let root = json!(current);
    if !roots.contains(&root) {
        roots.push(root);
    }
    let active = sessions::location(api, email, current)?;
    let mut entries = vec![];
    for root in roots.iter().rev().take(100).filter_map(Value::as_str) {
        if entries.len() >= 200 {
            break;
        }
        let path = sessions::location(api, email, Path::new(root))?;
        let mut sources = vec![];
        if path != active && path.is_file() {
            sources.push((path.clone(), String::new()));
        }
        for (id, _) in sessions::archived(&path).unwrap_or_default() {
            sources.push((path.with_extension("").join(format!("{id}.json")), id));
        }
        for (file, archive) in sources {
            let Ok(value) = sessions::load(&file) else {
                continue;
            };
            if value["epoch"] != *epoch {
                continue;
            }
            let title = value["messages"]
                .as_array()
                .and_then(|m| m.iter().find(|v| v["role"] == "user"))
                .and_then(|m| m["content"].as_str())
                .unwrap_or("Untitled session")
                .chars()
                .filter(|c| !c.is_control())
                .take(70)
                .collect::<String>();
            let updated = value["updated_at"]
                .as_i64()
                .or_else(|| {
                    std::fs::metadata(&file)
                        .ok()?
                        .modified()
                        .ok()?
                        .duration_since(std::time::UNIX_EPOCH)
                        .ok()
                        .map(|d| d.as_secs() as i64)
                })
                .unwrap_or(0);
            let id =
                format!("{:x}", Sha256::digest(file.to_string_lossy().as_bytes()))[..32].to_owned();
            entries.push(json!({"id":id,"title":title,"root":root,"file":file,"archive":archive,"updated_at":updated}));
            if entries.len() >= 200 {
                break;
            }
        }
    }
    entries.sort_by_key(|v| std::cmp::Reverse(v["updated_at"].as_i64().unwrap_or(0)));
    entries.truncate(200);
    Ok(entries)
}
#[cfg(test)]
mod tests;
