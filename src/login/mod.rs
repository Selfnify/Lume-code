mod flow;
mod view;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
        MouseEventKind,
    },
    execute,
};
use std::{
    io,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
#[derive(Clone, Copy)]
enum Phase {
    Idle,
    Starting,
    Waiting,
    Error,
    Connected,
}
struct Model {
    phase: Phase,
    status: String,
    link: Option<flow::Link>,
    link_started: Option<Instant>,
    scroll: u16,
}
impl Default for Model {
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            status: "Ready when you are.".into(),
            link: None,
            link_started: None,
            scroll: 0,
        }
    }
}
fn open_browser(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", url])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Command::new(if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        })
        .arg(url)
        .spawn()
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}
fn copy(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        clipboard_win::set_clipboard_string(url).map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = url;
        Err("Select and copy the URL shown above.".into())
    }
}
fn contains(r: ratatui::layout::Rect, x: u16, y: u16) -> bool {
    x >= r.x && x < r.right() && y >= r.y && y < r.bottom()
}
pub fn run() -> io::Result<bool> {
    let api = crate::connection::api().map_err(io::Error::other)?;
    let mut terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;
    let result = screen(&mut terminal, api);
    let _ = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}
fn screen(terminal: &mut ratatui::DefaultTerminal, api: String) -> io::Result<bool> {
    let mut model = Model::default();
    let mut flow: Option<flow::Flow> = None;
    let mut connected = None;
    let mut cancelling = None;
    loop {
        if let Some(active) = &flow {
            while let Ok(e) = active.events.try_recv() {
                match e {
                    flow::Event::Ready(link) => {
                        if cancelling.is_some() {
                            continue;
                        }
                        model.phase = Phase::Waiting;
                        model.status = "Waiting for approval in your browser…".into();
                        model.link_started = Some(Instant::now());
                        model.scroll = 0;
                        if open_browser(&link.url).is_err() {
                            model.status="The browser could not open. Use Copy URL to open the link yourself.".into();
                        }
                        model.link = Some(link);
                    }
                    flow::Event::Status(text) => {
                        if cancelling.is_none() {
                            model.status = text;
                        }
                    }
                    flow::Event::Connected(email) => {
                        model.phase = Phase::Connected;
                        model.status = format!("Signed in as {email}.");
                        connected = Some(Instant::now());
                    }
                    flow::Event::Failed(error) => {
                        model.phase = Phase::Error;
                        model.status = error;
                        model.scroll = 0;
                    }
                    flow::Event::Cancelled => return Ok(false),
                }
            }
        }
        if connected.is_some_and(|t: Instant| t.elapsed() > Duration::from_millis(700)) {
            return Ok(true);
        }
        if cancelling.is_some_and(|t: Instant| t.elapsed() > Duration::from_secs(2)) {
            return Ok(false);
        }
        let mut buttons = view::Buttons::default();
        terminal.draw(|f| buttons = view::draw(f, &model))?;
        if !event::poll(Duration::from_millis(50))? {
            continue;
        }
        let (mut login, mut clipboard, mut cancel) = (false, false, false);
        match event::read()? {
            Event::Key(k) if k.kind == KeyEventKind::Press => match k.code {
                KeyCode::Enter => login = true,
                KeyCode::Char('c' | 'C')
                    if k.modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    cancel = true
                }
                KeyCode::Char('c' | 'C') => clipboard = true,
                KeyCode::Esc => cancel = true,
                KeyCode::PageDown | KeyCode::Down => model.scroll = model.scroll.saturating_add(3),
                KeyCode::PageUp | KeyCode::Up => model.scroll = model.scroll.saturating_sub(3),
                _ => {}
            },
            Event::Mouse(m) => match m.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    login = contains(buttons.login, m.column, m.row);
                    clipboard = buttons.copy.is_some_and(|r| contains(r, m.column, m.row));
                    cancel = contains(buttons.cancel, m.column, m.row);
                }
                MouseEventKind::ScrollDown => model.scroll = model.scroll.saturating_add(3),
                MouseEventKind::ScrollUp => model.scroll = model.scroll.saturating_sub(3),
                _ => {}
            },
            _ => {}
        }
        if cancel {
            if matches!(model.phase, Phase::Starting | Phase::Waiting) {
                if let Some(f) = &flow {
                    f.cancel();
                }
                model.status = "Cancelling sign-in…".into();
                cancelling = Some(Instant::now());
            } else {
                return Ok(false);
            }
        }
        if cancelling.is_some() || connected.is_some() {
            continue;
        }
        if clipboard {
            if let Some(link) = &model.link {
                model.status = match copy(&link.url) {
                    Ok(()) => "URL copied. Open it in your browser to finish signing in.".into(),
                    Err(e) => format!("Could not copy the URL: {e}"),
                };
            }
        }
        if login {
            if matches!(model.phase, Phase::Waiting) {
                if let Some(link) = &model.link {
                    if open_browser(&link.url).is_err() {
                        model.status = "Use Copy URL to open the link in your browser.".into();
                    }
                }
            } else if !matches!(model.phase, Phase::Starting) {
                flow = Some(flow::Flow::start(api.clone()));
                model = Model {
                    phase: Phase::Starting,
                    status: "Creating your sign-in link…".into(),
                    ..Model::default()
                };
            }
        }
    }
}
#[cfg(test)]
mod tests;
