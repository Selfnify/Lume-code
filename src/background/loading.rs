use std::f64::consts::PI;

use ratatui::{
    style::Color,
    symbols::Marker,
    widgets::canvas::{Canvas, Circle, Context, Line as CanvasLine},
    Frame,
};

use super::colors::cream_terracotta as theme;

pub const DURATION: f64 = 6.5;

const SPIN_START: f64 = 1.9;
const SPIN_DUR: f64 = 0.9;
const SHIFT_START: f64 = 2.8;
const SHIFT_DUR: f64 = 1.3;
const LETTER_START: f64 = 2.9;
const LETTER_STAGGER: f64 = 0.3;
const LETTER_DUR: f64 = 0.4;
const BREATH_START: f64 = 4.1;
const BREATH_DUR: f64 = 2.4;

const Y_HALF: f64 = 135.0;
const LOGO_SHIFT: f64 = -110.0;
const TEXT_X: f64 = 25.0;
const TEXT_Y: f64 = -26.0;
const TEXT_SCALE: f64 = 52.0;
const STROKE_STEP: f64 = 1.2;
const STROKE_RADIUS: i32 = 2;
const CIRCLE_R: f64 = 26.0;
const CORE_RINGS: usize = 8;
const BLADE_SAMPLES: usize = 32;
const CIRCLE_SAMPLES: usize = 48;

pub fn is_done(t: f64) -> bool {
    t >= DURATION
}

pub fn draw(frame: &mut Frame, t: f64) {
    let area = frame.area();
    let x_half = Y_HALF * area.width as f64 / (2.0 * area.height.max(1) as f64);
    let canvas = Canvas::default()
        .background_color(theme::BG)
        .marker(Marker::Braille)
        .x_bounds([-x_half, x_half])
        .y_bounds([-Y_HALF, Y_HALF])
        .paint(move |ctx| paint(ctx, t));
    frame.render_widget(canvas, area);
}

#[derive(Clone, Copy)]
struct Xform {
    rot: f64,
    scale: f64,
    dx: f64,
}

fn paint(ctx: &mut Context, t: f64) {
    let spin = ease_in_out(seg(t, SPIN_START, SPIN_DUR)) * 360.0;
    let breath = 1.0 + 0.05 * (PI * seg(t, BREATH_START, BREATH_DUR)).sin();
    let shift = LOGO_SHIFT * ease_cubic(seg(t, SHIFT_START, SHIFT_DUR));
    let xf = Xform {
        rot: spin,
        scale: breath,
        dx: shift,
    };

    for i in 0..8 {
        let blade_rot = 22.5 + 45.0 * i as f64;
        let start = 0.3 + 0.16 * i as f64;
        let progress = ease_in_out(seg(t, start, 0.45));
        if progress <= 0.0 {
            continue;
        }
        let color = if i % 2 == 0 {
            theme::ACCENT
        } else {
            mix(theme::BG, theme::ACCENT, 0.32)
        };
        draw_blade(ctx, blade_rot, progress, &xf, color);
    }

    let stroke = ease_out(seg(t, 0.1, 0.4));
    let fill = ease_in_out(seg(t, 0.3, 0.3));
    draw_core(ctx, stroke, fill, &xf);

    for (i, (offset, strokes)) in glyphs().iter().enumerate() {
        let start = LETTER_START + LETTER_STAGGER * i as f64;
        let progress = ease_out(seg(t, start, LETTER_DUR));
        if progress > 0.0 {
            draw_glyph(ctx, *offset, strokes, progress);
        }
    }
}

fn glyphs() -> Vec<(f64, Vec<Vec<(f64, f64)>>)> {
    vec![
        (0.0, vec![vec![(0.0, 1.0), (0.0, 0.0), (0.65, 0.0)]]),
        (
            1.0,
            vec![
                vec![(0.0, 0.7), (0.0, 0.2), (0.15, 0.0), (0.45, 0.0), (0.6, 0.2)],
                vec![(0.6, 0.7), (0.6, 0.0)],
            ],
        ),
        (
            2.0,
            vec![
                vec![(0.0, 0.0), (0.0, 0.7)],
                vec![(0.0, 0.6), (0.2, 0.7), (0.35, 0.6), (0.35, 0.0)],
                vec![(0.35, 0.6), (0.55, 0.7), (0.7, 0.6), (0.7, 0.0)],
            ],
        ),
        (
            3.0,
            vec![vec![
                (0.0, 0.35),
                (0.65, 0.35),
                (0.65, 0.55),
                (0.5, 0.7),
                (0.2, 0.7),
                (0.0, 0.5),
                (0.0, 0.2),
                (0.2, 0.0),
                (0.6, 0.0),
            ]],
        ),
    ]
}

