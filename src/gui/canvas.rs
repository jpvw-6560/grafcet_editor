use egui::{Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, Vec2};

use crate::grafcet::{Grafcet, StepKind};

// ── Constantes visuelles ──────────────────────────────────────────────────────
pub const STEP_W: f32 = 80.0;   // corps carré
pub const STEP_H: f32 = 80.0;   // corps carré
pub const STEP_WICK: f32 = 30.0; // longueur des mèches d'une étape
pub const TRANS_W: f32 = 60.0;
pub const TRANS_H: f32 = 3.0;   // corps « nul » = simple barre
pub const TRANS_WICK: f32 = 25.0; // longueur des mèches d'une transition

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

/// Dessine les segments de liaison uniquement (passe 1 : fond).
/// La barre de transition est dessinée dans `draw_transitions` (passe 3).
pub fn draw_links(painter: &Painter, grafcet: &Grafcet, offset: Vec2, zoom: f32) {
    let stroke = Stroke::new(1.5 * zoom, C_LINK);
    let arrow_stroke = Stroke::new(1.5 * zoom, C_ARROW);

    // Groupe les transitions par étape source pour détecter les divergences en OU
    let mut by_source: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
    for t in &grafcet.transitions {
        by_source.entry(t.from_step).or_default().push(t.id);
    }

    // Barre horizontale simple de divergence en OU
    // s'étend du bord gauche de la transition la plus à gauche
    // au bord droit de la transition la plus à droite
    for (from_id, trans_ids) in &by_source {
        if trans_ids.len() < 2 { continue; }
        let src = match grafcet.step(*from_id) { Some(s) => s, None => continue };
        let sx = src.pos[0] * zoom + offset.x;
        let sy_anchor = src.pos[1] * zoom + offset.y + (STEP_H / 2.0 + STEP_WICK) * zoom;
        let txs: Vec<f32> = trans_ids.iter()
            .filter_map(|&tid| grafcet.transition(tid))
            .map(|t| t.pos[0] * zoom + offset.x)
            .collect();
        let half_tw = TRANS_W * zoom / 2.0;
        let x_min = txs.iter().cloned().fold(sx, f32::min) - half_tw;
        let x_max = txs.iter().cloned().fold(sx, f32::max) + half_tw;
        let bar_stroke = Stroke::new(2.0 * zoom, C_LINK);
        painter.line_segment([Pos2::new(x_min, sy_anchor), Pos2::new(x_max, sy_anchor)], bar_stroke);
    }

    for t in &grafcet.transitions {
        let is_divergence = by_source.get(&t.from_step).map_or(false, |v| v.len() > 1);
        let src = grafcet.step(t.from_step);
        let dst = grafcet.step(t.to_step);
        if src.is_none() || dst.is_none() { continue; }
        let src = src.unwrap();
        let dst = dst.unwrap();

        // Positions écran (transition a sa propre position absolue)
        let sx = src.pos[0] * zoom + offset.x;
        // route_y override (canvas → écran) pour le segment horizontal src→barre
        let sy_anchor = if let Some(ry) = t.route_y {
            ry * zoom + offset.y
        } else {
            src.pos[1] * zoom + offset.y + (STEP_H / 2.0 + STEP_WICK) * zoom
        };
        let tx = t.pos[0] * zoom + offset.x;
        let ty = t.pos[1] * zoom + offset.y;
        let th = TRANS_H * zoom / 2.0;
        let t_top_y = ty - TRANS_WICK * zoom;   // bout mèche haute
        let t_bot_y = ty + TRANS_WICK * zoom;   // bout mèche basse
        let dx = dst.pos[0] * zoom + offset.x;
        let dy_anchor = dst.pos[1] * zoom + offset.y - (STEP_H / 2.0 + STEP_WICK) * zoom;

        // ── Mèches de la transition ──
        painter.line_segment([Pos2::new(tx, t_top_y), Pos2::new(tx, ty - th)], stroke);
        painter.line_segment([Pos2::new(tx, ty + th), Pos2::new(tx, t_bot_y)], stroke);

        // ── Source → mèche haute ──
        // Divergence en OU : drop vertical direct depuis la barre horizontale
        // Transition unique : L-shape depuis l'ancre de l'étape source
        if is_divergence {
            painter.line_segment([Pos2::new(tx, sy_anchor), Pos2::new(tx, t_top_y)], stroke);
        } else if (sx - tx).abs() > 2.0 {
            painter.line_segment([Pos2::new(sx, sy_anchor), Pos2::new(tx, sy_anchor)], stroke);
            painter.line_segment([Pos2::new(tx, sy_anchor), Pos2::new(tx, t_top_y)], stroke);
        } else {
            painter.line_segment([Pos2::new(sx, sy_anchor), Pos2::new(tx, t_top_y)], stroke);
        }

        // ── Mèche basse → destination ──
        let goes_up = dy_anchor < t_bot_y; // destination au-dessus du bas de la transition
        if goes_up {
            let route_x = if let Some(rx) = t.dst_route_x {
                rx * zoom + offset.x
            } else {
                tx.min(dx) - (STEP_W / 2.0 + 25.0) * zoom
            };
            painter.line_segment([Pos2::new(tx, t_bot_y), Pos2::new(route_x, t_bot_y)], stroke);
            painter.line_segment([Pos2::new(route_x, t_bot_y), Pos2::new(route_x, dy_anchor)], stroke);
            painter.line_segment([Pos2::new(route_x, dy_anchor), Pos2::new(dx, dy_anchor)], stroke);
            let mid_y = (t_bot_y + dy_anchor) / 2.0;
            let sz = 7.0 * zoom;
            painter.line_segment([Pos2::new(route_x, mid_y), Pos2::new(route_x - sz/2.0, mid_y + sz)], arrow_stroke);
            painter.line_segment([Pos2::new(route_x, mid_y), Pos2::new(route_x + sz/2.0, mid_y + sz)], arrow_stroke);
        } else {
            // L-shape : horizontal si décalage X, puis vertical vers dy_anchor
            if (tx - dx).abs() > 2.0 {
                painter.line_segment([Pos2::new(tx, t_bot_y), Pos2::new(dx, t_bot_y)], stroke);
            }
            painter.line_segment([Pos2::new(dx, t_bot_y), Pos2::new(dx, dy_anchor)], stroke);
        }
    }
}

