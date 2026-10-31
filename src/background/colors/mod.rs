#[path = "cream-terracotta.rs"]
pub mod cream_terracotta;

use ratatui::{buffer::Buffer, style::Color};

/// Apple Terminal versions that report xterm-256color but lack truecolor
/// support smear RGB background escapes across adjacent cells. Quantize only
/// for that terminal; modern terminals keep the original RGB palette.
pub fn needs_ansi256_fallback() -> bool {
    if let Ok(depth) = std::env::var("LUME_COLOR_DEPTH") {
        return depth == "256";
    }
    if std::env::var("COLORTERM")
        .ok()
        .is_some_and(|value| matches!(value.to_lowercase().as_str(), "truecolor" | "24bit"))
    {
        return false;
    }
    if std::env::var("TERM").ok().is_some_and(|term| {
        let term = term.to_lowercase();
        term.contains("-direct") || term.contains("truecolor")
    }) {
        return false;
    }
    std::env::var("TERM_PROGRAM").as_deref() == Ok("Apple_Terminal")
}

/// Convert rendered RGB styles to the xterm 256-color palette after layout,
/// keeping all existing colors and styles untouched on truecolor terminals.
pub fn adapt_buffer(buffer: &mut Buffer) {
    if !needs_ansi256_fallback() {
        return;
    }
    for cell in &mut buffer.content {
        cell.set_fg(ansi256(cell.fg));
        cell.set_bg(ansi256(cell.bg));
    }
}

fn ansi256(color: Color) -> Color {
    let Color::Rgb(r, g, b) = color else {
        return color;
    };
    let levels = [0_u8, 95, 135, 175, 215, 255];
    let mut best = (u32::MAX, 0_u8);
    for red in 0..6_u8 {
        for green in 0..6_u8 {
            for blue in 0..6_u8 {
                let index = 16 + 36 * red + 6 * green + blue;
                let candidate = (
                    levels[red as usize],
                    levels[green as usize],
                    levels[blue as usize],
                );
                let distance = color_distance((r, g, b), candidate);
                if distance < best.0 {
                    best = (distance, index);
                }
            }
        }
    }
    for index in 0..24_u8 {
        let level = 8 + index * 10;
        let distance = color_distance((r, g, b), (level, level, level));
        if distance < best.0 {
            best = (distance, 232 + index);
        }
    }
    Color::Indexed(best.1)
}

fn color_distance(a: (u8, u8, u8), b: (u8, u8, u8)) -> u32 {
    let red = i32::from(a.0) - i32::from(b.0);
    let green = i32::from(a.1) - i32::from(b.1);
    let blue = i32::from(a.2) - i32::from(b.2);
    (red * red + green * green + blue * blue) as u32
}