fn draw_glyph(ctx: &mut Context, offset: f64, strokes: &[Vec<(f64, f64)>], progress: f64) {
    let segments: Vec<((f64, f64), (f64, f64))> = strokes
        .iter()
        .flat_map(|stroke| stroke.windows(2).map(|w| (w[0], w[1])))
        .collect();
    let visible = (progress * segments.len() as f64).round() as usize;

    for &((ax, ay), (bx, by)) in segments.iter().take(visible) {
        for dx in -STROKE_RADIUS..=STROKE_RADIUS {
            for dy in -STROKE_RADIUS..=STROKE_RADIUS {
                let ox = dx as f64 * STROKE_STEP;
                let oy = dy as f64 * STROKE_STEP;
                ctx.draw(&CanvasLine {
                    x1: TEXT_X + (offset + ax) * TEXT_SCALE + ox,
                    y1: TEXT_Y + ay * TEXT_SCALE + oy,
                    x2: TEXT_X + (offset + bx) * TEXT_SCALE + ox,
                    y2: TEXT_Y + by * TEXT_SCALE + oy,
                    color: theme::TEXT,
                });
            }
        }
    }
}

fn draw_blade(ctx: &mut Context, blade_rot: f64, progress: f64, xf: &Xform, color: Color) {
    let pts = blade_points();
    let n = ((progress * BLADE_SAMPLES as f64).round() as usize).min(BLADE_SAMPLES);
    let (s, c) = blade_rot.to_radians().sin_cos();

    let projected: Vec<(f64, f64)> = pts[..=n]
        .iter()
        .map(|&(x, y)| {
            let rx = x * c - y * s;
            let ry = x * s + y * c;
            project(rx, ry, xf)
        })
        .collect();

    for w in projected.windows(2) {
        ctx.draw(&CanvasLine {
            x1: w[0].0,
            y1: w[0].1,
            x2: w[1].0,
            y2: w[1].1,
            color,
        });
    }
}

fn draw_core(ctx: &mut Context, stroke: f64, fill: f64, xf: &Xform) {
    if stroke > 0.0 {
        let n = (stroke * CIRCLE_SAMPLES as f64).round() as usize;
        let ring: Vec<(f64, f64)> = (0..=n)
            .map(|i| {
                let a = 2.0 * PI * i as f64 / CIRCLE_SAMPLES as f64;
                project(CIRCLE_R * a.cos(), CIRCLE_R * a.sin(), xf)
            })
            .collect();
        for w in ring.windows(2) {
            ctx.draw(&CanvasLine {
                x1: w[0].0,
                y1: w[0].1,
                x2: w[1].0,
                y2: w[1].1,
                color: theme::ACCENT,
            });
        }
    }

    if fill > 0.0 {
        let (cx, cy) = project(0.0, 0.0, xf);
        for k in 1..=CORE_RINGS {
            let r = CIRCLE_R * fill * k as f64 / CORE_RINGS as f64 * xf.scale;
            ctx.draw(&Circle {
                x: cx,
                y: cy,
                radius: r,
                color: theme::ACCENT,
            });
        }
    }
}

fn blade_points() -> Vec<(f64, f64)> {
    let p0 = (0.0, -32.0);
    let c1 = (28.0, -60.0);
    let c2 = (90.0, -40.0);
    let p1 = (105.0, -10.0);
    let c3 = (80.0, 10.0);
    let c4 = (30.0, 0.0);
    let p2 = (0.0, -32.0);

    let mut pts = Vec::with_capacity(BLADE_SAMPLES + 1);
    for k in 0..16 {
        pts.push(bezier(p0, c1, c2, p1, k as f64 / 16.0));
    }
    for k in 0..=16 {
        pts.push(bezier(p1, c3, c4, p2, k as f64 / 16.0));
    }
    pts
}

fn bezier(p0: (f64, f64), p1: (f64, f64), p2: (f64, f64), p3: (f64, f64), t: f64) -> (f64, f64) {
    let u = 1.0 - t;
    let a = u * u * u;
    let b = 3.0 * u * u * t;
    let c = 3.0 * u * t * t;
    let d = t * t * t;
    (
        a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0,
        a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1,
    )
}

fn project(x: f64, y: f64, xf: &Xform) -> (f64, f64) {
    let (s, c) = xf.rot.to_radians().sin_cos();
    let rx = (x * c - y * s) * xf.scale;
    let ry = (x * s + y * c) * xf.scale;
    (rx + xf.dx, -ry)
}

fn seg(t: f64, start: f64, dur: f64) -> f64 {
    ((t - start) / dur).clamp(0.0, 1.0)
}

fn ease_in_out(p: f64) -> f64 {
    if p < 0.5 {
        2.0 * p * p
    } else {
        1.0 - (-2.0 * p + 2.0).powi(2) / 2.0
    }
}

fn ease_out(p: f64) -> f64 {
    1.0 - (1.0 - p).powi(2)
}

fn ease_cubic(p: f64) -> f64 {
    if p < 0.5 {
        4.0 * p * p * p
    } else {
        1.0 - (-2.0 * p + 2.0).powi(3) / 2.0
    }
}

fn mix(a: Color, b: Color, t: f64) -> Color {
    if let (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) = (a, b) {
        let t = t.clamp(0.0, 1.0);
        let lerp = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t).round() as u8;
        Color::Rgb(lerp(r1, r2), lerp(g1, g2), lerp(b1, b2))
    } else {
        a
    }
}