/// Dessine les barres de transition (passe 3 : au-dessus des corps d'étapes).
pub fn draw_transitions(
    painter: &Painter,
    grafcet: &Grafcet,
    offset: Vec2,
    zoom: f32,
    selected_trans: Option<u32>,
    hovered_trans: Option<u32>,
) {
    for t in &grafcet.transitions {
        let tx = t.pos[0] * zoom + offset.x;
        let ty = t.pos[1] * zoom + offset.y;
        let tw = TRANS_W * zoom / 2.0;
        let th = TRANS_H * zoom / 2.0;

        let is_active = selected_trans == Some(t.id) || hovered_trans == Some(t.id);
        let bar_color = if selected_trans == Some(t.id) {
            Color32::from_rgb(255, 220, 80)   // sélectionnée → jaune
        } else if hovered_trans == Some(t.id) {
            Color32::from_rgb(200, 240, 255)  // survolée → bleu clair
        } else if t.condition != "1" && !t.condition.is_empty() {
            Color32::WHITE                    // condition explicite → blanc
        } else {
            C_TRANS                           // défaut "1" → gris
        };

        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(tx - tw, ty - th),
                Pos2::new(tx + tw, ty + th),
            ),
            CornerRadius::ZERO,
            bar_color,
        );

        // Contour de la zone de clic si survolée ou sélectionnée
        if is_active {
            let hit_rect = Rect::from_min_max(
                Pos2::new(tx - tw, ty - 10.0 * zoom),
                Pos2::new(tx + tw, ty + 10.0 * zoom),
            );
            painter.rect_stroke(
                hit_rect,
                CornerRadius::same(2),
                Stroke::new(1.0 * zoom, bar_color),
                egui::StrokeKind::Outside,
            );
        }

        let font = FontId::proportional(11.0 * zoom);
        painter.text(
            Pos2::new(tx - tw - 6.0 * zoom, ty),
            egui::Align2::RIGHT_CENTER,
            format!("T{}", t.id),
            font.clone(),
            bar_color,
        );
        painter.text(
            Pos2::new(tx + tw + 6.0 * zoom, ty),
            egui::Align2::LEFT_CENTER,
            &t.condition,
            font,
            C_COND,
        );
    }
}

