use ratatui::style::Color;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{arctic, cream_terracotta, forest, midnight, obsidian, sunset};

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color,
    pub accent: Color,
    pub text: Color,
}

pub const THEMES: [(&str, Palette); 6] = [
    (
        "cream-terracotta",
        Palette {
            bg: cream_terracotta::BG,
            accent: cream_terracotta::ACCENT,
            text: cream_terracotta::TEXT,
        },
    ),
    ("midnight", midnight::PALETTE),
    ("obsidian", obsidian::PALETTE),
    ("forest", forest::PALETTE),
    ("arctic", arctic::PALETTE),
    ("sunset", sunset::PALETTE),
];

pub const DESCRIPTIONS: [&str; 6] = [
    "Warm cream with terracotta accents",
    "Dark navy with blue accents",
    "Deep black with purple accents",
    "Dark green with mint accents",
    "Cool white with cyan accents",
    "Warm peach with orange accents",
];

static CURRENT: AtomicUsize = AtomicUsize::new(1);

pub fn index() -> usize {
    CURRENT.load(Ordering::Relaxed)
}

pub fn set_index(index: usize) {
    CURRENT.store(index.min(THEMES.len() - 1), Ordering::Relaxed);
}

pub fn description(index: usize) -> &'static str {
    DESCRIPTIONS[index.min(DESCRIPTIONS.len() - 1)]
}

pub fn count() -> usize {
    THEMES.len()
}

pub fn palette() -> Palette {
    THEMES[CURRENT.load(Ordering::Relaxed)].1
}

pub fn bg() -> Color {
    palette().bg
}

pub fn accent() -> Color {
    palette().accent
}

pub fn text() -> Color {
    palette().text
}

pub fn names() -> Vec<&'static str> {
    THEMES.iter().map(|(name, _)| *name).collect()
}

pub fn current_name() -> &'static str {
    THEMES[CURRENT.load(Ordering::Relaxed)].0
}

pub fn select(name: &str) -> bool {
    match THEMES.iter().position(|(n, _)| *n == name) {
        Some(index) => {
            CURRENT.store(index, Ordering::Relaxed);
            true
        }
        None => false,
    }
}

fn settings_path() -> Option<std::path::PathBuf> {
    let base = match std::env::var_os("LOCALAPPDATA") {
        Some(dir) => std::path::PathBuf::from(dir),
        None => {
            let home = std::path::PathBuf::from(std::env::var_os("HOME")?);
            if cfg!(target_os = "macos") {
                home.join("Library/Application Support")
            } else {
                home
            }
        }
    };
    Some(base.join("LumeCode").join("theme.txt"))
}

pub fn load_saved() {
    if let Some(path) = settings_path() {
        if let Ok(name) = std::fs::read_to_string(path) {
            select(name.trim());
        }
    }
}

pub fn save_current() {
    if let Some(path) = settings_path() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, current_name());
    }
}