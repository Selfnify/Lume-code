use ratatui::style::Color;

use super::active::Palette;

pub const PALETTE: Palette = Palette {
    bg: Color::Rgb(240, 246, 250),
    accent: Color::Rgb(0, 170, 200),
    text: Color::Rgb(24, 34, 44),
};