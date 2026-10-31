use serde_json::{json, Value};
#[derive(Clone)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub label: String,
    pub done: bool,
    pub failed: bool,
    pub detail: String,
}
pub fn label(name: &str) -> &str {
    match name {
        "read_file" => "Reading file",
        "list_files" => "Listing files",
        "search_files" => "Searching files",
        "create_file" | "write_file_chunk" => "Writing file",
        "edit_file" => "Editing file",
        "run_command" => "Running command",
        "run_code" => "Running sandbox code",
        "web_search" => "Searching the web",
        "fetch_web" => "Reading web page",
        "update_task" => "Updating tasks",
        "get_datetime" => "Checking date and time",
        "context_compacting" | "context_compacted" => "Compacting conversation",
        "deep_search" => "Deep research",
        "get_info" => "Checking project",
        "create_folder" => "Creating folder",
        "move_file" => "Moving file",
        "delete_file" => "Deleting file",
        "compile_ZIP" => "Packaging files",
        "unzip" => "Extracting files",
        "present_file" => "Presenting file",
        "highlight" => "Highlighting useful information",
        "image_search" => "Searching images",
        "switch_mode" => "Updating mode",
        "coworker_manage" | "coworker_status" | "request_coworker_files" => "Checking Coworker",
        "tool_preparing" | "tool" | "file_preview" => "Working",
        "tool_done" | "tool_generation_failed" => "Tool activity",
        _ => "Using a connected tool",
    }
}
fn finished_label(name: &str) -> &str {
    match name {
        "read_file" => "File read",
        "list_files" => "Files listed",
        "search_files" => "Files searched",
        "create_file" | "write_file_chunk" | "edit_file" => "File saved",
        "run_command" => "Command finished",
        "run_code" => "Code finished",
        "web_search" => "Searched the web",
        "fetch_web" => "Page read",
        "context_compacting" | "context_compacted" => "Conversation compacted",
        _ => label(name),
    }
}
pub fn apply(tasks: &mut Vec<Task>, turn: usize, v: &Value) {
    let state = v["state"].as_str().unwrap_or("");
    if !matches!(
        state,
        "tool_preparing"
            | "tool"
            | "tool_done"
            | "tool_reused"
            | "file_preview"
            | "tool_generation_failed"
            | "context_compacting"
            | "context_compacted"
    ) {
        return;
    }
    let supplied = v["name"].as_str().filter(|s| !s.is_empty());
    let name = supplied.unwrap_or(state);
    let prefix = format!("{turn}_");
    let id = if let Some(id) = v["activity_id"].as_str() {
        format!("{prefix}{id}")
    } else {
        tasks
            .iter()
            .rev()
            .find(|t| {
                t.id.starts_with(&prefix) && (supplied.is_none() || t.name == name) && !t.done
            })
            .map(|t| t.id.clone())
            .unwrap_or_else(|| format!("{prefix}{name}_{}", tasks.len()))
    };
    let done = matches!(
        state,
        "tool_done" | "tool_reused" | "tool_generation_failed" | "context_compacted"
    );
    let failed = v["ok"] == false || state == "tool_generation_failed";
    let detail = if state == "file_preview" {
        v["text_delta"].as_str().unwrap_or("")
    } else {
        v["output"].as_str().unwrap_or("")
    };
    if let Some(t) = tasks.iter_mut().find(|t| t.id == id) {
        if let Some(name) = supplied {
            t.name = name.into();
            if !t.done {
                t.label = label(name).into();
            }
        }
        t.done |= done;
        t.failed |= failed;
        if done {
            t.label = if failed {
                format!("Stopped: {}", label(&t.name))
            } else {
                finished_label(&t.name).into()
            };
        }
        if state == "file_preview" {
            if v["reset"] == true {
                t.detail.clear();
            }
            t.detail.push_str(detail);
            t.detail = t.detail.chars().take(12000).collect();
        } else if !detail.is_empty() {
            t.detail = detail.chars().take(4000).collect();
        }
    } else {
        tasks.push(Task {
            id,
            name: name.into(),
            label: if done {
                if failed {
                    format!("Stopped: {}", label(name))
                } else {
                    finished_label(name).into()
                }
            } else {
                label(name).into()
            },
            done,
            failed,
            detail: detail.chars().take(4000).collect(),
        });
    }
}
pub fn finish(tasks: &mut [Task]) {
    for t in tasks {
        if !t.done {
            t.done = true;
            t.failed = true;
        }
    }
}
pub fn record(v: &Value) -> Value {
    let mut out = json!({});
    for key in ["state", "activity_id", "name", "ok", "reset"] {
        if let Some(value) = v.get(key) {
            out[key] = value.clone();
        }
    }
    for key in ["output", "text_delta"] {
        if let Some(value) = v[key].as_str() {
            out[key] = json!(value.chars().take(4000).collect::<String>());
        }
    }
    out
}
pub fn card_note(v: &Value) -> String {
    let mut text = String::new();
    if let Some(s) = v["highlight"]["text"].as_str() {
        text.push_str(&format!("\nHighlighted: {s}"));
    }
    if let Some(id) = v["coworker"]["id"].as_str() {
        text.push_str(&format!("\nCoworker: {id}\nUse /coworker upload ID [\"main.py\",\"data.csv\"], then /coworker start ID."));
    }
    if let Some(id) = v["research"]["id"].as_str() {
        text.push_str(&format!("\nResearch plan: {id}. Use /research show ID to review, then /research start ID or /research cancel ID."));
    }
    text
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn streamed_tool_names_update_the_existing_activity_without_ghost_rows() {
        let mut tasks = vec![];
        apply(
            &mut tasks,
            1,
            &json!({"state":"tool_preparing","activity_id":"one","name":""}),
        );
        assert_eq!(tasks[0].label, "Working");
        apply(
            &mut tasks,
            1,
            &json!({"state":"tool_preparing","activity_id":"one","name":"create_file"}),
        );
        assert_eq!(tasks[0].label, "Writing file");
        apply(
            &mut tasks,
            1,
            &json!({"state":"tool_done","activity_id":"one","ok":true}),
        );
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].label, "File saved");
    }
    #[test]
    fn missing_result_ids_complete_the_right_tool_without_merging_distinct_calls() {
        let mut tasks = vec![];
        for id in ["tool_0_0", "tool_0_1"] {
            apply(
                &mut tasks,
                1,
                &json!({"state":"tool_preparing","activity_id":id,"name":"web_search"}),
            );
            apply(&mut tasks, 1, &json!({"state":"tool","name":"web_search"}));
            apply(
                &mut tasks,
                1,
                &json!({"state":"tool_done","name":"web_search","ok":true,"output":"sources"}),
            );
        }
        assert_eq!(tasks.len(), 2);
        assert!(tasks.iter().all(|t| t.done && !t.failed));
    }
}
