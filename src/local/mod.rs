mod commands;
mod files;
mod lifetime;
mod astra;
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};
pub fn path(root: &Path, name: &str) -> Result<PathBuf, String> {
    if name.is_empty()
        || name.len() > 500
        || name.contains(['\\', ':'])
        || name.chars().any(char::is_control)
    {
        return Err("Use a relative project path with forward slashes.".into());
    }
    for part in name.split('/') {
        let trimmed = part.trim_end_matches(['.', ' ']);
        let stem = trimmed.split('.').next().unwrap_or("").to_ascii_uppercase();
        if trimmed != part
            || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        {
            return Err("Reserved Windows device paths are not project files.".into());
        }
    }
    let relative = Path::new(name);
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("Paths must stay in the selected project.".into());
    }
    let mut out = root.to_owned();
    for part in relative.components() {
        out.push(part);
        if let Ok(meta) = std::fs::symlink_metadata(&out) {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if meta.file_attributes() & 0x400 != 0 {
                    return Err("Links and junctions are not followed.".into());
                }
            }
            if meta.file_type().is_symlink()
                || !out
                    .canonicalize()
                    .map_err(|e| e.to_string())?
                    .starts_with(root)
            {
                return Err("This path leaves the selected project.".into());
            }
        }
    }
    Ok(out)
}
pub fn needs_approval(name: &str, args: &Value) -> bool {
    name == "run_command"
        || name == "delete_file"
        || name == "move_file"
        || (name == "create_file" && args["overwrite"] == true)
}
pub async fn execute(root: &Path, name: &str, args: &Value) -> Value {
    let result = if name == "astra_search" { astra::search(root,args) } else if name == "astra_preview_replace" { astra::preview_replace(root,args) } else if name == "run_command" {
        commands::run(root, args).await
    } else {
        files::run(root, name, args)
    };
    result.unwrap_or_else(|error| json!({"error":error}))
}
pub async fn execute_in(root: &Path, directories: &[PathBuf], name: &str, args: &Value) -> Value {
    match working_root(root, directories, args) {
        Some(root) => execute(root, name, args).await,
        None => json!({"error":"Choose main or an explicitly added dirN working directory."}),
    }
}
pub fn working_root<'a>(
    root: &'a Path,
    directories: &'a [PathBuf],
    args: &Value,
) -> Option<&'a Path> {
    let alias = args["directory"].as_str().unwrap_or("main");
    let selected = if alias == "main" {
        Some(root)
    } else {
        alias
            .strip_prefix("dir")
            .and_then(|n| n.parse::<usize>().ok())
            .and_then(|n| n.checked_sub(1))
            .and_then(|n| directories.get(n))
            .map(|p| p.as_path())
    };
    selected
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_reject_absolute_parent_and_windows_devices() {
        let root = std::env::temp_dir().canonicalize().unwrap();
        for n in [
            "../secret",
            "/etc/passwd",
            "C:/Windows",
            "file:stream",
            "a\\b",
            "a/../../x",
            "NUL",
            "CON.txt",
            "COM1",
            "file. ",
        ] {
            assert!(path(&root, n).is_err(), "{n}");
        }
    }
    #[test]
    fn destructive_actions_and_commands_require_approval() {
        assert!(needs_approval("run_command", &json!({})));
        assert!(needs_approval("delete_file", &json!({})));
        assert!(!needs_approval("read_file", &json!({})));
    }
    #[cfg(windows)]
    #[test]
    fn junctions_and_reserved_devices_cannot_escape_the_project() {
        let project = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = project.path().canonicalize().unwrap();
        std::fs::write(outside.path().join("private.txt"), "outside").unwrap();
        for name in ["CON", "NUL.txt", "COM1", "file. "] {
            assert!(path(&root, name).is_err());
        }
        let junction = root.join("linked");
        let result = std::process::Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(outside.path())
            .output()
            .unwrap();
        assert!(result.status.success());
        let blocked = path(&root, "linked/private.txt").is_err();
        std::fs::remove_dir(&junction).unwrap();
        assert!(blocked);
        assert_eq!(
            std::fs::read_to_string(outside.path().join("private.txt")).unwrap(),
            "outside"
        );
    }
}
