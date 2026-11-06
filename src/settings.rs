use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

pub const ITEMS: [&str; 2] = ["Completion bell", "@ file mentions"];

static BELL: AtomicBool = AtomicBool::new(true);
static MENTIONS: AtomicBool = AtomicBool::new(true);

pub fn bell() -> bool {
    BELL.load(Ordering::Relaxed)
}

pub fn mentions() -> bool {
    MENTIONS.load(Ordering::Relaxed)
}

pub fn get(index: usize) -> bool {
    match index {
        0 => bell(),
        _ => mentions(),
    }
}

pub fn toggle(index: usize) {
    let flag = if index == 0 { &BELL } else { &MENTIONS };
    flag.fetch_xor(true, Ordering::Relaxed);
    save();
}

fn path() -> Option<PathBuf> {
    let base = match std::env::var_os("LOCALAPPDATA") {
        Some(dir) => PathBuf::from(dir),
        None => {
            let home = PathBuf::from(std::env::var_os("HOME")?);
            if cfg!(target_os = "macos") {
                home.join("Library/Application Support")
            } else {
                home
            }
        }
    };
    Some(base.join("LumeCode").join("settings.txt"))
}

pub fn load() {
    let Some(path) = path() else { return };
    let Ok(text) = std::fs::read_to_string(path) else { return };
    for line in text.lines() {
        match line.trim() {
            "bell=on" => BELL.store(true, Ordering::Relaxed),
            "bell=off" => BELL.store(false, Ordering::Relaxed),
            "mentions=on" => MENTIONS.store(true, Ordering::Relaxed),
            "mentions=off" => MENTIONS.store(false, Ordering::Relaxed),
            _ => {}
        }
    }
}

fn save() {
    let Some(path) = path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let text = format!(
        "bell={}\nmentions={}\n",
        if bell() { "on" } else { "off" },
        if mentions() { "on" } else { "off" }
    );
    let _ = std::fs::write(path, text);
}