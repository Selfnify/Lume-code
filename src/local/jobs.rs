use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const FOREGROUND_WAIT: Duration = Duration::from_secs(10);
const OUTPUT_LIMIT: usize = 65536;

struct Job {
    id: String,
    command: String,
    child: Child,
}

struct Spawned {
    id: String,
    child: Child,
    log: PathBuf,
}

struct Foreground(Option<Child>);

impl Drop for Foreground {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            kill_tree(child.id());
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

static JOBS: Mutex<Vec<Job>> = Mutex::new(Vec::new());

fn jobs() -> MutexGuard<'static, Vec<Job>> {
    JOBS.lock().unwrap_or_else(|e| e.into_inner())
}

fn spawn(root: &Path, script: &str, stdin: &str) -> Result<Spawned, String> {
    let id = format!(
        "job{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            % 1_000_000
    );
    let dir = std::env::temp_dir().join("lume-code-jobs");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let log = dir.join(format!("{id}.log"));
    let out = std::fs::File::create(&log).map_err(|e| e.to_string())?;
    let err = out.try_clone().map_err(|e| e.to_string())?;
    let mut command = if cfg!(windows) {
        let mut c = Command::new("powershell.exe");
        c.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", script]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", script]);
        c
    };
    command
        .current_dir(root)
        .stdin(if stdin.is_empty() {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .stdout(out)
        .stderr(err);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    if let Some(mut pipe) = child.stdin.take() {
        let input = stdin.as_bytes().to_vec();
        std::thread::spawn(move || {
            let _ = pipe.write_all(&input);
        });
    }
    Ok(Spawned { id, child, log })
}

fn register(id: String, script: &str, child: Child) {
    jobs().push(Job {
        id,
        command: script.chars().take(200).collect(),
        child,
    });
}

fn read_log(path: &Path) -> String {
    let Ok(file) = std::fs::File::open(path) else {
        return String::new();
    };
    let mut buf = Vec::new();
    let _ = file.take(OUTPUT_LIMIT as u64).read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).into_owned()
}

pub fn start(root: &Path, script: &str, stdin: &str) -> Result<Value, String> {
    let Spawned { id, child, log } = spawn(root, script, stdin)?;
    let result = json!({
        "background": true,
        "job": id.clone(),
        "output_file": log.display().to_string(),
        "status": "started; it keeps running until the user stops it with /stop"
    });
    register(id, script, child);
    Ok(result)
}

pub async fn run_foreground(root: &Path, script: &str, stdin: &str) -> Result<Value, String> {
    let Spawned { id, child, log } = spawn(root, script, stdin)?;
    let mut guard = Foreground(Some(child));
    let started = Instant::now();
    loop {
        let finished = match guard.0.as_mut() {
            Some(child) => child.try_wait().map_err(|e| e.to_string())?,
            None => None,
        };
        if let Some(status) = finished {
            guard.0 = None;
            return Ok(json!({
                "exit_code": status.code().unwrap_or(-1),
                "stdout": read_log(&log),
                "stderr": "",
                "output_limit": OUTPUT_LIMIT
            }));
        }
        if started.elapsed() >= FOREGROUND_WAIT {
            if let Some(child) = guard.0.take() {
                register(id.clone(), script, child);
            }
            return Ok(json!({
                "background": true,
                "job": id,
                "output_file": log.display().to_string(),
                "status": "still running after 10 seconds, so it moved to the background and keeps running. Check it with /ps; stop it with /stop."
            }));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

pub fn running_count() -> usize {
    jobs()
        .iter_mut()
        .map(|j| matches!(j.child.try_wait(), Ok(None)))
        .filter(|running| *running)
        .count()
}

pub fn list() -> Vec<String> {
    jobs()
        .iter_mut()
        .map(|j| {
            let state = match j.child.try_wait() {
                Ok(None) => "running",
                _ => "exited",
            };
            format!("{} · {} · {}", j.id, state, j.command)
        })
        .collect()
}

pub fn stop_all() -> usize {
    let mut stopped = 0;
    let drained: Vec<Job> = jobs().drain(..).collect();
    for job in drained {
        let mut child = job.child;
        if matches!(child.try_wait(), Ok(None)) {
            kill_tree(child.id());
            let _ = child.kill();
            let _ = child.wait();
            stopped += 1;
        }
    }
    stopped
}

fn kill_tree(pid: u32) {
    if cfg!(windows) {
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    } else {
        let _ = Command::new("kill")
            .args(["-TERM", "--", &format!("-{pid}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_commands_return_their_output_without_a_time_limit() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let root = std::env::temp_dir();
        let value = runtime
            .block_on(run_foreground(&root, "echo lume", ""))
            .unwrap();
        assert_eq!(value["exit_code"], 0);
        assert!(value["stdout"].as_str().unwrap().contains("lume"));
    }
}