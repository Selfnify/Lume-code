use chat::tasker::{apply as apply_activity, Task};
mod settings;
mod clipboard;
mod actions;
#[path = "AI.rs"]
mod ai;
mod background;
mod chat;
mod connection;
mod local;
mod login;
mod session_catalog;
mod session_commands;
mod session_store;
mod sessions;
mod side_question;
mod usage_monitor;
#[cfg(test)]
mod workspace_tests;

use std::{
    io,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use chrono::Local;
use crossterm::{
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
        MouseButton, MouseEventKind,
    },
    execute,
};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    DefaultTerminal, Frame,
};

use background::{colors::active as theme, loading, ui};
use chat::{
    header,
    textbox::textbox::{self as tb, TextBox},
};

const EXIT_WINDOW: Duration = Duration::from_secs(2);

const SPINNER: [&str; 6] = ["·", "✢", "✳", "✶", "✻", "✽"];
const SPINNER_MS: u128 = 120;
const PAGE: usize = 10;
const WHEEL_STEP: usize = 3;
const BUTTON_BG: Color = Color::Rgb(200, 196, 190);
const WORDS: &[&str] = &[
    "Accomplishing…",
    "Actioning…",
    "Baking…",
    "Brewing…",
    "Calculating…",
    "Cerebrating…",
    "Churning…",
    "Clauding…",
    "Cogitating…",
    "Computing…",
    "Conjuring…",
    "Considering…",
    "Cooking…",
    "Crafting…",
    "Crunching…",
    "Deliberating…",
    "Doing…",
    "Elucidating…",
    "Enchanting…",
    "Envisioning…",
    "Finagling…",
    "Forging…",
    "Frolicking…",
    "Generating…",
    "Hatching…",
    "Ideating…",
    "Imagining…",
    "Marinating…",
    "Meandering…",
    "Moseying…",
    "Mulling…",
    "Noodling…",
    "Percolating…",
    "Pondering…",
    "Puttering…",
    "Puzzling…",
    "Ruminating…",
    "Simmering…",
    "Spinning…",
    "Stewing…",
    "Tinkering…",
    "Vibing…",
    "Wandering…",
    "Whirring…",
    "Wibbling…",
    "Working…",
];
enum Message {
    Tools(usize),
    User(String),
    Ai {
        text: String,
        summary: Option<Summary>,
    },
}

struct Summary {
    secs: u64,
    verb: &'static str,
    done_at: String,
}

struct Pending {
    text: String,
    started: Instant,
    word: &'static str,
}

struct DrawInfo {
    max_back: usize,
    button: Option<Rect>,
    hits: Vec<(Rect, Hit)>,
    back: usize,
}
enum Hit {
    Group(usize),
    Input(String),
}

