use super::*;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
fn serve(
    mut socket: TcpStream,
    task: Value,
    done: Arc<AtomicBool>,
    result: Arc<Mutex<Value>>,
    complete: bool,
) {
    socket.set_nonblocking(false).unwrap();
    let mut reader = BufReader::new(socket.try_clone().unwrap());
    let mut first = String::new();
    reader.read_line(&mut first).unwrap();
    let route = first.split_whitespace().nth(1).unwrap_or("").to_owned();
    let post = first.starts_with("POST ");
    let mut size = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line.trim().is_empty() {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            if key.eq_ignore_ascii_case("content-length") {
                size = value.trim().parse().unwrap();
            }
        }
    }
    let mut bytes = vec![0; size];
    reader.read_exact(&mut bytes).unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or_default();
    if route == "/code/chat" {
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            )
            .unwrap();
        let pre =
            json!({"lume":{"state":"tool_preparing","activity_id":"tool_0_0","name":task["name"]}});
        let _ = write!(socket, "data: {pre}\n\n");
        let _ = socket.flush();
        for _ in 0..300 {
            if done.load(Ordering::Acquire) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let event = json!({"lume":{"state":"tool_done","activity_id":"tool_0_0","name":task["name"],"ok":true,"output":"saved"}});
        let _ = write!(
            socket,
            "data: {event}\n\ndata: {}\n\n",
            json!({"choices":[{"delta":{"content":"Finished."}}]})
        );
        if complete {
            let _ = socket.write_all(b"data: [DONE]\n\n");
            let _ = socket.flush();
            std::thread::sleep(Duration::from_millis(10));
        }
        return;
    }
    let response = if route == "/code/connect" {
        json!({"id":"code_fixture","epoch":0})
    } else if route == "/code/clients/code_fixture" && post {
        *result.lock().unwrap() = body["result"].clone();
        done.store(true, Ordering::Release);
        json!({"ok":true})
    } else if route == "/code/clients/code_fixture" {
        if done.load(Ordering::Acquire) {
            json!({"task":null})
        } else {
            json!({"task":task})
        }
    } else {
        json!({})
    };
    let data = response.to_string();
    let _=write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{data}",data.len());
}
#[test]
fn real_transport_streams_executes_local_files_denies_plan_and_detects_missing_done() {
    for (index, plan, complete, command) in [
        (0, false, true, false),
        (1, false, false, false),
        (2, true, true, false),
        (3, false, true, true),
    ] {
        let project = tempfile::tempdir().unwrap();
        let root = project.path().canonicalize().unwrap();
        let task = if command {
            json!({"id":"fixture_action","name":"run_command","arguments":{"command":"Set-Content should-not-exist.txt bad"}})
        } else {
            json!({"id":"fixture_action","name":"create_file","arguments":{"path":"hello.txt","content":"verified"}})
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let api = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let ended = Arc::new(AtomicBool::new(false));
        let result = Arc::new(Mutex::new(Value::Null));
        let (s, e, r) = (stop.clone(), ended.clone(), result.clone());
        let server = std::thread::spawn(move || {
            while !s.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((socket, _)) => {
                        let (t, d, r) = (task.clone(), e.clone(), r.clone());
                        std::thread::spawn(move || serve(socket, t, d, r, complete));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(10)),
                }
            }
        });
        let journal_dir = tempfile::tempdir().unwrap();
        let journal_path = journal_dir.path().join("active.json");
        let agent = Agent::spawn_at(
            root.clone(),
            Some(Connection {
                http: reqwest::Client::new(),
                api,
                token: "localhost-synthetic-session".into(),
            }),
            Some(journal_path.clone()),
        );
        agent
            .send
            .send(Turn {
                text: "Create a small test file".into(),
                plan,
            })
            .unwrap();
        let (mut text, mut noticed, mut activities, mut approved) =
            (String::new(), false, 0, false);
        loop {
            match agent
                .events
                .recv_timeout(Duration::from_secs(10))
                .expect("Stream must finish")
            {
                Event::Text(delta) => text.push_str(&delta),
                Event::Activity(_) => activities += 1,
                Event::Notice(n) => noticed |= n.contains("ended early"),
                Event::Approval { reply, .. } => {
                    approved = true;
                    let _ = reply.send(false);
                }
                Event::Done => break,
                Event::History(_) => {}
                Event::Directory(_) => {}
                Event::State(_) | Event::Picker(_) | Event::Usage(_) | Event::UsageRefresh => {}
            }
        }
        assert_eq!(text, "Finished.", "case {index}");
        assert!(activities >= 2);
        assert_eq!(noticed, !complete);
        let journal = crate::session_store::journal(&journal_path, &Value::Null);
        assert_eq!(
            journal["status"],
            if complete { "complete" } else { "interrupted" }
        );
        assert_eq!(journal["text"], "Finished.");
        assert!(journal["activities"].as_array().unwrap().len() >= 2);
        let mut restored = vec![];
        crate::session_store::restore(&journal_path, &Value::Null, &mut restored).unwrap();
        assert_eq!(restored[0]["content"], "Finished.");
        assert_eq!(approved, command);
        if plan || command {
            assert!(!root.join("hello.txt").exists());
            assert!(!root.join("should-not-exist.txt").exists());
            assert!(result.lock().unwrap()["error"].is_string());
        } else {
            assert_eq!(
                std::fs::read_to_string(root.join("hello.txt")).unwrap(),
                "verified"
            );
        }
        drop(agent);
        stop.store(true, Ordering::Release);
        server.join().unwrap();
    }
}
