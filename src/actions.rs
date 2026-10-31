use crate::{ai::Event, connection::Connection, local};
use base64::Engine;
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
        Arc,
    },
    time::Duration,
};
pub async fn run(
    c: &Connection,
    binding: &Value,
    root: &Path,
    text: &str,
    images: &mut Vec<String>,
    out: &Sender<Event>,
    cancel: Arc<AtomicBool>,
) -> Result<bool, String> {
    let words = text.split_whitespace().collect::<Vec<_>>();
    let Some(command) = words.first().copied() else {
        return Ok(false);
    };
    let epoch = &binding["epoch"];
    if command == "/attach" {
        let name = text
            .strip_prefix("/attach ")
            .ok_or("Use /attach relative/path.png")?
            .trim();
        let file = local::path(root, name)?;
        if std::fs::metadata(&file).map_err(|e| e.to_string())?.len() > 8 * 1024 * 1024 {
            return Err("Attachment exceeds 8 MiB.".into());
        }
        let data = std::fs::read(file).map_err(|e| e.to_string())?;
        c.request(&format!("/tools/workspaces/{}/files",binding["id"].as_str().unwrap()),Some(&json!({"path":name,"content":base64::engine::general_purpose::STANDARD.encode(data),"epoch":epoch}))).await?;
        if matches!(
            name.rsplit('.')
                .next()
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str(),
            "png" | "jpg" | "jpeg" | "webp"
        ) {
            images.push(name.into());
        }
        let _ = out.send(Event::Text(format!(
            "Attached {name}. Send your message when ready."
        )));
        return Ok(true);
    }
    if command == "/research" {
        let action = *words
            .get(1)
            .ok_or("Use /research show|watch|start|cancel|edit ID [plan.json]")?;
        let id = *words
            .get(2)
            .ok_or("Provide the research ID shown in the plan.")?;
        if !id.starts_with("research_")
            || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err("Invalid research ID.".into());
        }
        let endpoint = format!("/research/{id}");
        if matches!(action, "start" | "cancel" | "edit") {
            let mut body = json!({"epoch":epoch,"action":action});
            if let Some(file) = words.get(3) {
                let p = local::path(root, file)?;
                if std::fs::metadata(&p).map_err(|e| e.to_string())?.len() > 65536 {
                    return Err("Research plan exceeds 64 KiB.".into());
                }
                body["plan"] =
                    serde_json::from_slice(&std::fs::read(p).map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?;
            }
            let v = c.request(&endpoint, Some(&body)).await?;
            let _ = out.send(Event::Text(format!("Research: {}", v["job"]["status"])));
            return Ok(true);
        }
        if !matches!(action, "show" | "watch") {
            return Err("Choose show, watch, start, cancel or edit.".into());
        }
        let mut report = String::new();
        let mut last = String::new();
        loop {
            let v = c.request(&endpoint, None).await?;
            let job = &v["job"];
            let state = job["status"].as_str().unwrap_or("unknown");
            let activity = job["activity"].to_string();
            if activity != last {
                last = activity.clone();
                let _ = out.send(Event::Activity(
                    json!({"state":"tool","name":"deep_search","activity_id":id,"output":activity}),
                ));
            }
            let next = job["report"].as_str().unwrap_or("");
            if !next.is_empty() {
                let delta = next.strip_prefix(&report).unwrap_or(next);
                if !delta.is_empty() {
                    let _ = out.send(Event::Text(delta.into()));
                }
                report = next.into();
            }
            if action == "show" {
                if report.is_empty() {
                    let _ = out.send(Event::Text(format!(
                        "{}\nStatus: {state}\nPlan: {}",
                        job["title"], job["plan"]
                    )));
                }
                break;
            }
            if matches!(state, "completed" | "failed" | "cancelled") {
                let _=out.send(Event::Activity(json!({"state":"tool_done","name":"deep_search","activity_id":id,"ok":state=="completed","output":format!("Research {state}")})));
                break;
            }
            if cancel.load(Ordering::Acquire) {
                let _ = out.send(Event::Notice(
                    "Stopped watching. Research continues; use /research cancel ID to stop it."
                        .into(),
                ));
                break;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        return Ok(true);
    }
    if command == "/coworker" {
        let action = *words.get(1).unwrap_or(&"list");
        if action == "list" {
            let v = c.request("/coworker/jobs", None).await?;
            let _ = out.send(Event::Text(serde_json::to_string_pretty(&v).unwrap()));
            return Ok(true);
        }
        let id = *words.get(2).ok_or("Provide the Coworker ID.")?;
        if !id.starts_with("cw_") || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return Err("Invalid Coworker ID.".into());
        }
        let endpoint = format!("/coworker/jobs/{id}");
        let mut body = json!({"epoch":epoch,"action":action});
        if action == "upload" {
            let list = text
                .splitn(4, char::is_whitespace)
                .nth(3)
                .ok_or("Use /coworker upload ID [\"main.py\",\"data.csv\"]")?;
            let names: Vec<String> =
                serde_json::from_str(list).map_err(|_| "Provide file paths as a JSON array.")?;
            if names.is_empty() || names.len() > 64 {
                return Err("Select 1-64 project files.".into());
            }
            let mut files = serde_json::Map::new();
            let mut total = 0u64;
            for name in &names {
                let p = local::path(root, name)?;
                total += std::fs::metadata(&p).map_err(|e| e.to_string())?.len();
                if total > 8 * 1024 * 1024 {
                    return Err("Uploads exceed 8 MiB.".into());
                }
                files.insert(
                    name.clone(),
                    json!(base64::engine::general_purpose::STANDARD
                        .encode(std::fs::read(p).map_err(|e| e.to_string())?)),
                );
            }
            body["files"] = json!(files);
            body["entrypoint"] = json!(names[0]);
        }
        if !matches!(action, "show" | "upload" | "start" | "stop" | "restart") {
            return Err("Choose list, show, upload, start, stop or restart.".into());
        }
        let v = c
            .request(&endpoint, if action == "show" { None } else { Some(&body) })
            .await?;
        let _ = out.send(Event::Text(serde_json::to_string_pretty(&v).unwrap()));
        return Ok(true);
    }
    Ok(false)
}
