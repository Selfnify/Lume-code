use serde_json::Value;
#[cfg(test)]
mod tests;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
pub struct Monitor {
    pub events: mpsc::Receiver<Value>,
    refresh: mpsc::Sender<()>,
    stop: Arc<AtomicBool>,
}
impl Monitor {
    pub fn new() -> Self {
        let (tx, events) = mpsc::channel();
        let (refresh, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let cancel = stop.clone();
        std::thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Runtime::new() else {
                return;
            };
            while !cancel.load(Ordering::Acquire) {
                if let Ok(c) = crate::connection::Connection::load() {
                    if let Ok(value) = runtime.block_on(c.request("/account/usage", None)) {
                        let _ = tx.send(value);
                    }
                }
                if rx.recv_timeout(std::time::Duration::from_secs(15)).is_err()
                    && cancel.load(Ordering::Acquire)
                {
                    break;
                }
                while rx.try_recv().is_ok() {}
            }
        });
        Self {
            events,
            refresh,
            stop,
        }
    }
    pub fn refresh(&self) {
        let _ = self.refresh.send(());
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.refresh.send(());
    }
}
pub fn label(data: &Value) -> Option<String> {
    let mut windows = vec![];
    for (key, name) in [("five_hour", "Session"), ("weekly", "Weekly")] {
        let w = &data[key];
        let used = w["used"].as_u64()?;
        let limit = w["limit"].as_u64()?.max(1);
        let reset = w["resets_at"].as_i64()?;
        if reset <= crate::session_store::now() {
            continue;
        }
        if used.saturating_mul(10) >= limit.saturating_mul(9) {
            windows.push((name, used, limit, reset));
        }
    }
    if windows.is_empty() {
        return None;
    }
    let reached = windows.iter().any(|(_, u, l, _)| u >= l);
    let reset = windows
        .iter()
        .filter(|(_, u, l, _)| !reached || u >= l)
        .map(|w| w.3)
        .max()
        .unwrap();
    let date = chrono::DateTime::from_timestamp(reset, 0)
        .map(|v| {
            v.with_timezone(&chrono::Local)
                .format("%a %H:%M")
                .to_string()
        })
        .unwrap_or_default();
    Some(format!(
        "{} · {} · resets {date} · /usage",
        if reached {
            "Usage limit reached"
        } else {
            "Nearing usage limit"
        },
        windows
            .iter()
            .map(|(n, u, l, _)| format!("{n} {}%", if u >= l { 100 } else { u * 100 / l }))
            .collect::<Vec<_>>()
            .join(" / ")
    ))
}
