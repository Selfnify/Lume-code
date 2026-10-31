use std::path::{Path, PathBuf};
#[cfg(test)]
mod tests;
pub fn complete(input: &str, root: &Path) -> Vec<String> {
    let Some((command, arg)) = input.split_once(' ') else {
        return vec![];
    };
    if !matches!(command, "/cd" | "/add-dir") {
        return vec![];
    }
    let arg = arg.trim().trim_matches('"');
    let typed = Path::new(arg);
    let trailing = arg.is_empty() || arg.ends_with(['\\', '/']);
    let base = if trailing {
        typed.to_path_buf()
    } else {
        typed.parent().unwrap_or(Path::new("")).to_path_buf()
    };
    let prefix = if trailing {
        String::new()
    } else {
        typed
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase()
    };
    let lookup = if base.is_absolute() {
        base.clone()
    } else {
        root.join(&base)
    };
    let Ok(read) = std::fs::read_dir(lookup) else {
        return vec![];
    };
    let mut entries = read
        .take(2000)
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.chars().any(char::is_control) || !name.to_lowercase().starts_with(&prefix) {
                return None;
            }
            Some(base.join(name))
        })
        .collect::<Vec<PathBuf>>();
    entries.sort_by_key(|p| p.to_string_lossy().to_lowercase());
    entries.truncate(100);
    entries
        .into_iter()
        .map(|p| format!("{command} \"{}{}\"", p.display(), std::path::MAIN_SEPARATOR))
        .collect()
}
