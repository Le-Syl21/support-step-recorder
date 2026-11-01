//! Small vector icons painted with the palette. egui's fonts cover few UI
//! symbols (no `●`, `■`, `◐`…), so every pictogram is drawn here instead of
//! risking a missing-glyph box.

use egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Icon {
    /// A filled disc: start recording.
    Record,
    /// A filled square: stop recording.
    Stop,
    Sun,
    Moon,
    /// A screen: follow the system theme.
    Monitor,
    /// A mouse with its left button lit: a click step.
    Mouse,
    /// A keyboard: a typing step.
    Keyboard,
    /// A camera: the screenshot taken at each click.
    Camera,
    /// A document with a folded corner: the HTML report.
    Report,
    /// An arrow into a tray: save.
    Save,
    Plus,
    Minus,
    /// Four corners: fit to view.
    Fit,
    Check,
    /// A triangle with a bang: warning.
    Warning,
    Trash,
    Clock,
    /// A window frame: the active window.
    Window,
    /// A speech bubble: community and support.
    Chat,
    /// Two arrows apart: enlarge.
    Expand,
}

impl Icon {
    /// Paint the icon in `rect` (a square works best).
    pub fn paint(self, painter: &Painter, rect: Rect, color: Color32) {
        let s = rect.width().min(rect.height());
        let c = rect.center();
        let w = (s * 0.09).max(1.2);
        let stroke = Stroke::new(w, color);
        let at = |x: f32, y: f32| pos2(c.x + x * s, c.y + y * s);
        let line = |a: Pos2, b: Pos2| {
            painter.line_segment([a, b], stroke);
        };
        let frame = |min: Pos2, max: Pos2, r: f32| {
            painter.rect_stroke(
                Rect::from_min_max(min, max),
                r,
                stroke,
                egui::StrokeKind::Middle,
            );
        };
        match self {
            Icon::Record => {
                painter.circle_filled(c, s * 0.36, color);
            }
            Icon::Stop => {
                let sq = Rect::from_center_size(c, Vec2::splat(s * 0.62));
                painter.rect_filled(sq, s * 0.08, color);
            }
            Icon::Sun => {
                painter.circle_filled(c, s * 0.17, color);
                for i in 0..8 {
                    let a = i as f32 * std::f32::consts::TAU / 8.0;
                    let d = vec2(a.cos(), a.sin());
                    line(c + d * s * 0.28, c + d * s * 0.43);
                }
            }
            Icon::Moon => {
                // A crescent: the outer arc, then the inner arc back.
                let r = s * 0.36;
                let off = vec2(s * 0.17, -s * 0.13);
                let n = 48;
                let ring = |center: Pos2, radius: f32| {
                    (0..=n).map(move |i| {
                        let a = std::f32::consts::TAU * i as f32 / n as f32;
                        center + vec2(a.cos(), a.sin()) * radius
                    })
                };
                let inner_r = r * 0.82;
                let mut pts: Vec<Pos2> = ring(c, r)
                    .filter(|p| (*p - (c + off)).length() >= inner_r)
                    .collect();
                let mut inner: Vec<Pos2> = ring(c + off, inner_r)
                    .filter(|p| (*p - c).length() <= r)
                    .collect();
                inner.reverse();
                pts.extend(inner);
                painter.add(Shape::closed_line(pts, stroke));
            }
            Icon::Monitor => {
                frame(at(-0.42, -0.34), at(0.42, 0.20), 2.0);
                line(at(0.0, 0.20), at(0.0, 0.34));
                line(at(-0.20, 0.36), at(0.20, 0.36));
            }
            Icon::Mouse => {
                let body = Rect::from_min_max(at(-0.24, -0.42), at(0.24, 0.42));
                painter.rect_stroke(body, s * 0.24, stroke, egui::StrokeKind::Middle);
                // The left button, filled.
                let left = vec![
                    at(-0.24, -0.06),
                    at(-0.24, -0.20),
                    at(-0.17, -0.35),
                    at(-0.02, -0.42),
                    at(-0.02, -0.06),
                ];
                painter.add(Shape::convex_polygon(left, color, Stroke::NONE));
                line(at(-0.24, -0.06), at(0.24, -0.06));
            }
            Icon::Keyboard => {
                frame(at(-0.44, -0.26), at(0.44, 0.26), s * 0.06);
                let key = s * 0.07;
                for (row, y) in [-0.11f32, 0.02].iter().enumerate() {
                    let count = 5 - row;
                    for i in 0..count {
                        let x = -0.28 + i as f32 * 0.14 + row as f32 * 0.07;
                        painter.circle_filled(at(x, *y), key * 0.6, color);
                    }
                }
                line(at(-0.18, 0.14), at(0.18, 0.14));
            }
            Icon::Camera => {
                frame(at(-0.42, -0.22), at(0.42, 0.32), s * 0.07);
                let bump = vec![
                    at(-0.16, -0.22),
                    at(-0.10, -0.34),
                    at(0.10, -0.34),
                    at(0.16, -0.22),
                ];
                painter.add(Shape::line(bump, stroke));
                painter.circle_stroke(at(0.0, 0.05), s * 0.15, stroke);
            }
            Icon::Report => {
                let pts = vec![
                    at(-0.30, -0.42),
                    at(0.12, -0.42),
                    at(0.30, -0.24),
                    at(0.30, 0.42),
                    at(-0.30, 0.42),
                ];
                painter.add(Shape::closed_line(pts, stroke));
                painter.add(Shape::line(
                    vec![at(0.12, -0.42), at(0.12, -0.24), at(0.30, -0.24)],
                    stroke,
                ));
                for y in [-0.04, 0.10, 0.24] {
                    line(at(-0.16, y), at(0.16, y));
                }
            }
            Icon::Save => {
                line(at(0.0, -0.42), at(0.0, 0.12));
                painter.add(Shape::line(
                    vec![at(-0.18, -0.06), at(0.0, 0.12), at(0.18, -0.06)],
                    stroke,
                ));
                painter.add(Shape::line(
                    vec![
                        at(-0.40, 0.08),
                        at(-0.40, 0.38),
                        at(0.40, 0.38),
                        at(0.40, 0.08),
                    ],
                    stroke,
                ));
            }
            Icon::Plus => {
                line(at(-0.30, 0.0), at(0.30, 0.0));
                line(at(0.0, -0.30), at(0.0, 0.30));
            }
            Icon::Minus => {
                line(at(-0.30, 0.0), at(0.30, 0.0));
            }
            Icon::Fit => {
                for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                    let corner = at(0.38 * sx, 0.34 * sy);
                    painter.add(Shape::line(
                        vec![
                            corner + vec2(0.0, -0.18 * s * sy),
                            corner,
                            corner + vec2(-0.18 * s * sx, 0.0),
                        ],
                        stroke,
                    ));
                }
            }
            Icon::Check => {
                painter.add(Shape::line(
                    vec![at(-0.32, 0.02), at(-0.10, 0.24), at(0.34, -0.22)],
                    Stroke::new(w * 1.3, color),
                ));
            }
            Icon::Warning => {
                let tri = vec![at(0.0, -0.40), at(0.44, 0.36), at(-0.44, 0.36)];
                painter.add(Shape::closed_line(tri, stroke));
                line(at(0.0, -0.12), at(0.0, 0.12));
                painter.circle_filled(at(0.0, 0.24), w * 0.75, color);
            }
            Icon::Trash => {
                line(at(-0.38, -0.26), at(0.38, -0.26));
                painter.add(Shape::line(
                    vec![
                        at(-0.12, -0.26),
                        at(-0.10, -0.40),
                        at(0.10, -0.40),
                        at(0.12, -0.26),
                    ],
                    stroke,
                ));
                painter.add(Shape::line(
                    vec![
                        at(-0.28, -0.26),
                        at(-0.22, 0.40),
                        at(0.22, 0.40),
                        at(0.28, -0.26),
                    ],
                    stroke,
                ));
                line(at(-0.06, -0.10), at(-0.06, 0.24));
                line(at(0.08, -0.10), at(0.08, 0.24));
            }
            Icon::Clock => {
                painter.circle_stroke(c, s * 0.40, stroke);
                painter.add(Shape::line(
                    vec![at(0.0, -0.24), at(0.0, 0.0), at(0.18, 0.10)],
                    stroke,
                ));
            }
            Icon::Window => {
                frame(at(-0.42, -0.34), at(0.42, 0.34), s * 0.06);
                line(at(-0.42, -0.16), at(0.42, -0.16));
                painter.circle_filled(at(-0.30, -0.25), w * 0.6, color);
                painter.circle_filled(at(-0.20, -0.25), w * 0.6, color);
            }
            Icon::Chat => {
                let body = Rect::from_min_max(at(-0.42, -0.34), at(0.42, 0.20));
                painter.rect_stroke(body, s * 0.12, stroke, egui::StrokeKind::Middle);
                painter.add(Shape::line(
                    vec![at(-0.18, 0.20), at(-0.24, 0.40), at(0.02, 0.20)],
                    stroke,
                ));
                for x in [-0.18, 0.0, 0.18] {
                    painter.circle_filled(at(x, -0.07), w * 0.7, color);
                }
            }
            Icon::Expand => {
                line(at(-0.34, -0.34), at(0.34, 0.34));
                for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, 1.0)] {
                    let tip = at(0.36 * sx, 0.36 * sy);
                    painter.add(Shape::line(
                        vec![
                            tip + vec2(0.0, -0.22 * s * sy),
                            tip,
                            tip + vec2(-0.22 * s * sx, 0.0),
                        ],
                        stroke,
                    ));
                }
            }
        }
    }
}

/// An icon on its own, in a square of `size` points.
pub fn icon(ui: &mut Ui, icon: Icon, size: f32, color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    icon.paint(ui.painter(), rect, color);
    resp
}
