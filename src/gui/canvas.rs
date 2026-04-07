use egui::{Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, Vec2};

use crate::grafcet::{Grafcet, StepKind};

// ── Constantes visuelles ──────────────────────────────────────────────────────
pub const STEP_W: f32 = 120.0;
pub const STEP_H: f32 = 60.0;
pub const TRANS_W: f32 = 60.0;
pub const TRANS_H: f32 = 4.0;

const C_STEP_NORMAL: Color32 = Color32::from_rgb(55, 65, 85);
const C_STEP_INITIAL: Color32 = Color32::from_rgb(40, 80, 120);
const C_STEP_ACTIVE: Color32 = Color32::from_rgb(40, 130, 80);
const C_BORDER: Color32 = Color32::from_rgb(160, 200, 240);
const C_TRANS: Color32 = Color32::from_rgb(220, 220, 220);
const C_LINK: Color32 = Color32::from_rgb(200, 200, 200);
const C_COND: Color32 = Color32::from_rgb(200, 160, 230);
const C_ARROW: Color32 = Color32::from_rgb(200, 200, 200);

/// Renvoie le rectangle d'une étape dans les coordonnées canvas (pan + zoom).
pub fn step_rect(pos: [f32; 2], offset: Vec2, zoom: f32) -> Rect {
    let cx = pos[0] * zoom + offset.x;
    let cy = pos[1] * zoom + offset.y;
    let hw = STEP_W * zoom / 2.0;
    let hh = STEP_H * zoom / 2.0;
    Rect::from_min_max(Pos2::new(cx - hw, cy - hh), Pos2::new(cx + hw, cy + hh))
}

/// Dessine toutes les liaisons (étape → transition → étape).
pub fn draw_links(painter: &Painter, grafcet: &Grafcet, offset: Vec2, zoom: f32) {
    let stroke = Stroke::new(1.5 * zoom, C_LINK);
    let arrow_stroke = Stroke::new(1.5 * zoom, C_ARROW);

    for t in &grafcet.transitions {
        let src = grafcet.step(t.from_step);
        let dst = grafcet.step(t.to_step);
        if src.is_none() || dst.is_none() {
            continue;
        }
        let src = src.unwrap();
        let dst = dst.unwrap();

        // Bas de l'étape source, haut de l'étape destination (coordonnées écran)
        let sx = src.pos[0] * zoom + offset.x;
        let sy_bot = src.pos[1] * zoom + offset.y + STEP_H * zoom / 2.0;
        let dx = dst.pos[0] * zoom + offset.x;
        let dy_top = dst.pos[1] * zoom + offset.y - STEP_H * zoom / 2.0;

        // Y de la barre de transition : entre source et destination
        let bar_y = (sy_bot + dy_top) / 2.0;

        // 1. Ligne verticale : bas source → barre (sur l'axe X source)
        painter.line_segment([Pos2::new(sx, sy_bot), Pos2::new(sx, bar_y)], stroke);

        // 2. Barre de transition centrée sur l'axe X source
        let tw = TRANS_W * zoom / 2.0;
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(sx - tw, bar_y - TRANS_H * zoom / 2.0),
                Pos2::new(sx + tw, bar_y + TRANS_H * zoom / 2.0),
            ),
            CornerRadius::ZERO,
            C_TRANS,
        );

        // 3. Condition de transition (à droite de la barre)
        let font = FontId::proportional(11.0 * zoom);
        painter.text(
            Pos2::new(sx + tw + 6.0 * zoom, bar_y),
            egui::Align2::LEFT_CENTER,
            &t.condition,
            font,
            C_COND,
        );

        // 4. Ligne horizontale barre → axe X destination (si décalage)
        if (sx - dx).abs() > 2.0 {
            painter.line_segment([Pos2::new(sx, bar_y), Pos2::new(dx, bar_y)], stroke);
        }

        // 5. Ligne verticale : niveau barre → haut destination (sur l'axe X destination)
        painter.line_segment([Pos2::new(dx, bar_y), Pos2::new(dx, dy_top)], stroke);
        draw_arrow(painter, Pos2::new(dx, dy_top), arrow_stroke, zoom);
    }
}

/// Dessine une pointe de flèche vers le bas à la position donnée.
fn draw_arrow(painter: &Painter, tip: Pos2, stroke: Stroke, zoom: f32) {
    let sz = 7.0 * zoom;
    painter.line_segment([tip, Pos2::new(tip.x - sz / 2.0, tip.y - sz)], stroke);
    painter.line_segment([tip, Pos2::new(tip.x + sz / 2.0, tip.y - sz)], stroke);
}

/// Dessine toutes les étapes du grafcet.
pub fn draw_steps(painter: &Painter, grafcet: &Grafcet, offset: Vec2, zoom: f32) {
    for step in &grafcet.steps {
        let rect = step_rect(step.pos, offset, zoom);
        let bg = if step.active {
            C_STEP_ACTIVE
        } else if step.kind == StepKind::Initial {
            C_STEP_INITIAL
        } else {
            C_STEP_NORMAL
        };

        let rounding = CornerRadius::same((4.0 * zoom).round().clamp(0.0, 255.0) as u8);
        painter.rect_filled(rect, rounding, bg);
        painter.rect_stroke(rect, rounding, Stroke::new(1.5 * zoom, C_BORDER), egui::StrokeKind::Outside);

        // Double bordure pour étape initiale
        if step.kind == StepKind::Initial {
            let inner = rect.shrink(4.0 * zoom);
            painter.rect_stroke(inner, rounding, Stroke::new(1.0 * zoom, C_BORDER), egui::StrokeKind::Outside);
        }

        // Numéro de l'étape
        let font_num = FontId::monospace(14.0 * zoom);
        painter.text(
            rect.min + Vec2::new(8.0 * zoom, rect.height() / 2.0),
            egui::Align2::LEFT_CENTER,
            format!("{}", step.id),
            font_num,
            Color32::from_rgb(180, 220, 255),
        );

        // Label centré
        let font_lbl = FontId::proportional(11.0 * zoom);
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            &step.label,
            font_lbl,
            Color32::from_rgb(230, 230, 230),
        );

        // Actions (petite police, à gauche sous le label)
        let font_act = FontId::monospace(9.0 * zoom);
        for (i, action) in step.actions.iter().enumerate() {
            painter.text(
                Pos2::new(
                    rect.min.x + 8.0 * zoom,
                    rect.center().y + 14.0 * zoom + i as f32 * 11.0 * zoom,
                ),
                egui::Align2::LEFT_CENTER,
                action,
                font_act.clone(),
                Color32::from_rgb(100, 220, 140),
            );
        }
    }
}
