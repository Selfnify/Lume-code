use super::path;
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};
use walkdir::WalkDir;
const MAX: usize = 8 * 1024 * 1024;
fn text(p: &Path) -> Result<String, String> {
    if fs::metadata(p).map_err(|e| e.to_string())?.len() > MAX as u64 {
        return Err("Use a file smaller than 8 MiB.".into());
    }
    fs::read_to_string(p).map_err(|e| e.to_string())
}
fn entries(root: &Path) -> impl Iterator<Item = walkdir::DirEntry> {
    WalkDir::new(root)
        .follow_links(false)
        .max_depth(16)
        .into_iter()
        .filter_entry(|e| {
            e.depth() == 0
                || !matches!(
                    e.file_name().to_str(),
                    Some(".git" | "node_modules" | "target")
                )
        })
        .filter_map(Result::ok)
        .take(2000)
}
pub fn run(root: &Path, name: &str, a: &Value) -> Result<Value, String> {
    let value = |key: &str| a[key].as_str().unwrap_or("");
    if name == "get_info" {
        return Ok(
            json!({"os":std::env::consts::OS,"architecture":std::env::consts::ARCH,"project":root.display().to_string(),"shell":if cfg!(windows){"Windows PowerShell 5.1 (powershell.exe)"}else{"sh"}}),
        );
    }
    if name == "list_files" {
        let files = entries(root)
            .filter(|e| e.depth() > 0 && !e.path_is_symlink())
            .filter_map(|e| {
                let n = e
                    .path()
                    .strip_prefix(root)
                    .ok()?
                    .to_string_lossy()
                    .replace('\\', "/");
                path(root, &n).ok()?;
                Some(json!({"path":n,"directory":e.file_type().is_dir()}))
            })
            .collect::<Vec<_>>();
        return Ok(json!({"files":files,"limit":2000}));
    }
    if name == "search_files" {
        let q = value("query");
        if q.is_empty() || q.len() > 500 {
            return Err("Use a nonempty literal query up to 500 characters.".into());
        }
        let mut matches = vec![];
        for e in entries(root).filter(|e| e.file_type().is_file()) {
            let n = e
                .path()
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if path(root, &n).is_err() {
                continue;
            }
            if let Ok(t) = text(e.path()) {
                for (i, line) in t.lines().enumerate() {
                    if line.contains(q) {
                        matches.push(json!({"path":n,"line":i+1,"text":line.chars().take(250).collect::<String>()}));
                        if matches.len() == 50 {
                            return Ok(json!({"matches":matches,"limit":50}));
                        }
                    }
                }
            }
        }
        return Ok(json!({"matches":matches,"limit":50}));
    }
    let p = path(root, value("path"))?;
    match name {
        "read_file" => {
            let t = text(&p)?;
            let lines = t.lines().collect::<Vec<_>>();
            let start = a["start_line"].as_u64().unwrap_or(1).max(1) as usize;
            let count = a["line_count"].as_u64().unwrap_or(100).clamp(1, 200) as usize;
            let content = lines
                .iter()
                .skip(start - 1)
                .take(count)
                .copied()
                .collect::<Vec<_>>()
                .join("\n");
            Ok(
                json!({"path":value("path"),"content":content.chars().take(32000).collect::<String>(),"start_line":start,"total_lines":lines.len()}),
            )
        }
        "create_folder" => {
            fs::create_dir_all(&p).map_err(|e| e.to_string())?;
            Ok(json!({"created":value("path")}))
        }
        "create_file" | "write_file_chunk" | "edit_file" => {
            let old = if p.exists() { text(&p)? } else { String::new() };
            if name == "write_file_chunk" && !a.get("content").is_some() {
                return Ok(
                    json!({"exists":p.exists(),"next_offset":old.len(),"tail":old.chars().rev().take(300).collect::<Vec<_>>().into_iter().rev().collect::<String>()}),
                );
            }
            let required = if name == "edit_file" { "new_text" } else { "content" };
            if !a.get(required).is_some_and(Value::is_string) {
                return Err(format!("Provide {required} as text. No file was changed."));
            }
            let chunk = if name == "edit_file" {
                value("new_text")
            } else {
                value("content")
            };
            if chunk.len() > 4096 {
                return Err("Write at most 4096 UTF-8 bytes per operation.".into());
            }
            let next = if name == "edit_file" {
                let find = value("old_text");
                if find.is_empty() || old.matches(find).count() != 1 {
                    return Err("old_text must match exactly once. Read the file first.".into());
                }
                old.replacen(find, chunk, 1)
            } else if name == "write_file_chunk" {
                let offset = a["offset"].as_u64().ok_or("Provide a UTF-8 byte offset.")? as usize;
                if offset < old.len()
                    && old
                        .as_bytes()
                        .get(offset..offset.saturating_add(chunk.len()))
                        == Some(chunk.as_bytes())
                {
                    return Ok(json!({"next_offset":old.len(),"replayed":true}));
                }
                if offset != old.len() {
                    return Err(format!("Offset conflict: {} bytes are saved.", old.len()));
                }
                format!("{old}{chunk}")
            } else {
                if p.exists() && a["overwrite"] != true {
                    return Err(
                        "File exists. Read and edit it, or explicitly request overwrite.".into(),
                    );
                }
                chunk.to_owned()
            };
            if next.len() > MAX {
                return Err("File exceeds the 8 MiB limit.".into());
            }
            if let Some(parent) = p.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut staged =
                tempfile::NamedTempFile::new_in(p.parent().unwrap()).map_err(|e| e.to_string())?;
            staged
                .write_all(next.as_bytes())
                .map_err(|e| e.to_string())?;
            staged.as_file().sync_all().map_err(|e| e.to_string())?;
            staged.persist(&p).map_err(|e| e.to_string())?;
            if fs::read(&p).map_err(|e| format!("File was written but could not be verified: {e}"))? != next.as_bytes() {
                return Err("File was written but verification failed. Read it before retrying.".into());
            }
            Ok(json!({"saved":value("path"),"next_offset":next.len(),"size":next.len()}))
        }
        "move_file" => {
            let target = path(root, value("destination"))?;
            if target.exists() {
                return Err("Destination already exists.".into());
            }
            fs::rename(&p, target).map_err(|e| e.to_string())?;
            Ok(json!({"moved":true}))
        }
        "delete_file" => {
            if p.is_dir() {
                return Err("Directory removal is not automatic. Use an explicitly approved command if needed.".into());
            }
            fs::remove_file(p).map_err(|e| e.to_string())?;
            Ok(json!({"deleted":true}))
        }
        "present_file" => {
            let size = fs::metadata(&p).map_err(|e| e.to_string())?.len();
            if !p.is_file() {
                return Err("Choose an existing file.".into());
            }
            Ok(
                json!({"presented_file":{"path":p.display().to_string(),"name":value("path"),"size":size,"title":a["title"]},"message":"Saved locally; the user can open this project file."}),
            )
        }
        "compile_ZIP" => {
            let folder = path(root, value("folder"))?;
            if !folder.is_dir() || p.exists() {
                return Err("Choose a folder and a new ZIP output path.".into());
            }
            let out = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&p)
                .map_err(|e| e.to_string())?;
            let result = (|| {
                let mut zip = zip::ZipWriter::new(out);
                let mut total = 0;
                for e in entries(&folder).filter(|e| e.file_type().is_file()) {
                    let n = e
                        .path()
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    let file = path(root, &n)?;
                    if file == p {
                        continue;
                    }
                    if fs::metadata(&file).map_err(|e| e.to_string())?.len() > MAX as u64 {
                        return Err("ZIP input exceeds 8 MiB.".into());
                    }
                    let bytes = fs::read(file).map_err(|e| e.to_string())?;
                    total += bytes.len();
                    if total > MAX {
                        return Err("ZIP inputs exceed 8 MiB.".into());
                    }
                    zip.start_file(
                        e.path()
                            .strip_prefix(&folder)
                            .unwrap()
                            .to_string_lossy()
                            .replace('\\', "/"),
                        zip::write::SimpleFileOptions::default(),
                    )
                    .map_err(|e| e.to_string())?;
                    zip.write_all(&bytes).map_err(|e| e.to_string())?;
                }
                zip.finish().map_err(|e| e.to_string())?;
                Ok(json!({"created":value("path")}))
            })();
            if result.is_err() {
                let _ = fs::remove_file(&p);
            }
            result
        }
        "unzip" => {
            let folder = path(root, value("folder"))?;
            if folder.exists() {
                return Err("Extract into a new folder.".into());
            }
            if fs::metadata(&p).map_err(|e| e.to_string())?.len() > MAX as u64 {
                return Err("Archive exceeds 8 MiB.".into());
            }
            let file = fs::File::open(p).map_err(|e| e.to_string())?;
            let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
            if zip.len() > 1000 {
                return Err("Archive has too many entries.".into());
            }
            let mut planned = vec![];
            let mut total = 0u64;
            for i in 0..zip.len() {
                let f = zip.by_index(i).map_err(|e| e.to_string())?;
                let rel = f.enclosed_name().ok_or("Unsafe archive path.")?;
                if f.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
                    return Err("Archive links are not allowed.".into());
                }
                let n = format!(
                    "{}/{}",
                    value("folder"),
                    rel.to_string_lossy().replace('\\', "/")
                );
                let out = path(root, &n)?;
                total += f.size();
                if total > MAX as u64 {
                    return Err("Extracted files exceed 8 MiB.".into());
                }
                planned.push((out, f.is_dir()));
            }
            let mut actual = 0usize;
            for (i, (out, dir)) in planned.into_iter().enumerate() {
                if dir {
                    fs::create_dir_all(out).map_err(|e| e.to_string())?;
                } else {
                    fs::create_dir_all(out.parent().unwrap()).map_err(|e| e.to_string())?;
                    let mut bytes = vec![];
                    zip.by_index(i)
                        .map_err(|e| e.to_string())?
                        .take(MAX as u64 + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|e| e.to_string())?;
                    actual += bytes.len();
                    if actual > MAX {
                        return Err("Archive output exceeded the limit.".into());
                    }
                    fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(out)
                        .map_err(|e| e.to_string())?
                        .write_all(&bytes)
                        .map_err(|e| e.to_string())?;
                }
            }
            Ok(json!({"extracted":value("folder")}))
        }
        _ => Err("This local operation is not supported.".into()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_content_never_creates_or_erases_files() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap();
        assert!(run(&path, "create_file", &json!({"path":"new.txt"})).is_err());
        assert!(!path.join("new.txt").exists());
        fs::write(path.join("existing.txt"), "original").unwrap();
        assert!(run(&path, "create_file", &json!({"path":"existing.txt","overwrite":true})).is_err());
        assert!(run(&path, "edit_file", &json!({"path":"existing.txt","old_text":"original"})).is_err());
        assert_eq!(fs::read_to_string(path.join("existing.txt")).unwrap(), "original");
    }
    #[test]
    fn chunks_are_small_resumable_and_unicode_safe() {
        let root = std::env::temp_dir().join(format!("lume-code-files-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let _ = fs::remove_file(root.join("file.txt"));
        assert_eq!(
            run(
                &root,
                "write_file_chunk",
                &json!({"path":"file.txt","offset":0,"content":"é"})
            )
            .unwrap()["next_offset"],
            2
        );
        assert_eq!(
            run(
                &root,
                "write_file_chunk",
                &json!({"path":"file.txt","offset":0,"content":"é"})
            )
            .unwrap()["replayed"],
            true
        );
        assert!(run(
            &root,
            "write_file_chunk",
            &json!({"path":"file.txt","offset":1,"content":"x"})
        )
        .is_err());
        assert!(run(
            &root,
            "create_file",
            &json!({"path":"large.txt","content":"x".repeat(4097)})
        )
        .is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
