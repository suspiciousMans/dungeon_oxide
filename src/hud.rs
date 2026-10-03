//! The in-game HUD.
//!
//! Painted with egui primitives rather than the engine's stock widgets: the
//! default look is a grey panel with "Health" / "Armor" text labels, which
//! reads like a debug overlay rather than a dungeon crawl.
//!
//! Everything here is either a **pure layout function** (unit-tested, no egui)
//! or a thin painter that consumes those functions' output. Keeping the maths
//! out of the closure means the HUD's geometry can be verified without
//! opening a window.

use engine::ui::egui;

/// Everything the HUD needs for one frame, snapshotted so the paint closure
/// never borrows the game.
#[derive(Debug, Clone)]
pub struct HudView {
    pub health: f32,
    pub armor: f32,
    pub depth: i32,
    pub gold: i32,
    pub monsters_left: usize,
    pub toasts: Vec<(String, f32)>,
    pub banner: Option<(String, f32)>,
    pub dead: bool,
    pub won: bool,
    pub run_time: f32,
    pub danger: i32,
}

/// Where the HUD elements sit, in pixels from the top-left. Pure so it can be
/// tested against a known screen size.
/// Height of the armor bar. A constant so `layout` and `paint` agree.
pub const ARMOR_BAR_H: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HudLayout {
    pub orb_center: (f32, f32),
    pub orb_radius: f32,
    /// (x, y, width, height) of the armor bar.
    pub armor_bar: (f32, f32, f32, f32),
    pub status_origin: (f32, f32),
    pub toast_origin: (f32, f32),
    pub margin: f32,
}

/// Lays out the HUD for a screen of `w` x `h`.
///
/// Bottom-left health orb with the armor bar directly beneath it, status line
/// top-left, toasts stacked above the orb. `margin` scales with the window so
/// the HUD doesn't crowd a small window.
pub fn layout(w: f32, h: f32) -> HudLayout {
    let margin = (w * 0.025).clamp(14.0, 40.0);
    let orb_radius = (h * 0.075).clamp(34.0, 64.0);
    // The armor bar sits BELOW the orb, so the orb has to be raised by the
    // bar's height or the bar runs off the bottom edge. Clamped because a
    // degenerate window (height 0 mid-resize) would otherwise go negative.
    // Clamp the bar's BOTTOM edge, not just its top: with h=0 a top-only
    // clamp still leaves the 8px-tall bar hanging below the screen.
    let bar_bottom = (h - margin).clamp(0.0, h);
    // The bar's height shrinks to fit when there's no room, so a degenerate
    // window can't push any of it below the screen. Clamping only the top
    // edge isn't enough: at h=0 an 8px bar still hangs off the bottom.
    let bar_h = ARMOR_BAR_H.min(bar_bottom);
    let bar_y = (bar_bottom - bar_h).max(0.0);
    let orb_bottom = (bar_y - 10.0).max(orb_radius);
    HudLayout {
        orb_center: (margin + orb_radius, orb_bottom - orb_radius),
        orb_radius,
        armor_bar: (margin, bar_y, orb_radius * 2.0, bar_h),
        status_origin: (margin, margin + 12.0),
        // Toasts stack upward from just above the orb, newest lowest.
        toast_origin: (margin + orb_radius * 2.0 + 14.0, h - margin - orb_radius),
        margin,
    }
}

/// Health orb fill colour, shifting from red through amber as health drops.
/// Pure, so the "is low health obvious enough" question has a testable answer.
pub fn health_color(health: f32) -> egui::Color32 {
    let h = health.clamp(0.0, 1.0);
    let (r, g, b) = if h > 0.5 {
        // Green, warming slightly as it drops toward the midpoint.
        let t = (h - 0.5) * 2.0;
        (0.35 + 0.15 * (1.0 - t), 0.62 + 0.18 * t, 0.34, )
    } else {
        // Red — gets brighter the closer to death.
        let t = h * 2.0;
        (0.72 + 0.2 * t, 0.20 + 0.18 * t, 0.18,)
    };
    egui::Color32::from_rgb(
        (r * 255.0) as u8,
        (g * 255.0) as u8,
        (b * 255.0) as u8,
    )
}

/// Alpha for a toast given its remaining lifetime, so it fades out instead of
/// vanishing. Fades over the last `FADE` seconds.
pub fn toast_alpha(remaining: f32) -> f32 {
    const FADE: f32 = 0.9;
    (remaining / FADE).clamp(0.0, 1.0)
}

/// How strongly the screen edges should pulse red, from health + danger.
/// Pure so the "don't make it unreadable" ceiling is enforced by a test.
pub fn vignette_strength(health: f32, danger: i32) -> f32 {
    let hurt = (1.0 - health.clamp(0.0, 1.0)).powf(2.0);
    let threat = (danger.clamp(0, 10) as f32) / 10.0;
    // Capped: an opaque screen is worse than no warning.
    (hurt * 0.22 + threat * 0.02).clamp(0.0, 0.26)
}

