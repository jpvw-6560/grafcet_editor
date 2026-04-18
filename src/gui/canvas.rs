use egui::{Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, Vec2};
use std::collections::{HashMap, HashSet};

use crate::grafcet::{Grafcet, StepKind};

// ── Constantes visuelles ──────────────────────────────────────────────────────
pub const STEP_W: f32 = 55.0;   // corps étape
pub const STEP_H: f32 = 50.0;   // corps étape
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
    let stroke     = Stroke::new(1.5 * zoom, C_LINK);
    let and_stroke = Stroke::new(2.0 * zoom, C_LINK);

    // ── Groupes ET (and_group) ────────────────────────────────────────────
    const AND_GAP:   f32 = 3.5;  // demi-écart entre les deux lignes ET (logique px)
    const AND_EXTRA: f32 = 14.0; // extension latérale de la double barre ET
    const OR_EXTRA:  f32 = 8.0;  // extension latérale de la barre OU (au-delà du bord de la barre)
    const ARR_H:     f32 = 5.0;  // hauteur pointe de flèche (coords logiques)
    const ARR_W:     f32 = 3.5;  // demi-largeur pointe de flèche

    let ah = ARR_H * zoom;
    let aw = ARR_W * zoom;
    // Pointe de flèche vers le bas, tip à (x, tip_y)
    let arrow_down = |x: f32, tip_y: f32| {
        painter.add(egui::Shape::convex_polygon(
            vec![
                Pos2::new(x - aw, tip_y - ah),
                Pos2::new(x + aw, tip_y - ah),
                Pos2::new(x,      tip_y),
            ],
            C_LINK, Stroke::NONE,
        ));
    };
    // Pointe de flèche vers le haut, tip à (x, tip_y)
    let arrow_up = |x: f32, tip_y: f32| {
        painter.add(egui::Shape::convex_polygon(
            vec![
                Pos2::new(x - aw, tip_y + ah),
                Pos2::new(x + aw, tip_y + ah),
                Pos2::new(x,      tip_y),
            ],
            C_LINK, Stroke::NONE,
        ));
    };

    let mut and_groups: HashMap<u32, Vec<u32>> = HashMap::new();
    for t in &grafcet.transitions {
        if let Some(gid) = t.and_group {
            and_groups.entry(gid).or_default().push(t.id);
        }
    }
    // Transitions membres d'un groupe ET (≥ 2 membres) → routing géré séparément
    let and_trans: HashSet<u32> = and_groups.values()
        .filter(|v| v.len() >= 2)
        .flat_map(|v| v.iter().cloned())
        .collect();

    // ── Back-edges : dst_route_x OU détection géométrique ──────────────────
    // (détection géométrique : la destination est géométriquement au-dessus de la source)
    let is_back: HashSet<u32> = grafcet.transitions.iter()
        .filter(|t| !and_trans.contains(&t.id))
        .filter(|t| {
            if t.dst_route_x.is_some() { return true; }
            if let (Some(src), Some(dst)) = (grafcet.step(t.from_step), grafcet.step(t.to_step)) {
                let t_bot = t.pos[1] + TRANS_WICK;
                let d_top = dst.pos[1] - STEP_H / 2.0 - STEP_WICK;
                t_bot > d_top + 1.0
            } else { false }
        })
        .map(|t| t.id)
        .collect();

    // Seuil vertical (coords logiques) en-deçà duquel on dessine la boucle réelle
    const RENVOI_THRESHOLD: f32 = 240.0;
    let use_loop: HashSet<u32> = {
        let mut s = HashSet::new();
        for t in &grafcet.transitions {
            if is_back.contains(&t.id) {
                if let (Some(src), Some(dst)) = (grafcet.step(t.from_step), grafcet.step(t.to_step)) {
                    if src.pos[1] - dst.pos[1] < RENVOI_THRESHOLD {
                        s.insert(t.id);
                    }
                }
            }
        }
        s
    };

    // ── Forward uniquement (non back-edge, non ET) ────────────────────────
    let mut by_source: HashMap<u32, Vec<u32>> = HashMap::new();
    for t in &grafcet.transitions {
        if !and_trans.contains(&t.id) && !is_back.contains(&t.id) {
            by_source.entry(t.from_step).or_default().push(t.id);
        }
    }
    let mut by_dest: HashMap<u32, Vec<u32>> = HashMap::new();
    for t in &grafcet.transitions {
        if !and_trans.contains(&t.id) && !is_back.contains(&t.id) {
            by_dest.entry(t.to_step).or_default().push(t.id);
        }
    }
    let conv_trans: HashSet<u32> = by_dest.values()
        .filter(|v| v.len() >= 2)
        .flat_map(|v| v.iter().cloned())
        .collect();

    // Renvois d'entrée uniquement pour les back-edges longs (pas use_loop)
    let mut entry_renvois: HashMap<u32, Vec<String>> = HashMap::new();
    for t in &grafcet.transitions {
        if is_back.contains(&t.id) && !use_loop.contains(&t.id) {
            entry_renvois.entry(t.to_step)
                .or_default()
                .push(format!("Y{}", t.id));
        }
    }

    // ── Barres de divergence OU ───────────────────────────────────────────
    for (from_id, trans_ids) in &by_source {
        if trans_ids.len() < 2 { continue; }
        let src = match grafcet.step(*from_id) { Some(s) => s, None => continue };
        let sx = src.pos[0] * zoom + offset.x;
        let sy_anchor = src.pos[1] * zoom + offset.y + (STEP_H / 2.0 + STEP_WICK) * zoom;
        let txs: Vec<f32> = trans_ids.iter()
            .filter_map(|&tid| grafcet.transition(tid))
            .map(|t| t.pos[0] * zoom + offset.x)
            .collect();
        if txs.is_empty() { continue; }
        let all_xs: Vec<f32> = txs.iter().cloned().chain(std::iter::once(sx)).collect();
        let x_min = all_xs.iter().cloned().fold(f32::INFINITY,    f32::min) - OR_EXTRA * zoom;
        let x_max = all_xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max) + OR_EXTRA * zoom;
        painter.line_segment([Pos2::new(x_min, sy_anchor), Pos2::new(x_max, sy_anchor)],
            Stroke::new(2.0 * zoom, C_LINK));
    }

    // ── Liaisons individuelles (transitions non-ET) ───────────────────────
    for t in &grafcet.transitions {
        if and_trans.contains(&t.id) { continue; }
        let back  = is_back.contains(&t.id);
        let is_div = !back && by_source.get(&t.from_step).map_or(false, |v| v.len() > 1);
        let src = match grafcet.step(t.from_step) { Some(s) => s, None => continue };
        let dst = match grafcet.step(t.to_step)   { Some(s) => s, None => continue };

        let sx = src.pos[0] * zoom + offset.x;
        let sy_anchor = src.pos[1] * zoom + offset.y + (STEP_H / 2.0 + STEP_WICK) * zoom;
        let tx       = t.pos[0] * zoom + offset.x;
        let ty       = t.pos[1] * zoom + offset.y;
        let th       = TRANS_H * zoom / 2.0;
        let t_top_y  = ty - TRANS_WICK * zoom;
        let t_bot_y  = ty + TRANS_WICK * zoom;
        let dx       = dst.pos[0] * zoom + offset.x;
        let dy_anchor = dst.pos[1] * zoom + offset.y - (STEP_H / 2.0 + STEP_WICK) * zoom;

        // Mèches internes de la barre de transition
        painter.line_segment([Pos2::new(tx, t_top_y), Pos2::new(tx, ty - th)], stroke);
        painter.line_segment([Pos2::new(tx, ty + th), Pos2::new(tx, t_bot_y)], stroke);

        // ── Source → mèche haute ─────────────────────────────────────────
        if is_div {
            painter.line_segment([Pos2::new(tx, sy_anchor), Pos2::new(tx, t_top_y)], stroke);
        } else if (sx - tx).abs() > 2.0 {
            painter.line_segment([Pos2::new(sx, sy_anchor), Pos2::new(tx, sy_anchor)], stroke);
            painter.line_segment([Pos2::new(tx, sy_anchor), Pos2::new(tx, t_top_y)], stroke);
        } else {
            painter.line_segment([Pos2::new(sx, sy_anchor), Pos2::new(tx, t_top_y)], stroke);
        }

        // ── Mèche basse → destination ─────────────────────────────────────
        if back && use_loop.contains(&t.id) {
            // Boucle latérale réelle : flèche montante sur le segment vertical gauche
            let route_x = (t.dst_route_x.unwrap_or(src.pos[0] - 60.0)) * zoom + offset.x;
            painter.line_segment([Pos2::new(tx, t_bot_y),        Pos2::new(route_x, t_bot_y)],   stroke);
            painter.line_segment([Pos2::new(route_x, t_bot_y),   Pos2::new(route_x, dy_anchor)], stroke);
            painter.line_segment([Pos2::new(route_x, dy_anchor),  Pos2::new(dx, dy_anchor)],      stroke);
            // Pointe de flèche MONTANTE au milieu du segment vertical gauche
            let mid_vert = (t_bot_y + dy_anchor) / 2.0;
            arrow_up(route_x, mid_vert);
        } else if back {
            // Renvoi de sortie : flèche pleine pointant vers le bas + id étape
            // (pas de cercle/"sucette")
            let arr_h   = 8.0 * zoom;
            let arr_w   = 5.0 * zoom;
            let line_end = t_bot_y + 5.0 * zoom;
            let arr_tip  = line_end + arr_h;
            painter.line_segment([Pos2::new(tx, t_bot_y), Pos2::new(tx, line_end)], stroke);
            painter.add(egui::Shape::convex_polygon(
                vec![
                    Pos2::new(tx - arr_w, line_end),
                    Pos2::new(tx + arr_w, line_end),
                    Pos2::new(tx,         arr_tip),
                ],
                C_LINK,
                Stroke::NONE,
            ));
            painter.text(
                Pos2::new(tx, arr_tip + 2.0 * zoom),
                egui::Align2::CENTER_TOP,
                format!("X{}", dst.id),
                FontId::proportional(9.0 * zoom),
                Color32::from_rgb(180, 230, 160),
            );
        } else if conv_trans.contains(&t.id) {
            // Convergence OU forward : descendre verticalement jusqu'à la barre
            painter.line_segment([Pos2::new(tx, t_bot_y), Pos2::new(tx, dy_anchor)], stroke);
        } else {
            // Liaison simple forward : L-shape, sans pointe de flèche (direction implicite)
            if (tx - dx).abs() > 2.0 {
                painter.line_segment([Pos2::new(tx, t_bot_y), Pos2::new(dx, t_bot_y)], stroke);
            }
            painter.line_segment([Pos2::new(dx, t_bot_y), Pos2::new(dx, dy_anchor)], stroke);
        }
    }

    // ── Renvois d'entrée : queue-de-flèche (T) + barre OU si plusieurs  ──
    // Notation : label → petit trait vertical → T horizontal (queue de flèche)
    // aligné sur dy_anchor - MARK_H. Si ≥ 2 renvois : barre OU les relie.
    const MARK_W: f32   = 7.0;   // demi-largeur du T
    const MARK_H: f32   = 22.0;  // hauteur stub (T → step entry)
    const ENTRY_SP: f32 = 34.0;  // espacement horizontal entre renvois
    for (dest_id, conditions) in &entry_renvois {
        let dst = match grafcet.step(*dest_id) { Some(s) => s, None => continue };
        let dx        = dst.pos[0] * zoom + offset.x;
        let dy_anchor = dst.pos[1] * zoom + offset.y - (STEP_H / 2.0 + STEP_WICK) * zoom;
        let n  = conditions.len();
        let mw = MARK_W  * zoom;
        let mark_y = dy_anchor - MARK_H * zoom;
        let sp = ENTRY_SP * zoom;
        let xs: Vec<f32> = (0..n)
            .map(|i| dx + (i as f32 - (n as f32 - 1.0) / 2.0) * sp)
            .collect();

        if n == 1 {
            let ex = xs[0];
            // Trait au-dessus du T
            painter.line_segment(
                [Pos2::new(ex, mark_y - 8.0 * zoom), Pos2::new(ex, mark_y)], stroke);
            // T horizontal (queue de flèche)
            painter.line_segment(
                [Pos2::new(ex - mw, mark_y), Pos2::new(ex + mw, mark_y)],
                Stroke::new(1.8 * zoom, C_LINK));
            // T → step entry + pointe de flèche
            painter.line_segment([Pos2::new(ex, mark_y), Pos2::new(ex, dy_anchor - ah)], stroke);
            arrow_down(ex, dy_anchor);
            // Label
            painter.text(
                Pos2::new(ex, mark_y - 10.0 * zoom),
                egui::Align2::CENTER_BOTTOM,
                conditions[0].as_str(),
                FontId::proportional(9.0 * zoom),
                C_COND,
            );
        } else {
            // ≥ 2 : barre OU + stubs individuels
            let x_min = xs[0]   - OR_EXTRA * zoom;
            let x_max = xs[n-1] + OR_EXTRA * zoom;
            // Barre OU
            painter.line_segment(
                [Pos2::new(x_min, mark_y), Pos2::new(x_max, mark_y)],
                Stroke::new(2.0 * zoom, C_LINK));
            // Trait unique barre OU → step (depuis le centre = dx) + pointe de flèche
            painter.line_segment([Pos2::new(dx, mark_y), Pos2::new(dx, dy_anchor - ah)], stroke);
            arrow_down(dx, dy_anchor);
            // Stubs + labels
            for (i, condition) in conditions.iter().enumerate() {
                let ex = xs[i];
                painter.line_segment(
                    [Pos2::new(ex, mark_y - 8.0 * zoom), Pos2::new(ex, mark_y)], stroke);
                painter.text(
                    Pos2::new(ex, mark_y - 10.0 * zoom),
                    egui::Align2::CENTER_BOTTOM,
                    condition.as_str(),
                    FontId::proportional(9.0 * zoom),
                    C_COND,
                );
            }
        }
    }

    // ── Barres de convergence OU (forward uniquement) ─────────────────────
    for (to_id, trans_ids) in &by_dest {
        if trans_ids.len() < 2 { continue; }
        let trans_grp: Vec<_> = trans_ids.iter().filter_map(|&tid| grafcet.transition(tid)).collect();
        let dst = match grafcet.step(*to_id) { Some(s) => s, None => continue };
        let dx        = dst.pos[0] * zoom + offset.x;
        let dy_anchor = dst.pos[1] * zoom + offset.y - (STEP_H / 2.0 + STEP_WICK) * zoom;

        let txs: Vec<f32> = trans_grp.iter()
            .map(|t| t.pos[0] * zoom + offset.x)
            .collect();
        if txs.len() < 2 { continue; }

        let all_xs: Vec<f32> = txs.iter().cloned().chain(std::iter::once(dx)).collect();
        let x_min = all_xs.iter().cloned().fold(f32::INFINITY,    f32::min) - OR_EXTRA * zoom;
        let x_max = all_xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max) + OR_EXTRA * zoom;
        painter.line_segment(
            [Pos2::new(x_min, dy_anchor), Pos2::new(x_max, dy_anchor)],
            Stroke::new(2.0 * zoom, C_LINK),
        );
        arrow_down(dx, dy_anchor);
    }

    // ── Liaisons ET  (double barre ═══) ──────────────────────────────────
    for (_gid, tids) in &and_groups {
        if tids.len() < 2 { continue; }
        let trans: Vec<_> = tids.iter().filter_map(|&id| grafcet.transition(id)).collect();

        let unique_from: HashSet<u32> = trans.iter().map(|t| t.from_step).collect();
        let unique_to:   HashSet<u32> = trans.iter().map(|t| t.to_step).collect();

        // Y moyen des barres de transition du groupe (auto-layout les place au même Y)
        let avg_bar_y = trans.iter().map(|t| t.pos[1]).sum::<f32>() / trans.len() as f32;
        let bar_ys = avg_bar_y * zoom + offset.y;
        let g = AND_GAP * zoom;

        let draw_arm = |painter: &Painter, tx: f32, t_top: f32, th: f32, ty: f32, t_bot: f32,
                        dst_x: f32, dst_y: f32, stroke: Stroke| {
            painter.line_segment([Pos2::new(tx, t_top), Pos2::new(tx, ty - th)], stroke);
            painter.line_segment([Pos2::new(tx, ty + th), Pos2::new(tx, t_bot)], stroke);
            if (tx - dst_x).abs() > 2.0 {
                painter.line_segment([Pos2::new(tx, t_bot), Pos2::new(dst_x, t_bot)], stroke);
            }
            painter.line_segment([Pos2::new(dst_x, t_bot), Pos2::new(dst_x, dst_y)], stroke);
        };

        // ── AND DIV (même étape source) ─────────────────────────────────
        if unique_from.len() == 1 {
            let src_id = *unique_from.iter().next().unwrap();
            let src = match grafcet.step(src_id) { Some(s) => s, None => continue };
            let sx       = src.pos[0] * zoom + offset.x;
            let sy_bot   = src.pos[1] * zoom + offset.y + (STEP_H / 2.0 + STEP_WICK) * zoom;

            let txs: Vec<f32> = trans.iter().map(|t| t.pos[0] * zoom + offset.x).collect();
            let x_min = txs.iter().cloned().fold(f32::INFINITY,    f32::min) - (TRANS_W / 2.0 + AND_EXTRA) * zoom;
            let x_max = txs.iter().cloned().fold(f32::NEG_INFINITY, f32::max) + (TRANS_W / 2.0 + AND_EXTRA) * zoom;

            // Ligne source → première ligne ET
            painter.line_segment([Pos2::new(sx, sy_bot), Pos2::new(sx, bar_ys - g)], stroke);
            // Double barre EST horizontale
            painter.line_segment([Pos2::new(x_min, bar_ys - g), Pos2::new(x_max, bar_ys - g)], and_stroke);
            painter.line_segment([Pos2::new(x_min, bar_ys + g), Pos2::new(x_max, bar_ys + g)], and_stroke);

            for t in &trans {
                let tx  = t.pos[0] * zoom + offset.x;
                let ty  = t.pos[1] * zoom + offset.y;
                let th  = TRANS_H * zoom / 2.0;
                let dst = match grafcet.step(t.to_step) { Some(s) => s, None => continue };
                let dx  = dst.pos[0] * zoom + offset.x;
                let dy  = dst.pos[1] * zoom + offset.y - (STEP_H / 2.0 + STEP_WICK) * zoom;
                // Descente de la double barre vers la barre de transition individuelle
                painter.line_segment([Pos2::new(tx, bar_ys + g), Pos2::new(tx, ty - TRANS_WICK * zoom)], stroke);
                draw_arm(painter, tx, ty - TRANS_WICK*zoom, th, ty, ty + TRANS_WICK*zoom, dx, dy, stroke);
            }
        }

        // ── AND CONV (même étape destination) ───────────────────────────
        if unique_to.len() == 1 {
            let dst_id = *unique_to.iter().next().unwrap();
            let dst = match grafcet.step(dst_id) { Some(s) => s, None => continue };
            let dx       = dst.pos[0] * zoom + offset.x;
            let dy_anchor = dst.pos[1] * zoom + offset.y - (STEP_H / 2.0 + STEP_WICK) * zoom;

            let txs: Vec<f32> = trans.iter().map(|t| t.pos[0] * zoom + offset.x).collect();
            let x_min = txs.iter().cloned().fold(f32::INFINITY,    f32::min) - (TRANS_W / 2.0 + AND_EXTRA) * zoom;
            let x_max = txs.iter().cloned().fold(f32::NEG_INFINITY, f32::max) + (TRANS_W / 2.0 + AND_EXTRA) * zoom;

            // Chaque branche : source → barre individuelle → double barre ET
            for t in &trans {
                let src = match grafcet.step(t.from_step) { Some(s) => s, None => continue };
                let sx      = src.pos[0] * zoom + offset.x;
                let sy_bot  = src.pos[1] * zoom + offset.y + (STEP_H / 2.0 + STEP_WICK) * zoom;
                let tx  = t.pos[0] * zoom + offset.x;
                let ty  = t.pos[1] * zoom + offset.y;
                let th  = TRANS_H * zoom / 2.0;
                let t_top = ty - TRANS_WICK * zoom;
                // Source → top de la barre (L-shape)
                if (sx - tx).abs() > 2.0 {
                    painter.line_segment([Pos2::new(sx, sy_bot), Pos2::new(tx, sy_bot)], stroke);
                    painter.line_segment([Pos2::new(tx, sy_bot), Pos2::new(tx, t_top)], stroke);
                } else {
                    painter.line_segment([Pos2::new(sx, sy_bot), Pos2::new(tx, t_top)], stroke);
                }
                // Mèches de la barre individuelle
                painter.line_segment([Pos2::new(tx, t_top), Pos2::new(tx, ty - th)], stroke);
                painter.line_segment([Pos2::new(tx, ty + th), Pos2::new(tx, ty + TRANS_WICK*zoom)], stroke);
                // Remontée de la barre vers la double barre ET
                painter.line_segment([Pos2::new(tx, ty + TRANS_WICK*zoom), Pos2::new(tx, bar_ys - g)], stroke);
            }
            // Double barre ET horizontale
            painter.line_segment([Pos2::new(x_min, bar_ys - g), Pos2::new(x_max, bar_ys - g)], and_stroke);
            painter.line_segment([Pos2::new(x_min, bar_ys + g), Pos2::new(x_max, bar_ys + g)], and_stroke);
            // Ligne de la double barre vers la destination
            painter.line_segment([Pos2::new(dx, bar_ys + g), Pos2::new(dx, dy_anchor)], stroke);
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
            format!("Y{}", t.id),
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

    // Numéro de l'étape (X0, X1…) — toujours centré dans le carré (actions hors carré)
    let font_num = FontId::monospace(13.0 * zoom);
    // Séparer les ordres de forçage (F/...) des actions normales
    let normal_actions: Vec<&str> = actions.iter().filter(|a| !a.starts_with("F/")).map(|s| s.as_str()).collect();
    let force_actions:  Vec<&str> = actions.iter().filter(|a|  a.starts_with("F/")).map(|s| s.as_str()).collect();

    // Numéro toujours au centre du carré
    painter.text(
        Pos2::new(rect.center().x, rect.center().y),
        egui::Align2::CENTER_CENTER,
        format!("X{id}"),
        font_num,
        Color32::from_rgb(180, 220, 255),
    );

    // ── Contenu à droite du carré ─────────────────────────────────────────
    // Disposition : [segment]—[boîtes d'actions]  puis  "label"
    // Les actions normales sont dessinées dans un rectangle à bord plein.
    const ACTION_W:  f32 = 80.0;  // largeur boîte action (coords logiques canvas)
    const ACTION_H:  f32 = 16.0;  // hauteur boîte action
    const ACTION_GAP: f32 = 2.0;  // espace entre boîtes
    const FORCE_W:   f32 = 80.0;  // largeur boîte forçage
    const FORCE_H:   f32 = 18.0;
    const FORCE_GAP: f32 = 2.0;
    const CONN_GAP:  f32 = 8.0;   // espace entre bord du carré et première boîte

    let action_box_w = ACTION_W * zoom;
    let action_box_h = ACTION_H * zoom;
    let action_gap   = ACTION_GAP * zoom;
    let force_box_w  = FORCE_W * zoom;
    let force_box_h  = FORCE_H * zoom;
    let force_gap    = FORCE_GAP * zoom;
    let conn_gap     = CONN_GAP * zoom;

    let font_act   = FontId::monospace(9.5 * zoom);
    let font_force = FontId::proportional(9.5 * zoom);

    // X de départ (bord droit du carré + espace)
    let items_x = rect.max.x + conn_gap;

    // Calculer la hauteur totale des boîtes pour centrer le bloc verticalement
    let n_act   = normal_actions.len() as f32;
    let n_force = force_actions.len() as f32;
    let n_total = n_act + n_force;
    let total_h = if n_total == 0.0 { 0.0 } else {
        n_act   * action_box_h + (n_act   - 1.0).max(0.0) * action_gap
        + n_force * force_box_h + (n_force - 1.0).max(0.0) * force_gap
        + if n_act > 0.0 && n_force > 0.0 { force_gap } else { 0.0 }
    };
    let y0 = rect.center().y - total_h / 2.0;

    let dash  = (4.0 * zoom).max(2.0);
    let gap_d = (3.0 * zoom).max(1.5);

    // ── Actions normales : boîte à bord plein ─────────────────────────────
    let mut row_y = y0;
    for (_i, action) in normal_actions.iter().enumerate() {
        let brect = egui::Rect::from_min_size(
            Pos2::new(items_x, row_y),
            egui::Vec2::new(action_box_w, action_box_h),
        );
        let mid_y = brect.center().y;
        // Ligne connectrice
        painter.line_segment(
            [Pos2::new(rect.max.x, mid_y), Pos2::new(items_x, mid_y)],
            Stroke::new(1.2 * zoom, border),
        );
        // Contour plein
        painter.rect_stroke(
            brect, CornerRadius::ZERO,
            Stroke::new(1.2 * zoom, border),
            egui::StrokeKind::Outside,
        );
        // Texte
        painter.text(
            brect.center(),
            egui::Align2::CENTER_CENTER,
            *action,
            font_act.clone(),
            Color32::from_rgb(100, 220, 140),
        );
        row_y += action_box_h + action_gap;
    }

    // ── Actions de forçage : boîte pointillée ────────────────────────────
    let dashed_stroke = Stroke::new(1.2 * zoom, border);
    for (_i, action) in force_actions.iter().enumerate() {
        let brect = egui::Rect::from_min_size(
            Pos2::new(items_x, row_y),
            egui::Vec2::new(force_box_w, force_box_h),
        );
        let mid_y = brect.center().y;
        // Ligne connectrice
        painter.line_segment(
            [Pos2::new(rect.max.x, mid_y), Pos2::new(items_x, mid_y)],
            Stroke::new(1.2 * zoom, border),
        );
        // Contour pointillé
        let corners = [
            brect.min,
            Pos2::new(brect.max.x, brect.min.y),
            brect.max,
            Pos2::new(brect.min.x, brect.max.y),
        ];
        for j in 0..4usize {
            for shape in egui::Shape::dashed_line(
                &[corners[j], corners[(j + 1) % 4]],
                dashed_stroke,
                dash,
                gap_d,
            ) {
                painter.add(shape);
            }
        }
        // Texte
        painter.text(
            brect.center(),
            egui::Align2::CENTER_CENTER,
            *action,
            font_force.clone(),
            Color32::from_rgb(120, 210, 255),
        );
        row_y += force_box_h + force_gap;
    }

    // ── Label commentaire au-dessus des boîtes d'actions ─────────────────
    if !label.is_empty() {
        let font_lbl = FontId::proportional(10.0 * zoom);
        if n_total == 0.0 {
            // Pas d'actions : label à droite du carré (centré verticalement)
            painter.text(
                Pos2::new(rect.max.x + 7.0 * zoom, rect.center().y),
                egui::Align2::LEFT_CENTER,
                label,
                font_lbl,
                Color32::from_rgb(160, 190, 220),
            );
        } else {
            // Actions présentes : label au-dessus de la première boîte
            painter.text(
                Pos2::new(items_x, y0 - 3.0 * zoom),
                egui::Align2::LEFT_BOTTOM,
                label,
                font_lbl,
                Color32::from_rgb(160, 190, 220),
            );
        }
    }
}

/// Dessine une étape fantôme (preview de placement) sous le curseur.
pub fn draw_step_ghost(painter: &Painter, pos: [f32; 2], offset: Vec2, zoom: f32) {
    draw_one_step(
        painter, pos, false, &StepKind::Normal,
        0, "", &[], offset, zoom, 0.45,
    );
}
