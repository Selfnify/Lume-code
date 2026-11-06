use ratatui::style::Color;

use super::active::Palette;

pub const PALETTE: Palette = Palette {
    bg: Color::Rgb(16, 24, 48),
    accent: Color::Rgb(88, 140, 255),
    text: Color::Rgb(220, 228, 240),
};