/// Paints the whole HUD. Thin wrapper — all the maths is in the pure functions
/// above.
pub fn paint(c: &egui::Context, view: &HudView) {
    let screen = c.screen_rect();
    let l = layout(screen.width(), screen.height());
    let painter = c.layer_painter(egui::LayerId::background());

    // --- danger / low-health vignette, drawn first so it's under everything
    let strength = vignette_strength(view.health, view.danger);
    // Below this it's pure noise on a healthy player — a warning nobody reads
    // is worse than no warning.
    if strength > 0.06 {
        let band = strength * 150.0;
        // Alpha premultiplied in GAMMA space: egui's from_rgba_unmultiplied
        // converts in linear space, which turns a subtle wash into a slab.
        let col = egui::Color32::from_rgba_premultiplied(60, 6, 6, band as u8);
        let t = 90.0;
        painter.rect_filled(
            egui::Rect::from_min_size(screen.min, egui::vec2(screen.width(), t)),
            0.0,
            col,
        );
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(screen.min.x, screen.max.y - t),
                egui::vec2(screen.width(), t),
            ),
            0.0,
            col,
        );
    }

    // --- status line, top-left
    let status = format!(
        "DEPTH {}   ·   {} GOLD   ·   {} LEFT",
        view.depth, view.gold, view.monsters_left
    );
    painter.text(
        egui::pos2(l.status_origin.0, l.status_origin.1),
        egui::Align2::LEFT_TOP,
        &status,
        egui::FontId::proportional(19.0),
        egui::Color32::from_rgba_unmultiplied(228, 222, 205, 235),
    );

    // --- health orb, bottom-left
    painter.circle_filled(
        egui::pos2(l.orb_center.0, l.orb_center.1),
        l.orb_radius,
        egui::Color32::from_rgba_unmultiplied(16, 14, 18, 205),
    );
    if view.health > 0.001 {
        painter.circle_filled(
            egui::pos2(l.orb_center.0, l.orb_center.1),
            l.orb_radius * view.health.clamp(0.0, 1.0),
            health_color(view.health),
        );
    }
    painter.circle_stroke(
        egui::pos2(l.orb_center.0, l.orb_center.1),
        l.orb_radius,
        egui::Stroke::new(2.0, egui::Color32::from_rgba_unmultiplied(90, 82, 70, 230)),
    );
    // Numeric readout inside the orb, so exact health is never a guess.
    painter.text(
        egui::pos2(l.orb_center.0, l.orb_center.1),
        egui::Align2::CENTER_CENTER,
        format!("{}", (view.health * 100.0).round() as i32),
        egui::FontId::proportional((l.orb_radius * 0.62).min(34.0)),
        egui::Color32::from_rgba_unmultiplied(240, 236, 224, 245),
    );

    // --- armor bar under the orb, only once you have any
    if view.armor > 0.001 {
        let (x, y, w, h) = l.armor_bar;
        painter.rect_filled(
            egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h)),
            3.0,
            egui::Color32::from_rgba_unmultiplied(20, 18, 22, 200),
        );
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(x, y),
                egui::vec2(w * view.armor.clamp(0.0, 1.0), h),
            ),
            3.0,
            egui::Color32::from_rgba_unmultiplied(150, 168, 196, 240),
        );
        painter.text(
            egui::pos2(x + w + 8.0, y + h / 2.0),
            egui::Align2::LEFT_CENTER,
            format!("{:.0}%", view.armor * 100.0),
            egui::FontId::proportional(13.0),
            egui::Color32::from_rgba_unmultiplied(180, 192, 208, 220),
        );
    }

    // --- toasts, stacked upward, fading
    let mut y = l.toast_origin.1;
    for (text, remaining) in view.toasts.iter().rev().take(5) {
        let a = toast_alpha(*remaining);
        if a <= 0.01 {
            continue;
        }
        painter.text(
            egui::pos2(l.toast_origin.0, y),
            egui::Align2::LEFT_BOTTOM,
            text,
            egui::FontId::proportional(17.0),
            egui::Color32::from_rgba_unmultiplied(232, 226, 208, (235.0 * a) as u8),
        );
        y -= 26.0;
    }

    // --- floor banner
    if let Some((text, remaining)) = &view.banner {
        let a = toast_alpha(*remaining);
        painter.text(
            egui::pos2(screen.width() / 2.0, screen.height() * 0.22),
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(26.0),
            egui::Color32::from_rgba_unmultiplied(226, 216, 192, (240.0 * a) as u8),
        );
    }

    // --- end-of-run panel
    if view.dead || view.won {
        let (title, title_col) = if view.won {
            ("YOU ESCAPED", egui::Color32::from_rgb(226, 214, 170))
        } else {
            ("YOU DIED", egui::Color32::from_rgb(206, 96, 84))
        };
        let panel = egui::Rect::from_center_size(
            egui::pos2(screen.width() / 2.0, screen.height() / 2.0),
            egui::vec2(420.0, 190.0),
        );
        painter.rect_filled(
            panel,
            6.0,
            egui::Color32::from_rgba_unmultiplied(14, 13, 16, 238),
        );
        painter.rect_stroke(
            panel,
            6.0,
            egui::Stroke::new(
                1.5,
                egui::Color32::from_rgba_unmultiplied(84, 76, 66, 220),
            ),
        );
        painter.text(
            egui::pos2(panel.center().x, panel.min.y + 34.0),
            egui::Align2::CENTER_CENTER,
            title,
            egui::FontId::proportional(30.0),
            title_col,
        );
        let detail = format!(
            "depth {}   ·   {} gold   ·   {:.0}s",
            view.depth, view.gold, view.run_time
        );
        painter.text(
            egui::pos2(panel.center().x, panel.center().y + 10.0),
            egui::Align2::CENTER_CENTER,
            detail,
            egui::FontId::proportional(17.0),
            egui::Color32::from_rgba_unmultiplied(206, 198, 180, 230),
        );
        painter.text(
            egui::pos2(panel.center().x, panel.max.y - 26.0),
            egui::Align2::CENTER_CENTER,
            "Esc to quit",
            egui::FontId::proportional(14.0),
            egui::Color32::from_rgba_unmultiplied(140, 132, 118, 200),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_keeps_the_orb_on_screen() {
        for (w, h) in [(640.0, 480.0), (1280.0, 720.0), (1920.0, 1080.0), (3840.0, 2160.0)] {
            let l = layout(w, h);
            assert!(
                l.orb_center.0 - l.orb_radius >= 0.0,
                "{w}x{h}: orb off the left edge"
            );
            assert!(
                l.orb_center.1 + l.orb_radius <= h,
                "{w}x{h}: orb off the bottom edge ({} + {} > {h})",
                l.orb_center.1,
                l.orb_radius
            );
            assert!(l.armor_bar.2 > 0.0, "{w}x{h}: armor bar has no width");
        }
    }

    #[test]
    fn layout_scales_with_the_window() {
        let small = layout(640.0, 480.0);
        let large = layout(1920.0, 1080.0);
        assert!(
            large.orb_radius > small.orb_radius,
            "orb should grow with the window"
        );
        assert!(large.margin > small.margin, "margin should scale too");
    }

    /// A degenerate window must not produce NaN geometry (which would make
    /// egui panic or draw nothing at all).
    #[test]
    fn layout_survives_degenerate_sizes() {
        for (w, h) in [(0.0, 0.0), (1.0, 1.0), (10.0, 4000.0)] {
            let l = layout(w, h);
            for v in [
                l.orb_center.0,
                l.orb_center.1,
                l.orb_radius,
                l.margin,
                l.armor_bar.3,
            ] {
                assert!(v.is_finite(), "{w}x{h}: produced {v}");
            }
            assert!(
                l.armor_bar.1 + l.armor_bar.3 <= h + 1e-3,
                "{w}x{h}: armor bar off-screen even at a degenerate size"
            );
        }
    }

    #[test]
    fn health_color_is_red_when_low_and_green_when_high() {
        let low = health_color(0.1);
        let high = health_color(0.9);
        assert!(low.r() > low.g(), "low health should be red-dominant");
        assert!(high.g() > high.r(), "high health should be green-dominant");
    }

    #[test]
    fn toast_alpha_fades_out() {
        assert_eq!(toast_alpha(0.0), 0.0, "expired toast must be invisible");
        assert!(toast_alpha(5.0) <= 1.0, "alpha must not exceed 1");
        assert!(toast_alpha(0.2) < toast_alpha(1.0), "must fade with time");
    }

    /// An opaque warning is worse than none — the ceiling is a real rule.
    #[test]
    fn vignette_never_obscures_the_screen() {
        for health in [0.0, 0.1, 0.5, 1.0] {
            for danger in [0, 5, 10] {
                let s = vignette_strength(health, danger);
                assert!(
                    s <= 0.6,
                    "vignette {s} at health {health} danger {danger} is too opaque"
                );
            }
        }
        assert_eq!(
            vignette_strength(1.0, 0),
            0.0,
            "full health, no danger = no vignette"
        );
        // Health must dominate. At 88% with a few monsters left the screen
        // should be essentially clean - a red wash on a healthy player is
        // noise, not information.
        assert!(
            vignette_strength(0.88, 3) < 0.05,
            "a healthy player shouldn't see a red wash: {}",
            vignette_strength(0.88, 3)
        );
    }
}