/// Renvoie l'id de la transition dont la barre est sous `cv` (coordonnées logiques canvas).
pub fn hit_transition(cv: Pos2, grafcet: &Grafcet) -> Option<u32> {
    for t in &grafcet.transitions {
        let tw = TRANS_W / 2.0;
        let hit_h = 15.0;
        if cv.x >= t.pos[0] - tw
            && cv.x <= t.pos[0] + tw
            && cv.y >= t.pos[1] - hit_h
            && cv.y <= t.pos[1] + hit_h
        {
            return Some(t.id);
        }
    }
    None
}

/// `dragging`
pub fn draw_steps(painter: &Painter, grafcet: &Grafcet, offset: Vec2, zoom: f32, dragging: Option<u32>) {
    // Étapes normales d'abord
    for step in &grafcet.steps {
        if Some(step.id) != dragging {
            draw_one_step(painter, step.pos, step.active, &step.kind, step.id, &step.label, &step.actions, offset, zoom, 1.0);
        }
    }
    // Étape glissée par-dessus toutes les autres
    if let Some(did) = dragging {
        if let Some(step) = grafcet.steps.iter().find(|s| s.id == did) {
            draw_one_step(painter, step.pos, step.active, &step.kind, step.id, &step.label, &step.actions, offset, zoom, 1.0);
        }
    }
}

/// Dessine une seule étape (factored pour réutilisation par le ghost).
fn draw_one_step(
    painter: &Painter,
    pos: [f32; 2],
    active: bool,
    kind: &StepKind,
    id: u32,
    label: &str,
    actions: &[String],
    offset: Vec2,
    zoom: f32,
    alpha: f32,  // 0.0–1.0 pour l'opacité (ghost = 0.5)
) {
    let rect = step_rect(pos, offset, zoom);
    let cx = rect.center().x;
    let a = (alpha * 255.0) as u8;

    let bg_base = if active {
        C_STEP_ACTIVE
    } else if *kind == StepKind::Initial {
        C_STEP_INITIAL
    } else {
        C_STEP_NORMAL
    };
    let bg = Color32::from_rgba_unmultiplied(bg_base.r(), bg_base.g(), bg_base.b(), a);
    let border = Color32::from_rgba_unmultiplied(C_BORDER.r(), C_BORDER.g(), C_BORDER.b(), a);

    // Mèches haut/bas (dessinées avant le corps pour être recouvertes aux extrémités)
    let wick_len = STEP_WICK * zoom;
    let wstroke = Stroke::new(1.5 * zoom, border);
    painter.line_segment([Pos2::new(cx, rect.min.y - wick_len), Pos2::new(cx, rect.min.y)], wstroke);
    painter.line_segment([Pos2::new(cx, rect.max.y), Pos2::new(cx, rect.max.y + wick_len)], wstroke);

    let rounding = CornerRadius::same((4.0 * zoom).round().clamp(0.0, 255.0) as u8);
    painter.rect_filled(rect, rounding, bg);
    painter.rect_stroke(rect, rounding, Stroke::new(1.5 * zoom, border), egui::StrokeKind::Outside);

    // Double bordure pour étape initiale
    if *kind == StepKind::Initial {
        let inner = rect.shrink(4.0 * zoom);
        painter.rect_stroke(inner, rounding, Stroke::new(1.0 * zoom, border), egui::StrokeKind::Outside);
    }

    if alpha < 1.0 {
        return; // ghost : pas de texte
    }

    // Numéro de l'étape
    let font_num = FontId::monospace(14.0 * zoom);
    painter.text(
        rect.min + Vec2::new(8.0 * zoom, rect.height() / 2.0),
        egui::Align2::LEFT_CENTER,
        format!("{id}"),
        font_num,
        Color32::from_rgb(180, 220, 255),
    );

    // Label centré
    let font_lbl = FontId::proportional(11.0 * zoom);
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        font_lbl,
        Color32::from_rgb(230, 230, 230),
    );

    // Actions (petite police, à gauche sous le label)
    let font_act = FontId::monospace(9.0 * zoom);
    for (i, action) in actions.iter().enumerate() {
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

/// Dessine une étape fantôme (preview de placement) sous le curseur.
pub fn draw_step_ghost(painter: &Painter, pos: [f32; 2], offset: Vec2, zoom: f32) {
    draw_one_step(
        painter, pos, false, &StepKind::Normal,
        0, "", &[], offset, zoom, 0.45,
    );
}
