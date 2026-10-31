use serde_json::{json, Value};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
};
async fn output<R: AsyncRead + Unpin>(mut pipe: R) -> String {
    let mut kept = vec![];
    let mut buf = [0u8; 4096];
    loop {
        match pipe.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let left = 65536usize.saturating_sub(kept.len());
                kept.extend_from_slice(&buf[..n.min(left)]);
            }
        }
    }
    String::from_utf8_lossy(&kept).into_owned()
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
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
    let mut command = if cfg!(windows) {
        let mut c = Command::new("powershell.exe");
        c.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", script]);
        c
    };
    command
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let _tree = match super::lifetime::ProcessTree::attach(&child) {
        Ok(v) => v,
        Err(e) => {
            let _ = child.kill().await;
            return Err(e);
        }
    };
    let stdout = tokio::spawn(output(child.stdout.take().unwrap()));
    let stderr = tokio::spawn(output(child.stderr.take().unwrap()));
    if let Some(mut stdin) = child.stdin.take() {
        let input = args["stdin"].as_str().unwrap_or("").as_bytes().to_vec();
        tokio::spawn(async move {
            let _ = stdin.write_all(&input).await;
        });
    }
    let code = match tokio::time::timeout(Duration::from_secs(60), child.wait()).await {
        Ok(result) => result.map_err(|e| e.to_string())?.code().unwrap_or(-1),
        Err(_) => {
            let _ = child.kill().await;
            stdout.abort();
            stderr.abort();
            return Err("Command reached the 60-second limit and was stopped. Inspect its saved work before retrying.".into());
        }
    };
    let out = tokio::time::timeout(Duration::from_secs(2), stdout)
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default();
    let err = tokio::time::timeout(Duration::from_secs(2), stderr)
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default();
    Ok(json!({"exit_code":code,"stdout":out,"stderr":err,"output_limit":65536}))
}
