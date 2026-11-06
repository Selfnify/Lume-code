use ratatui::style::Color;

use super::active::Palette;

pub const PALETTE: Palette = Palette {
    bg: Color::Rgb(252, 235, 220),
    accent: Color::Rgb(230, 110, 30),
    text: Color::Rgb(60, 30, 20),
};