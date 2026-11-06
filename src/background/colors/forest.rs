use ratatui::style::Color;

use super::active::Palette;

pub const PALETTE: Palette = Palette {
    bg: Color::Rgb(18, 36, 28),
    accent: Color::Rgb(120, 230, 180),
    text: Color::Rgb(218, 236, 226),
};