fn main() -> io::Result<()> {
    background::colors::active::load_saved();
    settings::load();
    let args = std::env::args().collect::<Vec<_>>();
    match tokio::runtime::Runtime::new()?.block_on(connection::command(&args)) {
        Ok(true) => return Ok(()),
        Err(e) => {
            eprintln!("{e}");
            return Ok(());
        }
        _ => {}
    }
    if let Some(i) = args.iter().position(|a| a == "--project") {
        let path = args
            .get(i + 1)
            .ok_or_else(|| io::Error::other("Provide a project path."))?;
        std::env::set_current_dir(std::fs::canonicalize(path)?)?;
    }
    let explicit_login = args.get(1).is_some_and(|v| v == "login");
    if explicit_login {
        if !login::run()? {
            return Ok(());
        }
        println!("Signed in to Lume Code.");
        return Ok(());
    }
    match connection::Connection::load() {
        Ok(connection) => {
            match tokio::runtime::Runtime::new()?.block_on(connection.check_session()) {
                connection::SessionStatus::Active => {}
                connection::SessionStatus::Banned { reason, appeal } => {
                    connection::forget(&connection.api);
                    eprintln!(
                        "This Lume account is banned. Lume Code cannot sign in with this account."
                    );
                    if !reason.is_empty() {
                        eprintln!("Reason: {reason}");
                    }
                    if !appeal.is_empty() {
                        eprintln!("{appeal}");
                    }
                    return Ok(());
                }
                connection::SessionStatus::Expired => {
                    connection::forget(&connection.api);
                    if !login::run()? {
                        return Ok(());
                    }
                }
                connection::SessionStatus::Unavailable(message) => {
                    eprintln!("{message} Starting Lume Code anyway; online features may not be available.");
                }
            }
        }
        Err(_) => {
            if !login::run()? {
                return Ok(());
            }
        }
    }
    let mut terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture, EnableBracketedPaste)?;
    let result = run(&mut terminal);
    let _ = execute!(io::stdout(), DisableBracketedPaste, DisableMouseCapture);
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal) -> io::Result<()> {
    let agent = ai::Agent::new(std::env::current_dir()?.canonicalize()?);
    let mut tasks: Vec<Task> = vec![];
    let tasker = false;
    let mut turn_number = 0;
    let mut approval: Option<(String, tokio::sync::oneshot::Sender<bool>)> = None;
    let mut synthetic: std::collections::VecDeque<Event> = Default::default();
    let start = Instant::now();
    let mut textbox = TextBox::new();
    let mut messages: Vec<Message> = Vec::new();
    let mut pending: Option<Pending> = None;
    let mut exit_armed_at: Option<Instant> = None;
    let mut scroll_back: usize = 0;
    let mut side: Option<side_question::SideQuestion> = None;
    let monitor = usage_monitor::Monitor::new();
    let mut side_item = serde_json::Value::Null;
    let mut side_tick = Instant::now();
    let _ = agent.send.send(ai::Turn {
        text: "/__restore".into(),
        plan: false,
    });

    'main: loop {
        while let Ok(value) = monitor.events.try_recv() {
            textbox.usage = value;
        }
        if let Some(question) = side.as_ref() {
            let mut ended = false;
            loop {
                match question.events.try_recv() {
                    Ok(Ok(delta)) => textbox
                        .side_reply
                        .get_or_insert_with(String::new)
                        .push_str(&delta),
                    Ok(Err(error)) => {
                        side_item["status"] = serde_json::json!("interrupted");
                        textbox
                            .side_reply
                            .get_or_insert_with(String::new)
                            .push_str(&format!("\n\n{error}"));
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        ended = true;
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                }
            }
            if ended {
                side = None;
                if side_item["status"] != "interrupted" {
                    side_item["status"] = serde_json::json!("complete");
                }
                monitor.refresh();
            }
            side_item["answer"] = serde_json::json!(session_store::side_answer(
                textbox.side_reply.as_deref().unwrap_or(""),
                side_item["question"].as_str().unwrap_or("")
            ));
            if ended || side_tick.elapsed() > Duration::from_millis(500) {
                if let Some(path) = textbox.state["path"].as_str() {
                    if let Err(error) = session_store::save_side(
                        std::path::Path::new(path),
                        &textbox.state["epoch"],
                        side_item.clone(),
                    ) {
                        messages.push(Message::Ai {
                            text: error,
                            summary: None,
                        });
                    }
                }
                side_tick = Instant::now();
            }
        }
        if let Some(armed_at) = exit_armed_at {
            if armed_at.elapsed() >= EXIT_WINDOW {
                exit_armed_at = None;
            }
        }

        while let Ok(event) = agent.events.try_recv() {
            match event {
                ai::Event::History(history) => {
                    textbox.timeline = Default::default();
                    let current = match messages.last() {
                        Some(Message::User(text)) => Some(text.clone()),
                        _ => None,
                    };
                    tasks.clear();
                    let mut restored_turn = 0;
                    messages.clear();
                    for v in history {
                        if v["role"] == "user" {
                            restored_turn += 1;
                        }
                        if let Some(text) = v["content"].as_str() {
                            if v["role"] == "user" {
                                messages.push(Message::User(text.into()));
                            } else {
                                restore_reply(
                                    &mut messages,
                                    &mut textbox,
                                    &mut tasks,
                                    restored_turn,
                                    text,
                                    &v["activities"],
                                );
                            }
                        }
                    }
                    turn_number = restored_turn + 1;
                    chat::tasker::finish(&mut tasks);
                    for group in &mut textbox.timeline.groups {
                        group.settled = true;
                        group.restored = true;
                    }
                    if let Some(text) = current {
                        if !matches!(
                            text.split_whitespace().next(),
                            Some("/clear" | "/new" | "/resume")
                        ) {
                            messages.push(Message::User(text));
                        }
                    }
                }
                ai::Event::Directory(path) => {
                    let _ = std::env::set_current_dir(path);
                }
                ai::Event::Picker(entries) => {
                    textbox.picker = Some(chat::picker::Picker::new(entries));
                }
                ai::Event::State(value) => {
                    textbox.state = value;
                    if side.is_none() {
                        textbox.side_reply = textbox.state["side"]["items"]
                            .as_array()
                            .and_then(|v| v.last())
                            .map(session_store::side_display);
                    }
                }
                ai::Event::Usage(value) => {
                    textbox.usage = value;
                }
                ai::Event::UsageRefresh => {
                    monitor.refresh();
                }
                ai::Event::Text(delta) => {
                    if !delta.trim().is_empty() {
                        textbox.timeline.current = None;
                    }
                    if let Some(p) = pending.as_mut() {
                        p.text.push_str(&delta);
                    }
                }
                ai::Event::Notice(text) => {
                    if let Some(p) = pending.as_mut() {
                        p.text.push_str(&format!("\n\n{text}"));
                    } else {
                        messages.push(Message::Ai {
                            text,
                            summary: None,
                        });
                    }
                }
                ai::Event::Approval { label, reply } => approval = Some((label, reply)),
                ai::Event::Activity(v) => {
                    apply_activity(&mut tasks, turn_number, &v);
                    sync_timeline(&mut textbox, &mut messages, &mut pending, &tasks);
                    if let Some(p) = pending.as_mut() {
                        p.text.push_str(&chat::tasker::card_note(&v));
                        if v["state"] == "mode_change_requested" {
                            p.text.push_str("\nLume requests Edit mode. Review the plan, press Shift-Tab to select Edit, then send Continue.");
                        }
                    }
                }
                ai::Event::Done => {
                    if pending
                        .as_ref()
                        .is_some_and(|p| p.started.elapsed() >= Duration::from_secs(10))
                    {
                        if crate::settings::bell() { print!("\x07"); }
                        let _ = std::io::Write::flush(&mut std::io::stdout());
                    }
                    textbox.timeline.current = None;
                    for group in &mut textbox.timeline.groups {
                        group.settled = true;
                    }
                    monitor.refresh();
                    approval = None;
                    if let Some(p) = pending.take().filter(|p| !p.text.is_empty()) {
                        messages.push(Message::Ai {
                            text: p.text,
                            summary: Some(Summary {
                                secs: p.started.elapsed().as_secs(),
                                verb: "Worked",
                                done_at: Local::now().format("%-I:%M %p").to_string(),
                            }),
                        });
                    }
                    for t in &mut tasks {
                        if !t.done {
                            t.failed = true;
                            t.done = true;
                        }
                    }
                }
            }
        }
        let t = start.elapsed().as_secs_f64();
        let done = loading::is_done(t);
        let exit_armed = exit_armed_at.is_some();
        let working = pending.is_some();
        textbox.set_working(working || side.is_some());
        let spinner: &'static str =
            SPINNER[(start.elapsed().as_millis() / SPINNER_MS) as usize % SPINNER.len()];

        let mut info = DrawInfo {
            max_back: 0,
            button: None,
            hits: vec![],
            back: 0,
        };
        terminal.draw(|frame| {
            if done {
                info = draw_screen(
                    frame,
                    &textbox,
                    &messages,
                    pending.as_ref(),
                    spinner,
                    exit_armed,
                    scroll_back,
                    &tasks,
                    tasker,
                    approval.as_ref().map(|a| a.0.as_str()),
                );
            } else {
                loading::draw(frame, t);
            }
            background::colors::adapt_buffer(frame.buffer_mut());
        })?;
        let last_max = info.max_back;
        let button = info.button;
        scroll_back = info.back;
        let hits = info.hits;

        if pending.is_none() && approval.is_none() && synthetic.is_empty() && !textbox.queued.is_empty() {
            let next = textbox.queued.remove(0);
            textbox.set_input(next);
            synthetic.push_back(Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)));
        }
        if !synthetic.is_empty() || event::poll(Duration::from_millis(16))? {
            let burst = match synthetic.pop_front() {
                Some(event) => vec![event],
                None => read_burst()?,
            };
            let presses = burst
                .iter()
                .filter(|event| match event {
                    Event::Key(key) => key.kind == KeyEventKind::Press,
                    Event::Paste(_) => true,
                    _ => false,
                })
                .count();
            let in_paste = presses > 1;
            for event in burst {
                match event {
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        scroll_back = (scroll_back + WHEEL_STEP).min(last_max);
                    }
                    MouseEventKind::ScrollDown => {
                        scroll_back = scroll_back.saturating_sub(WHEEL_STEP);
                    }
                    MouseEventKind::Down(MouseButton::Left) => {
                        if textbox.picker.is_none()
                            && textbox.commands().is_empty()
                            && textbox.directory_choices().is_empty()
                        {
                            if let Some((_, action)) = hits
                                .iter()
                                .find(|(r, _)| contains(*r, mouse.column, mouse.row))
                            {
                                match action {
                                    Hit::Group(i) => {
                                        if let Some(g) = textbox.timeline.groups.get_mut(*i) {
                                            g.open = !g.open;
                                        }
                                    }
                                    Hit::Input(text) => textbox.set_input(text.clone()),
                                };
                                continue;
                            }
                        }
                        if button.map_or(false, |r| contains(r, mouse.column, mouse.row)) {
                            scroll_back = 0;
                        }
                    }
                    _ => {}
                },
                Event::Paste(text) if approval.is_none() => {
                    textbox.insert_paste(&text);
                }
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if approval.is_some() {
                        if matches!(
                            key.code,
                            KeyCode::Char('y')
                                | KeyCode::Char('Y')
                                | KeyCode::Char('n')
                                | KeyCode::Char('N')
                                | KeyCode::Esc
                        ) {
                            let (_, reply) = approval.take().unwrap();
                            let _ = reply
                                .send(matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y')));
                        }
                        continue;
                    }
                    if key.code == KeyCode::Char('o')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        textbox.timeline.full = !textbox.timeline.full;
                        continue;
                    }
                    if key.code == KeyCode::Char('t')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        textbox.close_overlay();
                        textbox.timeline.latest();
                        scroll_back = usize::MAX;
                        continue;
                    }
                    if key.code == KeyCode::Char('r')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                        && !working
                        && session_store::recovery_text(&textbox.state["recovery"]).is_some()
                    {
                        textbox.set_input(session_store::CONTINUE.into());
                        continue;
                    }
                    if key.code == KeyCode::Esc {
                        if textbox.close_overlay() {
                            continue;
                        }
                        if pending.is_none() && side.is_some() {
                            side = None;
                            side_item["status"] = serde_json::json!("interrupted");
                            side_item["answer"] = serde_json::json!(session_store::side_answer(
                                textbox.side_reply.as_deref().unwrap_or(""),
                                side_item["question"].as_str().unwrap_or("")
                            ));
                            if let Some(path) = textbox.state["path"].as_str() {
                                if let Err(error) = session_store::save_side(
                                    std::path::Path::new(path),
                                    &textbox.state["epoch"],
                                    side_item.clone(),
                                ) {
                                    messages.push(Message::Ai {
                                        text: error,
                                        summary: None,
                                    });
                                }
                            }
                            continue;
                        }
                        agent.cancel();
                        if pending.is_some() {
                            continue;
                        } else {
                            continue;
                        }
                    } else if key.code == KeyCode::Char('d')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        if exit_armed {
                            break 'main;
                        }
                        exit_armed_at = Some(Instant::now());
                    } else {
                        exit_armed_at = None;
                        match key.code {
                            KeyCode::PageUp => {
                                scroll_back = (scroll_back + PAGE).min(last_max);
                            }
                            KeyCode::PageDown => {
                                scroll_back = scroll_back.saturating_sub(PAGE);
                            }
                            KeyCode::End => {
                                scroll_back = 0;
                            }
                            KeyCode::Enter if in_paste && approval.is_none() => {
                                textbox.insert_paste("\n");
                            }
                            KeyCode::Enter
                                if working
                                    && approval.is_none()
                                    && !textbox.input().trim().is_empty()
                                    && !textbox.input().starts_with("/btw ") =>
                            {
                                let text = textbox.take_input();
                                textbox.queued.push(text.trim().to_string());
                            }
                            _ => {
                                if done
                                    && !(working
                                        && key.code == KeyCode::Enter
                                        && !textbox.input().starts_with("/btw "))
                                {
                                    if let Some(message) = textbox.handle_key(key) {
                                        if message == "/btw" || message.starts_with("/btw ") {
                                            let question =
                                                message.strip_prefix("/btw").unwrap().trim();
                                            if question.is_empty() {
                                                messages.push(Message::Ai {
                                                    text:
                                                        "Use /btw followed by your side question."
                                                            .into(),
                                                    summary: None,
                                                });
                                                continue;
                                            }
                                            if side.is_some() {
                                                messages.push(Message::Ai{text:"A side question is already running. Wait for it to finish before starting another.".into(),summary:None});
                                                continue;
                                            }
                                            let mut context = vec![];
                                            let mut remaining = 16000usize;
                                            for message in messages.iter().rev().take(6) {
                                                let (role, text) = match message {
                                                    Message::User(t) => ("user", t),
                                                    Message::Ai { text, .. } => ("assistant", text),
                                                    Message::Tools(_) => continue,
                                                };
                                                let snippet = text
                                                    .chars()
                                                    .take(remaining.min(4000))
                                                    .collect::<String>();
                                                remaining = remaining
                                                    .saturating_sub(snippet.chars().count());
                                                if !snippet.is_empty() {
                                                    context.push(serde_json::json!({"role":role,"content":snippet}));
                                                }
                                            }
                                            if textbox.state["path"].as_str().is_none() {
                                                messages.push(Message::Ai{text:"Your session is still loading. Try the side question again in a moment.".into(),summary:None});
                                                continue;
                                            }
                                            side_item = serde_json::json!({"id":nanos().to_string()+&session_store::now().to_string(),"question":question,"answer":"","status":"running","created_at":session_store::now()});
                                            if let Some(path) = textbox.state["path"].as_str() {
                                                if let Err(error) = session_store::save_side(
                                                    std::path::Path::new(path),
                                                    &textbox.state["epoch"],
                                                    side_item.clone(),
                                                ) {
                                                    messages.push(Message::Ai {
                                                        text: error,
                                                        summary: None,
                                                    });
                                                    continue;
                                                }
                                            }
                                            context.reverse();
                                            textbox.side_reply =
                                                Some(format!("**{}**\n\n", question));
                                            side = Some(side_question::SideQuestion::start(
                                                std::env::current_dir()?.display().to_string(),
                                                question.into(),
                                                context,
                                            ));
                                            continue;
                                        }
                                        if message == "/tools-full" {
                                            textbox.timeline.full = !textbox.timeline.full;
                                            continue;
                                        }
                                        if message == "/copy" {
                                            let last = messages.iter().rev().find_map(|m| match m {
                                                Message::Ai { text, .. } => Some(text.clone()),
                                                _ => None,
                                            });
                                            let note = match last {
                                                Some(text) => match clipboard::copy(&text) {
                                                    Ok(()) => "Copied the last reply.".to_string(),
                                                    Err(error) => format!("Could not copy: {error}"),
                                                },
                                                None => "There is no reply to copy yet.".to_string(),
                                            };
                                            messages.push(Message::Ai { text: note, summary: None });
                                            continue;
                                        }
                                        if message == "/tasks" {
                                            textbox.close_overlay();
                                            textbox.timeline.latest();
                                            scroll_back = usize::MAX;
                                            continue;
                                        }
                                        if message == "/side-history" {
                                            let items = if let Some(path) =
                                                textbox.state["path"].as_str()
                                            {
                                                session_store::extra(
                                                    std::path::Path::new(path),
                                                    &textbox.state["epoch"],
                                                )
                                            } else {
                                                serde_json::Value::Null
                                            };
                                            textbox.side_reply = Some(
                                                items["items"]
                                                    .as_array()
                                                    .map(|v| {
                                                        v.iter()
                                                            .map(|item| {
                                                                format!(
                                                                    "### {} · {}\n\n{}",
                                                                    item["question"]
                                                                        .as_str()
                                                                        .unwrap_or("Side question"),
                                                                    if item["status"] == "complete"
                                                                    {
                                                                        "Finished"
                                                                    } else {
                                                                        "Interrupted"
                                                                    },
                                                                    item["answer"]
                                                                        .as_str()
                                                                        .unwrap_or(
                                                                            "No answer saved yet."
                                                                        )
                                                                )
                                                            })
                                                            .collect::<Vec<_>>()
                                                            .join("\n\n")
                                                    })
                                                    .filter(|s| !s.is_empty())
                                                    .unwrap_or(
                                                        "No saved side questions in this session."
                                                            .into(),
                                                    ),
                                            );
                                            continue;
                                        }
                                        if side.is_some()
                                            && matches!(
                                                message.split_whitespace().next(),
                                                Some("/clear" | "/new" | "/cd" | "/resume")
                                            )
                                        {
                                            messages.push(Message::Ai{text:"Finish or stop the side question before switching sessions.".into(),summary:None});
                                            continue;
                                        }
                                        if message == "/help" {
                                            textbox.shortcuts = true;
                                            continue;
                                        }
                                        if message == "/logout" {
                                            let result = tokio::runtime::Runtime::new()?.block_on(
                                                connection::command(&[
                                                    "lume-code".into(),
                                                    "logout".into(),
                                                ]),
                                            );
                                            if result.is_ok() {
                                                break 'main;
                                            }
                                            messages.push(Message::Ai{text:"Could not finish signing out. Try lume-code logout after closing the app.".into(),summary:None});
                                            continue;
                                        }
                                        turn_number += 1;
                                        let _ = agent.send.send(ai::Turn {
                                            text: message.clone(),
                                            plan: textbox.plan(),
                                        });
                                        messages.push(Message::User(message));
                                        scroll_back = 0;
                                        pending = Some(Pending {
                                            text: String::new(),
                                            started: Instant::now(),
                                            word: pick_word(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
            }
        }
    }
    Ok(())
}

fn contains(r: Rect, col: u16, row: u16) -> bool {
    col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
}

fn nanos() -> usize {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0) as usize
}

fn pick_word() -> &'static str {
    WORDS[nanos() % WORDS.len()]
}

fn visible_chars(p: &Pending) -> usize {
    p.text.chars().count()
}

fn partial(p: &Pending) -> String {
    p.text.chars().take(visible_chars(p)).collect()
}
fn sync_timeline(
    textbox: &mut TextBox,
    messages: &mut Vec<Message>,
    pending: &mut Option<Pending>,
    tasks: &[Task],
) {
    let unseen = tasks.iter().any(|t| {
        !textbox
            .timeline
            .groups
            .iter()
            .any(|g| g.tasks.iter().any(|v| v.id == t.id))
    });
    if unseen {
        if let Some(p) = pending.as_mut() {
            if !p.text.is_empty() {
                messages.push(Message::Ai {
                    text: std::mem::take(&mut p.text),
                    summary: None,
                });
                textbox.timeline.current = None;
            }
        }
    }
    if let Some(id) = textbox.timeline.sync(tasks) {
        messages.push(Message::Tools(id));
    }
}
fn restore_reply(
    messages: &mut Vec<Message>,
    textbox: &mut TextBox,
    tasks: &mut Vec<Task>,
    turn: usize,
    text: &str,
    events: &serde_json::Value,
) {
    let mut cursor = 0;
    for event in events.as_array().into_iter().flatten() {
        let mut at = (event["at"].as_u64().unwrap_or(0) as usize)
            .min(text.len())
            .max(cursor);
        while !text.is_char_boundary(at) {
            at -= 1;
        }
        if at > cursor {
            messages.push(Message::Ai {
                text: text[cursor..at].into(),
                summary: None,
            });
            textbox.timeline.current = None;
            cursor = at;
        }
        apply_activity(tasks, turn, event);
        sync_timeline(textbox, messages, &mut None, tasks);
    }
    if cursor < text.len() {
        messages.push(Message::Ai {
            text: text[cursor..].into(),
            summary: None,
        });
    }
    textbox.timeline.current = None;
}

fn draw_screen(
    frame: &mut Frame,
    textbox: &TextBox,
    messages: &[Message],
    pending: Option<&Pending>,
    spinner: &'static str,
    exit_armed: bool,
    scroll_back: usize,
    tasks: &[Task],
    tasker: bool,
    approval: Option<&str>,
) -> DrawInfo {
    ui::draw(frame);

    let area = frame.area();
    let mut banners = vec![];
    if let Some(label) = usage_monitor::label(&textbox.usage) {
        banners.push((label, Hit::Input("/usage".into())));
    }
    if let Some(label) = session_store::recovery_text(&textbox.state["recovery"])
        .filter(|_| pending.is_none())
    {
        banners.push((label, Hit::Input(session_store::CONTINUE.into())));
    }
    let banner_height = banners
        .iter()
        .map(|(s, _)| {
            Paragraph::new(s.as_str())
                .wrap(Wrap { trim: false })
                .line_count(area.width)
                .min(3) as u16
        })
        .sum::<u16>();
    let chunks = Layout::vertical([
        Constraint::Length(header::height(area.width)),
        Constraint::Min(1),
        Constraint::Length(banner_height + 1),
        Constraint::Length(textbox.height()),
        Constraint::Length(1),
    ])
    .split(area);

    header::draw(frame, chunks[0]);

    let mut lines: Vec<Line> = vec![Line::raw("")];
    let mut headers = vec![];
    for message in messages {
        if let Message::Tools(id) = message {
            headers.push((lines.len(), *id));
            textbox.timeline.render(*id, &mut lines);
        } else {
            push_message(&mut lines, message);
        }
    }
    if let Some(p) = pending {
        push_ai(&mut lines, partial(p));
    }
    if let Some(text) = &textbox.side_reply {
        lines.push(Line::raw(""));
        push_ai(&mut lines, format!("### Side question\n\n{text}"));
    }

    if textbox.timeline.groups.is_empty() && !tasks.is_empty() {
        let mut timeline = chat::timeline::Timeline::default();
        timeline.sync(tasks);
        timeline.groups[0].open = tasker;
        timeline.render(0, &mut lines);
    }
    let height = chunks[1].height as usize;
    let rendered = Paragraph::new(lines.clone())
        .wrap(Wrap { trim: false })
        .line_count(chunks[1].width);
    let max_back = rendered.saturating_sub(height);
    let positions = headers
        .iter()
        .map(|(line, id)| {
            (
                Paragraph::new(lines[..*line].to_vec())
                    .wrap(Wrap { trim: false })
                    .line_count(chunks[1].width),
                *id,
            )
        })
        .collect::<Vec<_>>();
    let back = if scroll_back == usize::MAX {
        max_back.saturating_sub(
            positions
                .last()
                .map(|v| v.0)
                .unwrap_or(max_back)
                .min(max_back),
        )
    } else {
        scroll_back.min(max_back)
    };
    let top = (max_back - back).min(u16::MAX as usize) as u16;
    let mut hits = positions
        .iter()
        .filter_map(|(row, id)| {
            let y = *row as i64 - top as i64;
            if y >= 0 && y < chunks[1].height as i64 {
                Some((
                    Rect::new(chunks[1].x, chunks[1].y + y as u16, chunks[1].width, 1),
                    Hit::Group(*id),
                ))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let history = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((top, 0))
        .style(Style::default().bg(theme::bg()));
    frame.render_widget(history, chunks[1]);

    let mut banner_y = chunks[2].y;
    for (text, action) in banners {
        let height = (Paragraph::new(text.as_str())
            .wrap(Wrap { trim: false })
            .line_count(chunks[2].width) as u16)
            .min(3)
            .min(chunks[2].bottom().saturating_sub(banner_y));
        let rect = Rect::new(chunks[2].x, banner_y, chunks[2].width, height);
        frame.render_widget(
            Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .style(Style::default().fg(theme::accent()).bg(theme::bg())),
            rect,
        );
        hits.push((rect, action));
        banner_y += height;
    }
    if let Some(p) = pending {
        let spinner_line = Line::from(vec![
            Span::styled(spinner, Style::default().fg(theme::accent())),
            Span::styled(
                format!(
                    " {} · {}s · {} tools · esc to interrupt · Ctrl-T tasks",
                    tasks
                        .iter()
                        .rev()
                        .find(|t| !t.done)
                        .map(|t| t.label.as_str())
                        .unwrap_or(p.word),
                    p.started.elapsed().as_secs(),
                    tasks.len()
                ),
                Style::default().fg(theme::text()),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(spinner_line).style(Style::default().bg(theme::bg())),
            Rect::new(
                chunks[2].x,
                chunks[2].bottom().saturating_sub(1),
                chunks[2].width,
                1,
            ),
        );
    }

    tb::draw(frame, chunks[3], textbox);
    tb::draw_status(frame, chunks[4], textbox, exit_armed);
    let commands = textbox.commands();
    if !commands.is_empty() {
        let height = (commands.len().min(5) as u16).min(chunks[3].y);
        chat::commands::draw(
            frame,
            Rect::new(chunks[3].x, chunks[3].y - height, chunks[3].width, height),
            &commands,
            textbox.selected,
        );
    }
    if let Some(label) = approval {
        let body: Vec<&str> = label.lines().collect();
        let height = ((body.len() + 4) as u16).min(chunks[3].y);
        let area = Rect::new(chunks[3].x, chunks[3].y - height, chunks[3].width, height);
        let mut modal: Vec<Line> = body
            .iter()
            .map(|line| Line::styled(clean(line), Style::default().fg(theme::text())))
            .collect();
        modal.push(Line::raw(""));
        modal.push(Line::styled(
            "[Y] Approve   [N] Deny",
            Style::default()
                .fg(theme::accent())
                .add_modifier(ratatui::style::Modifier::BOLD),
        ));
        frame.render_widget(ratatui::widgets::Clear, area);
        frame.render_widget(
            Paragraph::new(modal)
                .wrap(Wrap { trim: false })
                .block(
                    ratatui::widgets::Block::default()
                        .borders(ratatui::widgets::Borders::ALL)
                        .border_type(ratatui::widgets::BorderType::Rounded)
                        .border_style(Style::default().fg(theme::accent()))
                        .title(" Approval required "),
                )
                .style(Style::default().bg(theme::bg())),
            area,
        );
    }
    if !textbox.queued.is_empty() {
        let shown: Vec<Line> = textbox
            .queued
            .iter()
            .take(3)
            .map(|text| {
                Line::styled(
                    format!("↳ queued: {}", clean(text)),
                    Style::default().fg(theme::text()),
                )
            })
            .collect();
        let height = (shown.len() as u16).min(chunks[3].y);
        frame.render_widget(
            Paragraph::new(shown).style(Style::default().bg(theme::bg())),
            Rect::new(chunks[3].x, chunks[3].y - height, chunks[3].width, height),
        );
    }
    if !textbox.mention_matches().is_empty() {
        let names = textbox.mention_matches().len() as u16;
        let height = (names + 2).min(chunks[3].y);
        tb::draw_mention_menu(
            frame,
            Rect::new(chunks[3].x, chunks[3].y - height, chunks[3].width, height),
            textbox,
        );
    }
    if textbox.resume_choice.is_some() {
        let height = 4u16.min(chunks[3].y);
        tb::draw_resume_choice(
            frame,
            Rect::new(chunks[3].x, chunks[3].y - height, chunks[3].width, height),
            textbox,
        );
    }
    if textbox.settings_open() {
        let height = ((crate::settings::ITEMS.len() as u16) + 2).min(chunks[3].y);
        tb::draw_settings_menu(
            frame,
            Rect::new(chunks[3].x, chunks[3].y - height, chunks[3].width, height),
            textbox,
        );
    }
    if textbox.theme_menu_open() {
        let height = ((theme::count() as u16) + 2).min(chunks[3].y);
        tb::draw_theme_menu(
            frame,
            Rect::new(chunks[3].x, chunks[3].y - height, chunks[3].width, height),
            textbox,
        );
    }
    if textbox.shortcuts {
        use ratatui::widgets::Clear;
        let height = 20.min(chunks[3].y);
        let area = Rect::new(chunks[3].x, chunks[3].y - height, chunks[3].width, height);
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(chat::commands::SHORTCUTS)
                .style(Style::default().fg(theme::text()).bg(theme::bg())),
            area,
        );
    }

    let directories = textbox.directory_choices();
    if !directories.is_empty() {
        use ratatui::widgets::Clear;
        let height = (directories.len().min(5) as u16).min(chunks[3].y);
        let rect = Rect::new(chunks[3].x, chunks[3].y - height, chunks[3].width, height);
        frame.render_widget(Clear, rect);
        let start = textbox.selected.saturating_sub(4);
        let list = directories
            .iter()
            .enumerate()
            .skip(start)
            .take(5)
            .map(|(i, p)| {
                Line::styled(
                    format!("{} {p}", if i == textbox.selected { "❯" } else { " " }),
                    Style::default().fg(if i == textbox.selected {
                        Color::Rgb(35, 95, 190)
                    } else {
                        theme::text()
                    }),
                )
            })
            .collect::<Vec<_>>();
        frame.render_widget(
            Paragraph::new(list).style(Style::default().bg(theme::bg())),
            rect,
        );
    }
    if let Some(picker) = &textbox.picker {
        let height = chunks[3].y.saturating_sub(chunks[0].bottom()).min(18);
        picker.draw(
            frame,
            Rect::new(chunks[1].x, chunks[3].y - height, chunks[1].width, height),
        );
    }
    let button = if back > 0 {
        let rect = Rect {
            x: chunks[3].x + chunks[3].width.saturating_sub(7),
            y: chunks[2].y,
            width: 5,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  ↓  ",
                Style::default().fg(theme::text()).bg(BUTTON_BG),
            ))),
            rect,
        );
        Some(rect)
    } else {
        None
    };

    DrawInfo {
        max_back,
        button,
        hits,
        back,
    }
}

fn push_message(lines: &mut Vec<Line<'static>>, message: &Message) {
    match message {
        Message::User(text) => {
            for (index, line) in text.lines().enumerate() {
                lines.push(Line::styled(
                    format!("{}{}", if index == 0 { "> " } else { "  " }, clean(line)),
                    Style::default().fg(theme::text()),
                ));
            }
            lines.push(Line::raw(""));
        }
        Message::Tools(_) => {}
        Message::Ai { text, summary } => {
            push_ai(lines, text.clone());
            if let Some(s) = summary {
                lines.push(Line::raw(""));
                lines.push(Line::from(vec![
                    Span::styled("✻ ", Style::default().fg(theme::accent())),
                    Span::styled(
                        format!("{} for {}s · done {}", s.verb, s.secs, s.done_at),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
            lines.push(Line::raw(""));
        }
    }
}

fn push_ai(lines: &mut Vec<Line<'static>>, text: String) {
    for (index, mut line) in chat::markdown::render(&text).into_iter().enumerate() {
        line.spans.insert(
            0,
            Span::styled(
                if index == 0 { "● " } else { "  " },
                Style::default().fg(Color::DarkGray),
            ),
        );
        lines.push(line);
    }
}
fn clean(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || *c == '\t')
        .collect()
}
#[cfg(test)]
mod ui_tests {
    use super::*;
    fn screen(expanded: bool, approval: bool) -> String {
        let backend = ratatui::backend::TestBackend::new(120, 40);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let pending = Pending {
            text: "A live response".into(),
            started: Instant::now(),
            word: "Working",
        };
        let tasks = vec![
            Task {
                command: String::new(),
                id: "one".into(),
                name: "create_file".into(),
                label: "Writing file".into(),
                done: false,
                failed: false,
                detail: "LIVE FILE CONTENTS".into(),
            },
            Task {
                command: String::new(),
                id: "two".into(),
                name: "read_file".into(),
                label: "Read file".into(),
                done: true,
                failed: false,
                detail: "Finished file".into(),
            },
        ];
        terminal
            .draw(|f| {
                draw_screen(
                    f,
                    &TextBox::new(),
                    &[],
                    Some(&pending),
                    "✢",
                    false,
                    0,
                    &tasks,
                    expanded,
                    if approval {
                        Some("Approve a local command?\n[Y] Approve  [N] Deny")
                    } else {
                        None
                    },
                );
            })
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>()
    }
    #[test]
    fn tasker_is_live_ungrouped_and_approval_is_visible() {
        let shown = screen(false, false);
        assert!(shown.contains("● Ran a tool"));
        assert!(shown.contains("LIVE FILE CONTENTS"));
        let approval = screen(true, true);
        assert!(approval.contains("[Y] Approve"));
        assert!(approval.contains("● Ran a tool"));
        assert!(approval.contains("Finished file"));
    }
    #[test]
    fn terminal_control_sequences_are_removed() {
        assert_eq!(clean("safe\u{1b}[31m\rtext"), "safe[31mtext");
    }

    #[test]
    fn formatted_history_and_live_replies_render_on_separate_rows() {
        use ratatui::style::Modifier;
        for width in [48, 120] {
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, 40)).unwrap();
            let messages = vec![
                Message::User("hello\nsecond user line".into()),
                Message::Ai {
                    text: "## Capabilities\n\n- **Coding**\n- Research\n\nA separate paragraph."
                        .into(),
                    summary: None,
                },
            ];
            let pending = Pending {
                text: "**Live reply**\n\nNext paragraph".into(),
                started: Instant::now(),
                word: "Working",
            };
            terminal
                .draw(|frame| {
                    draw_screen(
                        frame,
                        &TextBox::new(),
                        &messages,
                        Some(&pending),
                        "✢",
                        false,
                        0,
                        &[],
                        false,
                        None,
                    );
                })
                .unwrap();
            let buffer = terminal.backend().buffer();
            let rows: Vec<String> = buffer
                .content
                .chunks(width as usize)
                .map(|row| row.iter().map(|cell| cell.symbol()).collect())
                .collect();
            let row = |text: &str| rows.iter().position(|line| line.contains(text)).unwrap();
            assert!(row("hello") < row("second user line"));
            assert!(row("Capabilities") < row("• Coding"));
            assert!(row("• Coding") < row("• Research"));
            assert!(row("• Research") < row("A separate paragraph."));
            assert!(row("Live reply") < row("Next paragraph"));
            assert!(!rows.join("\n").contains("**"));
            let coding_row = row("• Coding");
            let coding_x = buffer
                .content
                .chunks(width as usize)
                .nth(coding_row)
                .unwrap()
                .iter()
                .position(|c| c.symbol() == "C")
                .unwrap();
            assert!(buffer[(coding_x as u16, coding_row as u16)]
                .modifier
                .contains(Modifier::BOLD));
        }
    }
}

fn read_burst() -> std::io::Result<Vec<Event>> {
    let mut events = vec![event::read()?];
    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_millis(120) && events.len() < 5000 {
        if event::poll(Duration::from_millis(12))? {
            events.push(event::read()?);
        } else {
            break;
        }
    }
    Ok(events)
}