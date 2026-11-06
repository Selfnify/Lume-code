use serde_json::Value;
use std::path::Path;

#[cfg(all(test, windows))]
mod tests {
    use super::run;
    use serde_json::json;
    use std::time::Duration;

    #[tokio::test]
    async fn native_command_runs_and_cancellation_terminates_descendants() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let value = run(&root, &json!({"command":"Write-Output 'native verified'"}))
            .await
            .unwrap();
        assert_eq!(value["exit_code"], 0);
        assert!(value["stdout"]
            .as_str()
            .unwrap()
            .contains("native verified"));
        let marker = root
            .join("child.txt")
            .display()
            .to_string()
            .replace('\'', "''");
        let script=format!("Start-Process -WindowStyle Hidden -FilePath powershell.exe -ArgumentList '-NoProfile','-NonInteractive','-Command',\"Start-Sleep -Seconds 2; Set-Content -LiteralPath '{marker}' child\"; Start-Sleep -Seconds 20");
        let task = tokio::spawn(async move { run(&root, &json!({"command":script})).await });
        tokio::time::sleep(Duration::from_millis(400)).await;
        task.abort();
        let _ = task.await;
        tokio::time::sleep(Duration::from_secs(3)).await;
        assert!(
            !temp.path().join("child.txt").exists(),
            "Cancelling must stop the child process before it writes"
        );
    }
}

pub async fn run(root: &Path, args: &Value) -> Result<Value, String> {
    let script = args["command"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 16000)
        .ok_or("Provide a finite shell command up to 16 KB.")?;
    let stdin = args["stdin"].as_str().unwrap_or("");
    if args["background"].as_bool() == Some(true) {
        return super::jobs::start(root, script, stdin);
    }
    super::jobs::run_foreground(root, script, stdin).await
}