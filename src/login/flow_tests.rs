use super::*;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
#[derive(Default)]
struct State {
    polls: usize,
    starts: usize,
    ack: bool,
    cancelled: bool,
    saved: Vec<String>,
    paths: Vec<String>,
}
fn handle(mut socket: TcpStream, state: Arc<Mutex<State>>, mode: &str) {
    socket.set_nonblocking(false).unwrap();
    let mut reader = BufReader::new(socket.try_clone().unwrap());
    let mut line = String::new();
    // Cancellation can close an accepted keep-alive connection before a request.
    if reader.read_line(&mut line).unwrap() == 0 {
        return;
    }
    let path = line.split_whitespace().nth(1).unwrap().to_owned();
    let mut len = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                len = value.trim().parse().unwrap();
            }
        }
    }
    let mut bytes = vec![0; len];
    reader.read_exact(&mut bytes).unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    let mut s = state.lock().unwrap();
    s.paths.push(path.clone());
    let token = "a".repeat(100);
    let proof = format!("lms_{}", "b".repeat(64));
    let value = match path.as_str() {
        "/code/login/start" => {
            s.starts += 1;
            json!({"public_token":token,"poll_secret":proof,"verification_uri":format!("{WEBSITE}/login?code_token={token}"),"expires_in":60,"interval":1})
        }
        "/code/login/poll" => {
            assert_eq!(body["poll_secret"], proof);
            s.polls += 1;
            if s.polls == 1 {
                json!({"state":"pending"})
            } else if mode == "denied" {
                json!({"state":"denied"})
            } else if mode == "retry_poll" && s.polls == 2 {
                return;
            } else {
                json!({"state":"approved","token":format!("lms_{}","c".repeat(64)),"user":{"email":"flow@test.invalid"}})
            }
        }
        "/code/login/ack" => {
            assert_eq!(
                s.saved.len(),
                1,
                "The credential must be saved before acknowledging delivery"
            );
            s.ack = true;
            json!({"state":"consumed"})
        }
        "/code/login/cancel" => {
            s.cancelled = true;
            json!({"state":"cancelled"})
        }
        _ => panic!("Unexpected endpoint"),
    };
    drop(s);
    let data = value.to_string();
    let written = write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{data}",data.len());
    if mode != "cancel" {
        written.unwrap();
    }
}
#[tokio::test]
async fn browser_login_polling_stores_only_approved_sessions_and_handles_recovery_denial_and_cancel(
) {
    for mode in ["success", "retry_poll", "denied", "cancel", "store_failure"] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let api = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(State::default()));
        let stop_server = Arc::new(AtomicBool::new(false));
        let (s, stop) = (state.clone(), stop_server.clone());
        let server = std::thread::spawn(move || {
            while !stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((socket, _)) => handle(socket, s.clone(), mode),
                    Err(_) => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        let (tx, rx) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let (cancel, store_state) = (cancelled.clone(), state.clone());
        let watcher = std::thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                if let Event::Ready(link) = event {
                    assert_eq!(link.token.len(), 100);
                    assert!(!link.url.contains("poll_secret"));
                    if mode == "cancel" {
                        cancel.store(true, Ordering::Release);
                    }
                    break;
                }
            }
        });
        let result = authenticate(&api, &tx, cancelled, move |_, token| {
            if mode == "store_failure" {
                return Err("Synthetic storage failure".into());
            }
            store_state.lock().unwrap().saved.push(token.into());
            Ok(())
        })
        .await;
        watcher.join().unwrap();
        stop_server.store(true, Ordering::Release);
        server.join().unwrap();
        let s = state.lock().unwrap();
        assert_eq!(s.starts, 1);
        assert!(s
            .paths
            .iter()
            .all(|p| !p.contains('?') && !p.contains("lms_")));
        match mode {
            "success" | "retry_poll" => {
                assert_eq!(result.unwrap(), Some("flow@test.invalid".into()));
                assert_eq!(s.saved.len(), 1);
                assert!(s.ack);
                if mode == "retry_poll" {
                    assert_eq!(s.polls, 3);
                }
            }
            "denied" => {
                assert!(result.is_err());
                assert!(s.saved.is_empty());
                assert!(!s.ack);
            }
            "cancel" => {
                assert_eq!(result.unwrap(), None);
                assert!(s.cancelled);
                assert!(s.saved.is_empty());
            }
            "store_failure" => {
                assert!(result.is_err());
                assert!(s.cancelled);
                assert!(!s.ack);
            }
            _ => unreachable!(),
        }
    }
}
