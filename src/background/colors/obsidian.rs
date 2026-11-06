use ratatui::style::Color;

use super::active::Palette;

pub const PALETTE: Palette = Palette {
    bg: Color::Rgb(12, 12, 14),
    accent: Color::Rgb(168, 108, 255),
    text: Color::Rgb(230, 230, 235),
};