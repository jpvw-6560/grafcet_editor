// gui/pages/gemma_page.rs — Canvas GEMMA interactif
//
// Système d'ancrages manuels pour créer les flèches :
//   - Mode "→ Flèche" : clic sur une ancre (N/S/E/W) du rectangle source,
//     puis clic sur l'ancre de destination → crée la transition avec waypoints.
//   - Bouton "💾 Sauvegarder flèches" → exporte les waypoints dans
//     data/gemma_routes.json.
//   - Clic droit sur une flèche → menu contextuel "Supprimer".
//   - Escape annule la flèche en cours.

use egui::{Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, Vec2};

use crate::gemma::{Expr, Gemma, GemmaState, GemmaTransition, StateType};
use crate::gemma::questionnaire::{Answer, Questionnaire};

const NODE_W: f32 = 110.0;
const NODE_H: f32 = 44.0;
const ANCHOR_R:   f32 = 5.0;  // rayon visuel point d'accroche (écran)
const SNAP_DIST:  f32 = 14.0; // distance max pour accrocher au périmètre (px écran)
const ARROW_HIT:  f32 = 6.0;  // distance max clic sur flèche (canvas)
const SEG_HIT_PX: f32 = 12.0; // distance max clic sur handle segment (px écran)

/// Côté de sortie / d'entrée d'un ancrage.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Side { N, S, E, W }

/// Un ancrage sur un état (bord d'un rectangle).
#[derive(Clone, Debug)]
struct Anchor {
    state_id: String,
    side:     Side,
    pos:      [f32; 2], // coordonnées canvas
}

/// Déplacement d'un segment intermédiaire orthogonal (mode Select).
#[derive(Clone)]
struct DragSegment {
    trans_id: u32,
    seg_idx:  usize,           // index du 1er point du segment dans waypoints
    is_horiz: bool,            // vrai = segment H → drag déplace Y
    start_cv: [f32; 2],        // position canvas du curseur au début du drag
    orig_pts: Vec<[f32; 2]>,   // snapshot des waypoints au début du drag
}

/// Déplacement d'une extrémité d'une flèche existante (mode Select).
#[derive(Clone)]
struct DragEndpoint {
    trans_id:     u32,
    is_start:     bool,    // true = on déplace le début, false = la fin
    fixed_anchor: Anchor,  // l'extrémité qui reste fixe
}

#[derive(Default, PartialEq, Clone)]
enum GemmaTool {
    #[default]
    Select,
    AddTransition,
    Delete,
}

pub struct GemmaPage {
    tool:       GemmaTool,
    offset:     Vec2,
    zoom:       f32,
    dragging:   Option<String>,
    drag_offset: Vec2,
    selected_state:  Option<String>,
    selected_trans:  Option<u32>,
    editing_cond:    Option<(u32, String)>,
    editing_action:  Option<String>,
    show_questionnaire: bool,
    questionnaire:      Questionnaire,
    pub pending_fit:  bool,
    pub needs_save:   bool,
    // Création de flèche manuelle
    pending_from: Option<Anchor>,          // ancre source en attente de la destination
    // Menu contextuel clic droit sur flèche
    ctx_menu_trans: Option<(u32, Pos2)>,   // (trans_id, pos écran)
    // Déplacement d'une extrémité de flèche (clic sur point doré/cyan en mode Select)
    dragging_ep: Option<DragEndpoint>,
    // Déplacement d'un segment intermédiaire (clic sur handle carré en mode Select)
    dragging_seg: Option<DragSegment>,
    // Dialogue de confirmation de réinitialisation
    confirm_reset: bool,
    // ── Simulation ─────────────────────────────────────────────────────────
    sim_active:  bool,
    sim_state:   String,              // ID de l'état actif courant
    sim_history: Vec<String>,         // Log : "A1 →[cond]→ F1"
}

impl Default for GemmaPage {
    fn default() -> Self {
        Self {
            tool:       GemmaTool::Select,
            offset:     Vec2::ZERO,
            zoom:       1.0,
            dragging:   None,
            drag_offset: Vec2::ZERO,
            selected_state: None,
            selected_trans: None,
            editing_cond:   None,
            editing_action: None,
            show_questionnaire: false,
            questionnaire: Questionnaire::load(),
            pending_fit:  false,
            needs_save:   false,
            pending_from: None,
            ctx_menu_trans: None,
            confirm_reset: false,
            dragging_ep: None,
            dragging_seg: None,
            sim_active:  false,
            sim_state:   String::new(),
            sim_history: Vec::new(),
        }
    }
}

impl GemmaPage {

    pub fn show(&mut self, ui: &mut egui::Ui, gemma: &mut Gemma) -> Option<String> {
        let ctx = ui.ctx().clone();
        let mut status_out: Option<String> = None;

        // ── Barre d'outils ─────────────────────────────────────────────────
        egui::Panel::top("gemma_toolbar")
            .frame(egui::Frame::new()
                .fill(Color32::from_rgb(26, 37, 47))
                .inner_margin(egui::Margin::same(6)))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    // Questionnaire
                    let is_open = self.show_questionnaire;
                    let q_btn = egui::Button::new(
                        egui::RichText::new("📋  Questionnaire").size(12.0)
                    ).fill(if is_open { Color32::from_rgb(41, 128, 185) }
                           else { Color32::from_rgb(44, 62, 80) });
                    if ui.add(q_btn).clicked() {
                        self.show_questionnaire = !self.show_questionnaire;
                    }

                    ui.separator();
                    tool_btn(ui, "↖ Sélect",   GemmaTool::Select,        &mut self.tool);
                    tool_btn(ui, "→ Flèche",   GemmaTool::AddTransition, &mut self.tool);
                    tool_btn(ui, "✖ Suppr.",   GemmaTool::Delete,        &mut self.tool);
                    ui.separator();

                    if ui.button("🔍+").clicked() { self.zoom = (self.zoom * 1.2).min(4.0); }
                    if ui.button("🔍−").clicked() { self.zoom = (self.zoom / 1.2).max(0.2); }
                    if ui.button("100%").clicked() { self.zoom = 1.0; self.offset = Vec2::ZERO; }
                    if ui.button("🔲 Ajuster").clicked() { self.pending_fit = true; }
                    ui.separator();

                    // Sauvegarder les flèches
                    if ui.add(egui::Button::new(
                        egui::RichText::new("💾 Sauver flèches").size(12.0)
                    ).fill(Color32::from_rgb(39, 100, 58))).clicked() {
                        match save_routes(gemma) {
                            Ok(path) => status_out = Some(format!("Flèches → {}", path)),
                            Err(e)   => status_out = Some(format!("Erreur : {}", e)),
                        }
                    }

                    // Supprimer TOUTES les flèches
                    if ui.add(egui::Button::new(
                        egui::RichText::new("🗑 Tout effacer").size(12.0)
                    ).fill(Color32::from_rgb(120, 30, 30))).clicked() {
                        gemma.transitions.clear();
                        self.selected_trans = None;
                        self.pending_from   = None;
                        self.needs_save = true;
                        status_out = Some("Toutes les flèches supprimées".to_string());
                    }
                    ui.separator();

                    if ui.button("✔ Valider").clicked() {
                        match gemma.validate() {
                            Ok(())  => status_out = Some("GEMMA valide ✓".to_string()),
                            Err(e)  => status_out = Some(format!("Erreurs : {}", e.join(" | "))),
                        }
                    }

                    ui.separator();

                    // ── Bouton Simulation ─────────────────────────────────
                    if self.sim_active {
                        if ui.add(egui::Button::new(
                            egui::RichText::new("⏹ Arrêter sim.").size(12.0)
                        ).fill(Color32::from_rgb(150, 40, 40))).clicked() {
                            self.sim_active  = false;
                            self.sim_state   = String::new();
                            self.sim_history.clear();
                            status_out = Some("Simulation arrêtée".to_string());
                        }
                        ui.label(
                            egui::RichText::new(format!("⬤ {}", self.sim_state))
                                .size(12.0).strong()
                                .color(Color32::from_rgb(255, 220, 60))
                        );
                    } else {
                        if ui.add(egui::Button::new(
                            egui::RichText::new("▶ Simuler").size(12.0)
                        ).fill(Color32::from_rgb(30, 100, 50))).clicked() {
                            // Démarrer depuis l'état initial (A1 ou premier état)
                            let start = gemma.states.iter()
                                .find(|s| s.id == "A1")
                                .or_else(|| gemma.states.first())
                                .map(|s| s.id.clone())
                                .unwrap_or_default();
                            self.sim_active  = true;
                            self.sim_state   = start.clone();
                            self.sim_history.clear();
                            self.sim_history.push(format!("▶ Départ : {start}"));
                            // Désactiver les outils d'édition
                            self.selected_state = None;
                            self.selected_trans = None;
                            self.editing_cond   = None;
                            self.editing_action = None;
                            status_out = Some(format!("Simulation démarrée — état actif : {start}"));
                        }
                    }

                    // Indice outil courant
                    if self.tool == GemmaTool::AddTransition {
                        ui.separator();
                        let txt = if self.pending_from.is_some() {
                            "→ Approchez le bord de l'état destination"
                        } else {
                            "→ Approchez le bord d'un état pour accrocher"
                        };
                        ui.label(egui::RichText::new(txt).size(11.0)
                            .color(Color32::from_rgb(200, 200, 100)));
                    }
                });
            });

        // ── Questionnaire (gauche) ──────────────────────────────────────────
        if self.show_questionnaire {
            egui::Panel::left("gemma_questionnaire")
                .min_size(320.0)
                .resizable(true)
                .frame(egui::Frame::new()
                    .fill(Color32::from_rgb(20, 30, 40))
                    .inner_margin(egui::Margin::same(10)))
                .show_inside(ui, |ui| {
                    if let Some(msg) = self.draw_questionnaire(ui, gemma) {
                        status_out = Some(msg);
                    }
                });
        }

        // ── Propriétés / Simulation (droite) ───────────────────────────────
        egui::Panel::right("gemma_props")
            .min_size(200.0)
            .resizable(true)
            .frame(egui::Frame::new()
                .fill(Color32::from_rgb(22, 32, 42))
                .inner_margin(egui::Margin::same(8)))
            .show_inside(ui, |ui| {
                if self.sim_active {
                    if let Some(msg) = self.draw_simulation(ui, gemma) {
                        status_out = Some(msg);
                    }
                } else {
                    ui.heading("Propriétés");
                    ui.separator();
                    self.draw_props(ui, gemma);
                }
            });

        // ── Canvas ─────────────────────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::from_rgb(18, 26, 35)))
            .show_inside(ui, |ui| {
                let resp = ui.allocate_rect(
                    ui.available_rect_before_wrap(),
                    egui::Sense::click_and_drag(),
                );
                let painter = ui.painter_at(resp.rect);
                let origin  = resp.rect.min;

                // Fit-to-canvas
                if self.pending_fit && !gemma.states.is_empty() {
                    let (ox, oy, z) = fit_states_to_rect(&gemma.states, resp.rect);
                    self.offset = Vec2::new(ox, oy);
                    self.zoom   = z;
                    self.pending_fit = false;
                }

                draw_grid(&painter, resp.rect, self.offset, self.zoom);

                // ── Flèches ───────────────────────────────────────────────
                // Seules les flèches avec waypoints (posées manuellement) sont affichées.
                let hover_pos_cv = ctx.pointer_hover_pos().map(|hp|
                    to_canvas(hp, origin, self.offset, self.zoom));
                for t in &gemma.transitions {
                    if t.waypoints.is_empty() { continue; }
                    let is_sel = self.selected_trans == Some(t.id);
                    draw_arrow(&painter, t, &gemma.states, self.offset, self.zoom, origin, is_sel);
                }
                // ── Tooltip condition au survol d'une flèche ──────────────
                if let Some(cv) = hover_pos_cv {
                    if let Some(tid) = hit_trans(&gemma.transitions, &gemma.states, cv) {
                        if let Some(t) = gemma.transitions.iter().find(|t| t.id == tid) {
                            if !t.waypoints.is_empty() {
                                let cond = t.condition.to_display();
                                let has_cond = cond != "FALSE" && cond != "TRUE";
                                let line1 = format!("{} -> {}", t.from, t.to);
                                let line2 = if has_cond {
                                    Some(format!("Condition : {}", cond))
                                } else {
                                    None
                                };

                                // Dessin manuel du tooltip sur couche Tooltip (pas de clip)
                                if let Some(mp) = ctx.pointer_hover_pos() {
                                    let tip_painter = ctx.layer_painter(egui::LayerId::new(
                                        egui::Order::Tooltip,
                                        egui::Id::new("gemma_trans_tooltip"),
                                    ));
                                    let font       = egui::FontId::proportional(12.0);
                                    let text_color = Color32::BLACK;
                                    let bg_color   = Color32::from_rgb(255, 253, 180);
                                    let border_col = Color32::from_rgb(80, 80, 80);
                                    let pad = egui::Vec2::new(8.0, 6.0);

                                    // Mesure des deux lignes
                                    let gal1 = ctx.fonts_mut(|f| {
                                        f.layout_no_wrap(line1.clone(), font.clone(), text_color)
                                    });
                                    let w1 = gal1.size().x;
                                    let h1 = gal1.size().y;

                                    let (w2, h2, gal2) = if let Some(ref l2) = line2 {
                                        let g = ctx.fonts_mut(|f| {
                                            f.layout_no_wrap(l2.clone(), font.clone(), text_color)
                                        });
                                        (g.size().x, g.size().y, Some(g))
                                    } else {
                                        (0.0_f32, 0.0_f32, None)
                                    };

                                    let sep = if line2.is_some() { 4.0 } else { 0.0 };
                                    let content_w = w1.max(w2);
                                    let content_h = h1 + sep + h2;

                                    let box_w = content_w + pad.x * 2.0;
                                    let box_h = content_h + pad.y * 2.0;

                                    // Position : décalée en bas-droite du curseur
                                    let offset_tip = egui::Vec2::new(14.0, 14.0);
                                    let mut tl = mp + offset_tip;
                                    // Rester dans la fenêtre
                                    let screen = ctx.screen_rect();
                                    if tl.x + box_w > screen.max.x - 4.0 {
                                        tl.x = mp.x - box_w - 6.0;
                                    }
                                    if tl.y + box_h > screen.max.y - 4.0 {
                                        tl.y = mp.y - box_h - 6.0;
                                    }
                                    let rect = egui::Rect::from_min_size(tl, egui::Vec2::new(box_w, box_h));

                                    tip_painter.rect_filled(rect, 3.0, bg_color);
                                    tip_painter.rect_stroke(
                                        rect, 3.0,
                                        egui::Stroke::new(1.0, border_col),
                                        egui::StrokeKind::Outside,
                                    );
                                    tip_painter.galley(
                                        egui::Pos2::new(tl.x + pad.x, tl.y + pad.y),
                                        gal1, text_color,
                                    );
                                    if let Some(g2) = gal2 {
                                        tip_painter.galley(
                                            egui::Pos2::new(tl.x + pad.x, tl.y + pad.y + h1 + sep),
                                            g2, text_color,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }

                // ── États ─────────────────────────────────────────────────
                for state in &gemma.states {
                    let is_sel = self.selected_state.as_deref() == Some(&state.id);
                    let is_sim_active = self.sim_active && self.sim_state == state.id;
                    draw_gemma_node(&painter, state, self.offset, self.zoom, origin,
                                    is_sel, is_sim_active);
                }

                // ── Indicateurs de transitions disponibles (mode simulation) ─
                if self.sim_active {
                    for t in &gemma.transitions {
                        if t.from != self.sim_state { continue; }
                        if t.waypoints.is_empty() { continue; }
                        // Dessiner un losange vert clignotant sur la flèche
                        if let Some(dst_state) = gemma.state(&t.to) {
                            let sp = canvas_to_screen(
                                dst_state.pos, self.offset, self.zoom, origin);
                            let eff_h = if dst_state.h > 0.0 { dst_state.h } else { NODE_H };
                            let arrow_tip = Pos2::new(sp.x, sp.y - eff_h * self.zoom / 2.0 - 8.0);
                            let r = 7.0;
                            painter.add(egui::Shape::convex_polygon(
                                vec![
                                    Pos2::new(arrow_tip.x, arrow_tip.y - r),
                                    Pos2::new(arrow_tip.x + r, arrow_tip.y),
                                    Pos2::new(arrow_tip.x, arrow_tip.y + r),
                                    Pos2::new(arrow_tip.x - r, arrow_tip.y),
                                ],
                                Color32::from_rgba_unmultiplied(60, 200, 80, 210),
                                egui::Stroke::new(1.5, Color32::from_rgb(120, 255, 140)),
                            ));
                        }
                    }
                }

                // (Handles de déplacement supprimés — édition de position inhibée)

                // ── Snap périmètre + preview en mode Flèche ──────────────
                if self.tool == GemmaTool::AddTransition {
                    let hover_pos = ctx.pointer_hover_pos();
                    if let Some(hp) = hover_pos {
                        if resp.rect.contains(hp) {
                            // Point d'accroche sur n'importe quel bord de rectangle
                            let snap = snap_perimeter(hp, &gemma.states, SNAP_DIST,
                                self.offset, self.zoom, origin);
                            if let Some((_, snap_sp)) = &snap {
                                draw_snap_dot(&painter, *snap_sp, self.zoom, false);
                            }
                            // Si source en attente : point doré fixe + preview ortho
                            if let Some(ref pf) = self.pending_from {
                                let src_sp = canvas_to_screen(
                                    pf.pos, self.offset, self.zoom, origin);
                                draw_snap_dot(&painter, src_sp, self.zoom, true);
                                // Dest canvas : snap ou curseur libre
                                let dest_cv = if let Some((ref sa, _)) = snap {
                                    sa.pos
                                } else {
                                    let c = to_canvas(hp, origin, self.offset, self.zoom);
                                    [c.x, c.y]
                                };
                                let route = preview_route(pf.pos, pf.side, dest_cv);
                                let stroke = Stroke::new(1.5, Color32::from_rgb(200, 200, 80));
                                let mut pp = canvas_to_screen(route[0], self.offset, self.zoom, origin);
                                for seg in &route[1..] {
                                    let np = canvas_to_screen(*seg, self.offset, self.zoom, origin);
                                    painter.line_segment([pp, np], stroke);
                                    pp = np;
                                }
                            }
                        }
                    }
                }

                // ── Entrées souris ─────────────────────────────────────────
                let just_pressed   = ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
                let right_pressed  = ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary));
                let primary_down   = ctx.input(|i| i.pointer.button_down(egui::PointerButton::Primary));
                let primary_up     = ctx.input(|i| i.pointer.button_released(egui::PointerButton::Primary));
                let ptr            = ctx.pointer_interact_pos();

                // Escape → annuler/sélect
                if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.tool = GemmaTool::Select;
                    self.pending_from = None;
                    self.ctx_menu_trans = None;
                }

                // ── Clic droit → menu contextuel ──────────────────────────
                if right_pressed {
                    if let Some(mp) = ptr {
                        if resp.rect.contains(mp) {
                            let cv = to_canvas(mp, origin, self.offset, self.zoom);
                            if let Some(tid) = hit_trans(&gemma.transitions, &gemma.states, cv) {
                                self.ctx_menu_trans = Some((tid, mp));
                            } else {
                                self.ctx_menu_trans = None;
                            }
                        }
                    }
                }

                // ── Affichage menu contextuel ──────────────────────────────
                let mut close_ctx = false;
                let mut del_trans: Option<u32> = None;
                if let Some((tid, menu_pos)) = self.ctx_menu_trans {
                    egui::Area::new(egui::Id::new("__ctx_trans"))
                        .fixed_pos(menu_pos)
                        .order(egui::Order::Foreground)
                        .show(&ctx, |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                ui.set_min_width(160.0);
                                if ui.button("🗑 Supprimer la flèche").clicked() {
                                    del_trans  = Some(tid);
                                    close_ctx  = true;
                                }
                                ui.separator();
                                if ui.button("↩ Annuler").clicked() {
                                    close_ctx = true;
                                }
                            });
                        });
                    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                        close_ctx = true;
                    }
                }
                if let Some(tid) = del_trans {
                    gemma.transitions.retain(|t| t.id != tid);
                    if self.selected_trans == Some(tid) { self.selected_trans = None; }
                    self.needs_save = true;
                    status_out = Some(format!("Transition #{} supprimée", tid));
                }
                if close_ctx { self.ctx_menu_trans = None; }

                // ── Clic gauche ────────────────────────────────────────────
                if just_pressed && resp.contains_pointer()
                        && self.ctx_menu_trans.is_none() {
                    if let Some(mp) = ptr {
                        let cv = to_canvas(mp, origin, self.offset, self.zoom);

                        // Mode simulation : clic sur un état accessible
                        if self.sim_active {
                            if let Some(sid) = hit_node(cv, &gemma.states) {
                                let reachable = gemma.transitions.iter()
                                    .any(|t| t.from == self.sim_state && t.to == sid);
                                if reachable {
                                    let cond = gemma.transitions.iter()
                                        .find(|t| t.from == self.sim_state && t.to == sid)
                                        .map(|t| t.condition.to_display())
                                        .unwrap_or_default();
                                    let entry = format!("{} →[{}]→ {}", self.sim_state, cond, sid);
                                    self.sim_history.push(entry);
                                    status_out = Some(format!("Transition : {} → {}", self.sim_state, sid));
                                    self.sim_state = sid;
                                }
                            }
                        } else {

                        match self.tool {
                            GemmaTool::Select => {
                                // Sélection uniquement (déplacements inhibés)
                                if let Some(tid) = hit_trans(
                                        &gemma.transitions, &gemma.states, cv) {
                                    if gemma.transitions.iter()
                                            .find(|t| t.id == tid)
                                            .map_or(false, |t| !t.waypoints.is_empty()) {
                                        self.selected_trans = Some(tid);
                                        self.selected_state = None;
                                        self.editing_cond   = None;
                                        self.editing_action = None;
                                    }
                                } else if let Some(sid) = hit_node(cv, &gemma.states) {
                                    // clic sur état : sélectionner l'état
                                    self.selected_trans  = None;
                                    self.editing_cond    = None;
                                    self.editing_action  = None;
                                    self.selected_state  = Some(sid);
                                } else {
                                    self.selected_state = None;
                                    self.selected_trans = None;
                                }
                            }

                            GemmaTool::AddTransition => {
                                // Snap au périmètre le plus proche
                                let anchor = snap_perimeter(
                                    mp, &gemma.states, SNAP_DIST,
                                    self.offset, self.zoom, origin,
                                ).map(|(a, _)| a);
                                if let Some(a) = anchor {
                                    if let Some(ref pf) = self.pending_from.clone() {
                                        // Deuxième clic : finaliser
                                        if a.state_id != pf.state_id {
                                            let from = pf.state_id.clone();
                                            let to   = a.state_id.clone();
                                            let id = gemma.add_transition(
                                                from.clone(), to.clone(), Expr::True);
                                            if let Some(t) = gemma.transitions.iter_mut()
                                                    .find(|t| t.id == id) {
                                                // Route mémorisée → priorité absolue
                                                let saved = crate::gemma::load_saved_routes();
                                                let key = (from.clone(), to.clone());
                                                if let Some((saved_wpts, saved_cond)) = saved.get(&key) {
                                                    if !saved_wpts.is_empty() {
                                                        t.waypoints = saved_wpts.clone();
                                                    } else {
                                                        t.waypoints = orthogonal_route(
                                                            pf.pos, pf.side, a.pos, a.side);
                                                    }
                                                    if !saved_cond.is_empty() {
                                                        t.condition = Expr::from_str(saved_cond);
                                                    }
                                                } else {
                                                    t.waypoints = orthogonal_route(
                                                        pf.pos, pf.side, a.pos, a.side);
                                                }
                                            }
                                            self.pending_from = None;
                                            self.needs_save = true;
                                            status_out = Some(format!(
                                                "Flèche {} → {} créée (#{id})", from, to));
                                        } else {
                                            // Même état : on change juste la source
                                            self.pending_from = Some(a);
                                        }
                                    } else {
                                        // Premier clic : mémoriser ancre source
                                        self.pending_from = Some(a);
                                    }
                                } else {
                                    // Clic dans le vide → annuler
                                    self.pending_from = None;
                                }
                            }

                            GemmaTool::Delete => {
                                if let Some(sid) = hit_node(cv, &gemma.states) {
                                    gemma.states.retain(|s| s.id != sid);
                                    gemma.transitions.retain(
                                        |t| t.from != sid && t.to != sid);
                                    if self.selected_state.as_deref() == Some(&sid) {
                                        self.selected_state = None;
                                    }
                                    self.needs_save = true;
                                    status_out = Some(format!("État {} supprimé", sid));
                                } else if let Some(tid) = hit_trans(
                                        &gemma.transitions, &gemma.states, cv) {
                                    gemma.transitions.retain(|t| t.id != tid);
                                    if self.selected_trans == Some(tid) {
                                        self.selected_trans = None;
                                    }
                                    self.needs_save = true;
                                    status_out = Some(format!("Transition #{tid} supprimée"));
                                }
                            }
                            _ => {}
                        }
                        } // fin else !sim_active
                    }
                }
                // Fermer ctx_menu aussi sur clic gauche n'importe où (sauf boutons du menu)
                if just_pressed && self.ctx_menu_trans.is_some() && del_trans.is_none() {
                    self.ctx_menu_trans = None;
                }

                // Pan (bouton milieu)
                if resp.dragged_by(egui::PointerButton::Middle) {
                    self.offset += resp.drag_delta();
                }

                // ── Handles de déplacement de segments (transition sélectionnée) ──
                // Un handle carré est placé au milieu de chaque segment intermédiaire.
                // ui.interact(Sense::drag) gère le hit-test et le drag nativement.
                // Convention écran : canvas_to_screen(pos, offset, zoom, origin)
                //                  = pos * zoom + offset + origin
                // ui.interact prend des coordonnées écran absolues → même convention.
                if self.tool == GemmaTool::Select {
                    if let Some(tid) = self.selected_trans {
                        // Normaliser pour garantir des segments strictement H/V
                        let wpts: Vec<[f32; 2]> = gemma.transitions.iter()
                            .find(|t| t.id == tid)
                            .map(|t| normalize_ortho_route(t.waypoints.clone()))
                            .unwrap_or_default();

                        const HS: f32 = 16.0;  // taille zone cliquable px
                        const HR: f32 = 5.0;   // demi-côté du carré visuel

                        for i in 0..wpts.len().saturating_sub(1) {
                            let a = wpts[i];
                            let b = wpts[i + 1];
                            let is_horiz = (a[1] - b[1]).abs() < 1.0;
                            let is_vert  = (a[0] - b[0]).abs() < 1.0;
                            if !is_horiz && !is_vert { continue; }

                            // Milieu du segment en canvas
                            let mid_cv = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
                            let mid_sp = canvas_to_screen(mid_cv, self.offset, self.zoom, origin);

                            let handle_rect = egui::Rect::from_center_size(
                                mid_sp, egui::Vec2::splat(HS),
                            );
                            let seg_resp = ui.interact(
                                handle_rect,
                                egui::Id::new("gemma_seg").with(tid).with(i),
                                egui::Sense::drag(),
                            );

                            if seg_resp.dragged() {
                                let delta_cv = seg_resp.drag_delta() / self.zoom;
                                if let Some(t) = gemma.transitions.iter_mut().find(|t| t.id == tid) {
                                    // Normaliser d'abord pour partir d'une base propre
                                    t.waypoints = normalize_ortho_route(
                                        std::mem::take(&mut t.waypoints));
                                    if i + 1 < t.waypoints.len() {
                                        if is_horiz {
                                            // Segment horizontal → déplacement Y
                                            t.waypoints[i][1]     += delta_cv.y;
                                            t.waypoints[i + 1][1] += delta_cv.y;
                                        } else {
                                            // Segment vertical → déplacement X
                                            t.waypoints[i][0]     += delta_cv.x;
                                            t.waypoints[i + 1][0] += delta_cv.x;
                                        }
                                        // Re-normaliser pour conserver la propriété
                                        t.waypoints = normalize_ortho_route(
                                            std::mem::take(&mut t.waypoints));
                                    }
                                    self.needs_save = true;
                                }
                            }

                            let col = if seg_resp.dragged() || seg_resp.is_pointer_button_down_on() {
                                Color32::from_rgb(255, 240, 80)
                            } else if seg_resp.hovered() {
                                Color32::from_rgb(255, 200, 60)
                            } else {
                                Color32::from_rgb(180, 140, 50)
                            };
                            if seg_resp.hovered() {
                                ctx.set_cursor_icon(if is_horiz {
                                    egui::CursorIcon::ResizeVertical
                                } else {
                                    egui::CursorIcon::ResizeHorizontal
                                });
                            }
                            painter.rect_filled(
                                egui::Rect::from_center_size(mid_sp, egui::Vec2::splat(HR * 2.0)),
                                egui::CornerRadius::ZERO,
                                col,
                            );
                            painter.rect_stroke(
                                egui::Rect::from_center_size(mid_sp, egui::Vec2::splat(HR * 2.0)),
                                egui::CornerRadius::ZERO,
                                egui::Stroke::new(1.0, Color32::WHITE),
                                egui::StrokeKind::Middle,
                            );
                        }
                    }
                }
                // ── Fin handles ────────────────────────────────────────────

                // Zoom molette
                let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
                if resp.contains_pointer() && scroll != 0.0 {
                    let f = if scroll > 0.0 { 1.1_f32 } else { 1.0 / 1.1 };
                    self.zoom = (self.zoom * f).clamp(0.2, 4.0);
                }
            });

        status_out
    }

    fn draw_simulation(&mut self, ui: &mut egui::Ui, gemma: &mut Gemma) -> Option<String> {
        let mut status_out: Option<String> = None;

        ui.label(
            egui::RichText::new("▶ Simulation GEMMA")
                .size(13.0).strong()
                .color(Color32::from_rgb(255, 220, 60))
        );
        ui.separator();
        ui.add_space(4.0);

        // État actif
        let state_info = gemma.state(&self.sim_state)
            .map(|s| (s.description.clone(), s.action.clone(), s.state_type));
        if let Some((desc, action, stype)) = &state_info {
            ui.label(
                egui::RichText::new("État actif :").size(11.0)
                    .color(Color32::from_rgb(140, 160, 180))
            );
            ui.label(
                egui::RichText::new(format!("⬤  {}", self.sim_state))
                    .size(14.0).strong().color(stype.color())
            );
            if !desc.is_empty() {
                ui.label(
                    egui::RichText::new(desc).size(11.0)
                        .color(Color32::from_rgb(160, 200, 240))
                );
            }
            if !action.is_empty() {
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(format!("⚙ {action}"))
                        .size(11.0).color(Color32::from_rgb(80, 220, 100))
                );
            }
        }

        // Transitions disponibles
        let avail: Vec<(u32, String, String)> = gemma.transitions.iter()
            .filter(|t| t.from == self.sim_state)
            .map(|t| (t.id, t.to.clone(), t.condition.to_display()))
            .collect();

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        if avail.is_empty() {
            ui.label(
                egui::RichText::new("Aucune transition disponible\n(état terminal)")
                    .size(11.0).italics().color(Color32::from_rgb(200, 100, 80))
            );
        } else {
            ui.label(
                egui::RichText::new("Transitions disponibles :")
                    .size(11.0).color(Color32::from_rgb(140, 160, 180))
            );
            ui.add_space(4.0);

            let mut fire: Option<(String, String)> = None;
            for (_, to_id, cond) in &avail {
                let has_cond = cond != "TRUE" && cond != "FALSE";
                ui.add_space(4.0);

                // Ligne destination
                ui.label(
                    egui::RichText::new(format!("→  {to_id}"))
                        .size(11.0).strong()
                        .color(Color32::from_rgb(180, 210, 240))
                );

                // Bouton condition (cliquable)
                let btn_text = if has_cond {
                    format!("[ {cond} ]")
                } else {
                    "[ condition libre ]".to_string()
                };
                let cond_color = if has_cond {
                    Color32::from_rgb(220, 170, 255)
                } else {
                    Color32::from_rgb(180, 220, 180)
                };
                if ui.add(
                    egui::Button::new(
                        egui::RichText::new(&btn_text).size(12.0).strong().color(cond_color)
                    )
                    .fill(Color32::from_rgb(35, 30, 55))
                    .min_size(Vec2::new(ui.available_width(), 0.0))
                )
                .on_hover_text(format!("Activer : {} → {}", self.sim_state, to_id))
                .clicked() {
                    fire = Some((to_id.clone(), cond.clone()));
                }
            }

            if let Some((to_id, cond)) = fire {
                let entry = format!("{} →[{}]→ {}", self.sim_state, cond, to_id);
                self.sim_history.push(entry);
                status_out = Some(format!("Transition : {} → {}", self.sim_state, to_id));
                self.sim_state = to_id;
            }
        }

        // Log
        if !self.sim_history.is_empty() {
            ui.add_space(8.0);
            ui.separator();
            ui.label(
                egui::RichText::new("Historique :").size(11.0)
                    .color(Color32::from_rgb(140, 160, 180))
            );
            egui::ScrollArea::vertical()
                .max_height(120.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for entry in &self.sim_history {
                        ui.label(
                            egui::RichText::new(entry).size(10.0)
                                .color(Color32::from_rgb(150, 170, 190))
                        );
                    }
                });
            if ui.add(egui::Button::new(
                    egui::RichText::new("🗑 Effacer historique").size(10.0))
                .fill(Color32::from_rgb(40, 30, 30))).clicked() {
                self.sim_history.clear();
            }
        }

        status_out
    }

    fn draw_props(&mut self, ui: &mut egui::Ui, gemma: &mut Gemma) {
        if let Some(tid) = self.selected_trans {
            // ── Transition sélectionnée ─────────────────────────────────
            let t_data = gemma.transitions.iter()
                .find(|t| t.id == tid)
                .map(|t| (t.from.clone(), t.to.clone(), t.condition.clone()));
            if let Some((from, to, cond)) = t_data {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!("Transition #{tid}"))
                        .strong().size(13.0).color(Color32::from_rgb(241, 196, 15))
                );
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(format!("{from}  →  {to}"))
                        .size(12.0).color(Color32::from_rgb(180, 210, 240))
                );
                ui.separator();
                ui.add_space(4.0);

                let cond_str_display = cond.to_display();
                let has_cond = cond_str_display != "FALSE" && cond_str_display != "TRUE";

                if has_cond {
                    ui.label(
                        egui::RichText::new("Condition actuelle :")
                            .size(11.0).color(Color32::from_rgb(140, 160, 180))
                    );
                    ui.label(
                        egui::RichText::new(&cond_str_display)
                            .size(12.0).strong().color(Color32::from_rgb(180, 140, 255))
                    );
                } else {
                    ui.label(
                        egui::RichText::new("⚠ Aucune condition")
                            .size(11.0).color(Color32::from_rgb(220, 80, 80))
                    );
                }
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Nouvelle condition :")
                        .size(11.0).color(Color32::from_rgb(140, 160, 180))
                );
                let ec = self.editing_cond.get_or_insert((tid, cond_str_display.clone()));
                ui.text_edit_singleline(&mut ec.1);
                ui.add_space(4.0);
                if ui.add(egui::Button::new("✔ Appliquer")
                        .fill(Color32::from_rgb(39, 100, 58))).clicked() {
                    if let Some((_, s)) = &self.editing_cond {
                        let new_cond = Expr::from_str(s);
                        if let Some(t) = gemma.transitions.iter_mut().find(|t| t.id == tid) {
                            t.condition = new_cond;
                        }
                    }
                    self.editing_cond = None;
                    self.needs_save = true;
                }
                ui.add_space(8.0);
                ui.separator();
                if ui.add(egui::Button::new("🗑 Supprimer")
                        .fill(Color32::from_rgb(100, 30, 30))).clicked() {
                    gemma.transitions.retain(|t| t.id != tid);
                    self.selected_trans = None;
                    self.editing_cond   = None;
                    self.needs_save = true;
                }
            }
        } else if let Some(sid) = self.selected_state.clone() {
            // ── État sélectionné ─────────────────────────────────────────
            let state_data = gemma.state(&sid)
                .map(|s| (s.description.clone(), s.action.clone(), s.state_type));
            if let Some((desc, action, stype)) = state_data {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!("État  {sid}"))
                        .strong().size(13.0).color(stype.color())
                );
                if !desc.is_empty() {
                    ui.label(
                        egui::RichText::new(&desc)
                            .size(11.0).color(Color32::from_rgb(160, 200, 240))
                    );
                }
                ui.separator();
                ui.add_space(4.0);
                if !action.is_empty() {
                    ui.label(
                        egui::RichText::new("Action actuelle :")
                            .size(11.0).color(Color32::from_rgb(140, 160, 180))
                    );
                    ui.label(
                        egui::RichText::new(&action)
                            .size(12.0).strong().color(Color32::from_rgb(80, 220, 100))
                    );
                } else {
                    ui.label(
                        egui::RichText::new("Aucune action définie")
                            .size(11.0).italics().color(Color32::from_rgb(120, 130, 140))
                    );
                }
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Nouvelle action :")
                        .size(11.0).color(Color32::from_rgb(140, 160, 180))
                );
                let ea = self.editing_action.get_or_insert(action.clone());
                ui.add(egui::TextEdit::singleline(ea)
                    .hint_text("Ex : Dosage et malaxage")
                    .desired_width(f32::INFINITY));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new("✔ Appliquer")
                            .fill(Color32::from_rgb(39, 100, 58))).clicked() {
                        if let Some(s) = self.editing_action.take() {
                            if let Some(st) = gemma.state_mut(&sid) { st.action = s; }
                        }
                        self.needs_save = true;
                    }
                    if ui.add(egui::Button::new("✕ Effacer")
                            .fill(Color32::from_rgb(80, 30, 30))).clicked() {
                        if let Some(st) = gemma.state_mut(&sid) { st.action.clear(); }
                        self.editing_action = None;
                        self.needs_save = true;
                    }
                });

                // ── Transitions sortantes ─────────────────────────────────
                let out_trans: Vec<(u32, String, String)> = gemma.transitions.iter()
                    .filter(|t| t.from == sid)
                    .map(|t| (t.id, t.to.clone(), t.condition.to_display()))
                    .collect();

                if !out_trans.is_empty() {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Transitions sortantes :")
                            .size(11.0).color(Color32::from_rgb(140, 160, 180))
                    );
                    ui.add_space(2.0);

                    let mut apply_tid:  Option<u32>          = None;
                    let mut cancel_edit                       = false;
                    let mut start_edit: Option<(u32, String)> = None;

                    for (tid, to_id, cond_str) in &out_trans {
                        let is_editing = self.editing_cond
                            .as_ref().map(|(id, _)| *id == *tid).unwrap_or(false);

                        ui.add_space(3.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("→ {to_id}"))
                                    .size(11.0).strong()
                                    .color(Color32::from_rgb(180, 210, 240))
                            );
                            if !is_editing {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.add(egui::Button::new(
                                                egui::RichText::new("✏").size(11.0)))
                                            .on_hover_text("Éditer la condition")
                                            .clicked()
                                        {
                                            start_edit = Some((*tid, cond_str.clone()));
                                        }
                                    }
                                );
                            }
                        });

                        if is_editing {
                            if let Some((_, ref mut s)) = self.editing_cond {
                                ui.add(egui::TextEdit::singleline(s)
                                    .hint_text("Condition (ex : Bp_f)")
                                    .desired_width(f32::INFINITY));
                            }
                            ui.horizontal(|ui| {
                                if ui.add(egui::Button::new("✔ Appliquer")
                                        .fill(Color32::from_rgb(39, 100, 58))).clicked() {
                                    apply_tid = Some(*tid);
                                }
                                if ui.add(egui::Button::new("✕").small()).clicked() {
                                    cancel_edit = true;
                                }
                            });
                        } else {
                            let display = if cond_str == "FALSE"
                                || cond_str == "TRUE"
                                || cond_str.is_empty()
                            {
                                egui::RichText::new("⚠ aucune condition")
                                    .size(10.0).italics()
                                    .color(Color32::from_rgb(200, 80, 80))
                            } else {
                                egui::RichText::new(cond_str)
                                    .size(10.0)
                                    .color(Color32::from_rgb(180, 140, 255))
                            };
                            ui.label(display);
                        }
                    }

                    // Appliquer les mutations après la boucle
                    if let Some(tid) = apply_tid {
                        if let Some((_, s)) = &self.editing_cond {
                            let new_cond = Expr::from_str(s);
                            if let Some(t) = gemma.transitions.iter_mut()
                                    .find(|t| t.id == tid) {
                                t.condition = new_cond;
                            }
                        }
                        self.editing_cond = None;
                        self.needs_save = true;
                    } else if cancel_edit {
                        self.editing_cond = None;
                    } else if let Some((tid, s)) = start_edit {
                        self.editing_cond = Some((tid, s));
                    }
                }
            }
        } else {
            ui.label(egui::RichText::new(
                "Cliquez sur un état\npour éditer son action\net ses transitions.")
                .size(11.0).italics().color(Color32::from_rgb(100, 120, 140)));
            ui.add_space(12.0);
            ui.separator();
            let total = gemma.transitions.len();
            let with_cond = gemma.transitions.iter()
                .filter(|t| !t.waypoints.is_empty())
                .filter(|t| {
                    let d = t.condition.to_display();
                    d != "FALSE" && d != "TRUE"
                }).count();
            let visible = gemma.transitions.iter()
                .filter(|t| !t.waypoints.is_empty()).count();
            ui.label(format!("États : {}", gemma.states.len()));
            ui.label(format!("Flèches visibles : {visible} / {total}"));
            if with_cond < visible {
                ui.label(
                    egui::RichText::new(format!("⚠ {} sans condition", visible - with_cond))
                        .size(11.0).color(Color32::from_rgb(220, 80, 80))
                );
            }
        }
    }


        fn draw_questionnaire(&mut self, ui: &mut egui::Ui, gemma: &mut Gemma) -> Option<String> {
            let mut status_out: Option<String> = None;
    
            ui.heading("📋 Questionnaire GEMMA");
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Répondez à chaque question pour générer automatiquement\nles états et transitions du GEMMA.")
                    .size(11.0)
                    .color(Color32::from_rgb(150, 170, 190)),
            );
            ui.add_space(8.0);
    
            let answered = self.questionnaire.answered_count();
            let total = self.questionnaire.questions.len();
            let progress = answered as f32 / total as f32;
            ui.add(egui::ProgressBar::new(progress)
                .text(format!("{}/{} répondues", answered, total)));
            ui.add_space(6.0);
    
            // Sélection rapide globale
            ui.horizontal(|ui| {
                if ui.button("✔ Tout OUI").clicked() {
                    for q in &mut self.questionnaire.questions {
                        q.answer = Answer::Yes;
                    }
                }
                if ui.button("✘ Tout NON").clicked() {
                    for q in &mut self.questionnaire.questions {
                        q.answer = Answer::No;
                    }
                }
                if ui.button("? Tout N/A").clicked() {
                    for q in &mut self.questionnaire.questions {
                        q.answer = Answer::Unanswered;
                    }
                }
            });
            ui.add_space(6.0);
    
            // Zone scrollable avec les questions
            egui::ScrollArea::vertical()
                .id_salt("questionnaire_scroll")
                .max_height(ui.available_height() - 60.0)
                .show(ui, |ui| {
                    for q in &mut self.questionnaire.questions {
                        let frame_color = match q.answer {
                            Answer::Yes => Color32::from_rgb(30, 60, 40),
                            Answer::No  => Color32::from_rgb(60, 30, 30),
                            Answer::Unanswered => Color32::from_rgb(25, 35, 50),
                        };
                        egui::Frame::new()
                            .fill(frame_color)
                            .corner_radius(4.0)
                            .inner_margin(egui::Margin::same(8))
                            .outer_margin(egui::Margin::symmetric(0, 3))
                            .show(ui, |ui| {
                                // Titre + numéro
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(format!("Q{}. {}", q.id, q.titre))
                                            .size(12.0)
                                            .strong()
                                            .color(Color32::from_rgb(200, 220, 240)),
                                    );
                                });
                                ui.add_space(2.0);
                                ui.label(
                                    egui::RichText::new(&q.question)
                                        .size(11.0)
                                        .color(Color32::from_rgb(210, 215, 220)),
                                );
                                ui.add_space(4.0);
                                // Boutons Oui / Non / ?
                                ui.horizontal(|ui| {
                                    let oui_active = q.answer == Answer::Yes;
                                    let non_active = q.answer == Answer::No;
                                    let na_active  = q.answer == Answer::Unanswered;
    
                                    let oui_btn = egui::Button::new(
                                        egui::RichText::new("✔ Oui").size(11.0)
                                    ).fill(if oui_active { Color32::from_rgb(39, 174, 96) } else { Color32::from_rgb(40, 60, 40) });
                                    if ui.add(oui_btn).clicked() { q.answer = Answer::Yes; }
    
                                    let non_btn = egui::Button::new(
                                        egui::RichText::new("✘ Non").size(11.0)
                                    ).fill(if non_active { Color32::from_rgb(192, 57, 43) } else { Color32::from_rgb(60, 40, 40) });
                                    if ui.add(non_btn).clicked() { q.answer = Answer::No; }
    
                                    let na_btn = egui::Button::new(
                                        egui::RichText::new("? N/A").size(11.0)
                                    ).fill(if na_active { Color32::from_rgb(100, 100, 100) } else { Color32::from_rgb(45, 45, 50) });
                                    if ui.add(na_btn).clicked() { q.answer = Answer::Unanswered; }
    
                                    // Aperçu des états activés
                                    if !q.conditions.is_empty() {
                                        ui.add_space(8.0);
                                        ui.label(
                                            egui::RichText::new(format!("[{}]", q.conditions.join(", ")))
                                                .size(9.0)
                                                .color(Color32::from_rgb(100, 130, 160)),
                                        );
                                    }
                                });
                            });
                    }
                });
    
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                // Bouton Appliquer
                let apply_btn = egui::Button::new(
                    egui::RichText::new("⚙ Appliquer au GEMMA").size(12.0)
                ).fill(Color32::from_rgb(41, 128, 185));
                if ui.add(apply_btn).clicked() {
                    let saved_routes = crate::gemma::load_saved_routes();
                    let before = gemma.states.len();
                    self.questionnaire.apply_to_gemma(gemma, &saved_routes);
                    let added = gemma.states.len() - before;
                    self.pending_fit = true;
                    self.needs_save = true;
                    status_out = Some(format!(
                        "GEMMA généré : {} états ajoutés ({} total)",
                        added, gemma.states.len()
                    ));
                }
    
                // Bouton Réinitialiser (avec confirmation)
                let reset_btn = egui::Button::new(
                    egui::RichText::new("↺ Réinitialiser").size(11.0)
                ).fill(Color32::from_rgb(60, 55, 45));
                if ui.add(reset_btn).clicked() {
                    self.confirm_reset = true;
                }

                // Dialogue de confirmation
                if self.confirm_reset {
                    egui::Window::new("⚠ Confirmation")
                        .collapsible(false)
                        .resizable(false)
                        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                        .show(ui.ctx(), |ui| {
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new("Effacer tous les états et transitions du GEMMA ?")
                                    .size(12.0)
                                    .color(Color32::from_rgb(220, 180, 80)),
                            );
                            ui.label(
                                egui::RichText::new("Cette action est irréversible.")
                                    .size(11.0)
                                    .color(Color32::from_rgb(180, 100, 100)),
                            );
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                let oui = egui::Button::new(
                                    egui::RichText::new("Oui, effacer").size(11.0)
                                ).fill(Color32::from_rgb(160, 40, 40));
                                if ui.add(oui).clicked() {
                                    self.questionnaire.reset_answers();
                                    gemma.states.clear();
                                    gemma.transitions.clear();
                                    gemma.next_trans_id = 0;
                                    // Pas d'auto-save : l'utilisateur doit sauvegarder
                                    // explicitement (Ctrl+S) après un reset destructif.
                                    self.confirm_reset = false;
                                    status_out = Some("GEMMA réinitialisé — sauvegardez avec Ctrl+S si souhaité".to_string());
                                }
                                let non = egui::Button::new(
                                    egui::RichText::new("Annuler").size(11.0)
                                ).fill(Color32::from_rgb(45, 55, 65));
                                if ui.add(non).clicked() {
                                    self.confirm_reset = false;
                                }
                            });
                        });
                }
            });
    
            status_out
        }
}

// ── Helpers canvas ────────────────────────────────────────────────────────────

/// Canvas → écran.
fn canvas_to_screen(pos: [f32; 2], offset: Vec2, zoom: f32, origin: Pos2) -> Pos2 {
    Pos2::new(pos[0] * zoom + offset.x + origin.x,
              pos[1] * zoom + offset.y + origin.y)
}

/// Écran → canvas.
fn to_canvas(mp: Pos2, origin: Pos2, offset: Vec2, zoom: f32) -> Pos2 {
    Pos2::new((mp.x - origin.x - offset.x) / zoom,
              (mp.y - origin.y - offset.y) / zoom)
}

/// Hit-test d'un état (rectangle).
fn hit_node(cv: Pos2, states: &[GemmaState]) -> Option<String> {
    for s in states {
        let hw = if s.w > 0.0 { s.w } else { NODE_W } / 2.0;
        let hh = if s.h > 0.0 { s.h } else { NODE_H } / 2.0;
        if cv.x >= s.pos[0] - hw && cv.x <= s.pos[0] + hw
            && cv.y >= s.pos[1] - hh && cv.y <= s.pos[1] + hh
        {
            return Some(s.id.clone());
        }
    }
    None
}

// ── Ancres périmètre ──────────────────────────────────────────────────────────

/// Trouve le point le plus proche sur le périmètre de n'importe quel état,
/// dans un rayon `max_px` (pixels écran).
/// Retourne l'ancre canvas et sa position écran.
fn snap_perimeter(
    mp:     Pos2,
    states: &[GemmaState],
    max_px: f32,
    offset: Vec2, zoom: f32, origin: Pos2,
) -> Option<(Anchor, Pos2)> {
    let mut best: Option<(f32, Anchor, Pos2)> = None;
    for st in states {
        let hw = if st.w > 0.0 { st.w } else { NODE_W } / 2.0;
        let hh = if st.h > 0.0 { st.h } else { NODE_H } / 2.0;
        // Les 4 arêtes avec leur côté
        let edges: [([f32;2], [f32;2], Side); 4] = [
            ([st.pos[0]-hw, st.pos[1]-hh], [st.pos[0]+hw, st.pos[1]-hh], Side::N),
            ([st.pos[0]-hw, st.pos[1]+hh], [st.pos[0]+hw, st.pos[1]+hh], Side::S),
            ([st.pos[0]-hw, st.pos[1]-hh], [st.pos[0]-hw, st.pos[1]+hh], Side::W),
            ([st.pos[0]+hw, st.pos[1]-hh], [st.pos[0]+hw, st.pos[1]+hh], Side::E),
        ];
        for (ca, cb, side) in edges {
            let sa = canvas_to_screen(ca, offset, zoom, origin);
            let sb = canvas_to_screen(cb, offset, zoom, origin);
            let dx = sb.x - sa.x;
            let dy = sb.y - sa.y;
            let len2 = dx*dx + dy*dy;
            let t = if len2 < 0.001 { 0.0 } else {
                ((mp.x - sa.x)*dx + (mp.y - sa.y)*dy) / len2
            };
            let t = t.clamp(0.0, 1.0);
            let nearest_screen = Pos2::new(sa.x + t*dx, sa.y + t*dy);
            let d = (mp - nearest_screen).length();
            if d <= max_px {
                if best.as_ref().map_or(true, |(bd, _, _)| d < *bd) {
                    let cp = to_canvas(nearest_screen, origin, offset, zoom);
                    best = Some((d, Anchor {
                        state_id: st.id.clone(),
                        side,
                        pos: [cp.x, cp.y],
                    }, nearest_screen));
                }
            }
        }
    }
    best.map(|(_, a, sp)| (a, sp))
}

// ── Normalisation de route orthogonale ────────────────────────────────────────

/// Nettoie une liste de waypoints pour garantir des segments strictement
/// horizontaux ou verticaux :
/// 1. Snape chaque point de façon à ce que le segment avec le précédent
///    soit exactement H ou V (en propageant depuis le premier point).
/// 2. Supprime les points dégénérés (distance < 0.5 px canvas du précédent).
/// 3. Fusionne les segments colinéaires consécutifs (supprime les points
///    intermédiaires sur la même droite).
fn normalize_ortho_route(pts: Vec<[f32; 2]>) -> Vec<[f32; 2]> {
    if pts.len() < 2 { return pts; }

    // Passe 1 : snap vers H ou V en partant du premier point
    let mut s = pts;
    for i in 0..s.len() - 1 {
        let dx = (s[i + 1][0] - s[i][0]).abs();
        let dy = (s[i + 1][1] - s[i][1]).abs();
        if dy <= dx { s[i + 1][1] = s[i][1]; }  // H : même Y
        else        { s[i + 1][0] = s[i][0]; }  // V : même X
    }

    // Passe 2 : supprimer les doublons / segments quasi-nuls (< 0.5 px)
    let mut d: Vec<[f32; 2]> = Vec::with_capacity(s.len());
    for p in &s {
        match d.last() {
            Some(&last)
                if (p[0] - last[0]).abs() < 0.5
                && (p[1] - last[1]).abs() < 0.5 => {}
            _ => d.push(*p),
        }
    }

    // Passe 3 : re-snap après dédoublonnage
    for i in 0..d.len().saturating_sub(1) {
        let dx = (d[i + 1][0] - d[i][0]).abs();
        let dy = (d[i + 1][1] - d[i][1]).abs();
        if dy <= dx { d[i + 1][1] = d[i][1]; }
        else        { d[i + 1][0] = d[i][0]; }
    }

    // Passe 4 : fusionner les segments colinéaires
    if d.len() < 3 { return d; }
    let mut r: Vec<[f32; 2]> = Vec::with_capacity(d.len());
    r.push(d[0]);
    for i in 1..d.len() - 1 {
        let prev = *r.last().unwrap();
        let curr = d[i];
        let next = d[i + 1];
        let h_prev = (curr[1] - prev[1]).abs() < 0.1;
        let h_next = (next[1] - curr[1]).abs() < 0.1;
        let v_prev = (curr[0] - prev[0]).abs() < 0.1;
        let v_next = (next[0] - curr[0]).abs() < 0.1;
        // Point intermédiaire colinéaire = inutile, on le saute
        if (h_prev && h_next) || (v_prev && v_next) { continue; }
        r.push(curr);
    }
    r.push(*d.last().unwrap());
    r
}

// ── Routage orthogonal ─────────────────────────────────────────────────────────

/// Calcule un chemin orthogonal depuis l'ancre (p1, s1) jusqu'à (p2, s2).
///
/// Le chemin part de p1 dans la direction s1 et arrive sur p2
/// depuis la direction s2 (en entrant dans le côté signifié par s2).
fn orthogonal_route(p1: [f32; 2], s1: Side, p2: [f32; 2], s2: Side) -> Vec<[f32; 2]> {
    const MARGIN: f32 = 14.0; // distance avant le premier coude

    // ── Cas "faces opposées" : N↔S ou E↔W ───────────────────────────────────
    // Les deux sorties pointent l'une vers l'autre → S-shape simple ou droite.
    let facing = matches!((s1, s2),
        (Side::N, Side::S) | (Side::S, Side::N) |
        (Side::E, Side::W) | (Side::W, Side::E));
    if facing {
        if !is_horiz(s1) {
            // Vertical : même X → droite, sinon S-shape par le milieu Y
            if (p1[0] - p2[0]).abs() < 0.5 {
                return vec![p1, p2];
            }
            let mid_y = (p1[1] + p2[1]) / 2.0;
            return vec![p1, [p1[0], mid_y], [p2[0], mid_y], p2];
        } else {
            // Horizontal : même Y → droite, sinon S-shape par le milieu X
            if (p1[1] - p2[1]).abs() < 0.5 {
                return vec![p1, p2];
            }
            let mid_x = (p1[0] + p2[0]) / 2.0;
            return vec![p1, [mid_x, p1[1]], [mid_x, p2[1]], p2];
        }
    }

    // ── Cas général (même direction ou axes croisés) ──────────────────────────
    let q1 = extend_from(p1, s1, MARGIN);
    let q2 = extend_from(p2, s2, MARGIN);

    let h1 = is_horiz(s1);
    let h2 = is_horiz(s2);

    let mut pts = vec![p1, q1];
    if h1 == h2 {
        // Même axe, même sens → Z-shape (milieu perpendiculaire)
        if h1 {
            let mid_y = (q1[1] + q2[1]) / 2.0;
            pts.push([q1[0], mid_y]);
            pts.push([q2[0], mid_y]);
        } else {
            let mid_x = (q1[0] + q2[0]) / 2.0;
            pts.push([mid_x, q1[1]]);
            pts.push([mid_x, q2[1]]);
        }
    } else {
        // Axes perpendiculaires → L-shape
        if h1 {
            pts.push([q2[0], q1[1]]); // coude
        } else {
            pts.push([q1[0], q2[1]]); // coude
        }
    }
    pts.push(q2);
    pts.push(p2);
    pts
}

fn is_horiz(s: Side) -> bool { matches!(s, Side::E | Side::W) }

fn extend_from(p: [f32; 2], side: Side, dist: f32) -> [f32; 2] {
    match side {
        Side::N => [p[0], p[1] - dist],
        Side::S => [p[0], p[1] + dist],
        Side::E => [p[0] + dist, p[1]],
        Side::W => [p[0] - dist, p[1]],
    }
}

/// Détecte le côté d'un état sur lequel se trouve (ou est le plus proche de) `pos`.
fn detect_side(pos: [f32; 2], state: &GemmaState) -> Side {
    let hw = if state.w > 0.0 { state.w } else { NODE_W } / 2.0;
    let hh = if state.h > 0.0 { state.h } else { NODE_H } / 2.0;
    let dn = (pos[1] - (state.pos[1] - hh)).abs();
    let ds = (pos[1] - (state.pos[1] + hh)).abs();
    let dw = (pos[0] - (state.pos[0] - hw)).abs();
    let de = (pos[0] - (state.pos[0] + hw)).abs();
    if dn <= ds && dn <= dw && dn <= de { Side::N }
    else if ds <= dw && ds <= de        { Side::S }
    else if dw <= de                    { Side::W }
    else                                { Side::E }
}

/// Chemin L-shape simplifié pour la prévisualisation (sans marges).
/// Part de p1 dans la direction de s1, arrive à p2 avec un seul coude.
fn preview_route(p1: [f32; 2], s1: Side, p2: [f32; 2]) -> Vec<[f32; 2]> {
    if is_horiz(s1) {
        vec![p1, [p2[0], p1[1]], p2]  // horizontal puis vertical
    } else {
        vec![p1, [p1[0], p2[1]], p2]  // vertical puis horizontal
    }
}

// ── Dessin point d'accroche ───────────────────────────────────────────────────

/// Dessine un seul point d'accroche sur le périmètre.
/// `source` = true → doré (ancre de départ), false → cyan (destination).
fn draw_snap_dot(painter: &Painter, sp: Pos2, zoom: f32, source: bool) {
    let r = (ANCHOR_R * zoom).max(4.0);
    let fill = if source {
        Color32::from_rgb(255, 190, 0)
    } else {
        Color32::from_rgba_unmultiplied(60, 200, 255, 220)
    };
    painter.circle_filled(sp, r, fill);
    painter.circle_stroke(sp, r, Stroke::new(1.5, Color32::WHITE));
}

/// Handle carré sur le milieu d'un segment intermédiaire déplaçable.
/// Retourne la plage d'indices i tels que le segment i→i+1 est librement déplaçable.
/// - n=2,3 : droite/demi-coude → aucun handle
/// - n=4   : S-shape faces opposées → handle sur segment 1 (le milieu)
/// - n=5   : L-shape 90° → aucun handle
/// - n=6   : Z-shape → handle sur segment 2 (le milieu entre les deux stubs)
/// - n≥7   : multi-segment → handles sur 2..n-4
fn seg_handle_range(n: usize) -> std::ops::Range<usize> {
    match n {
        4      => 1..2,
        n if n >= 6 => 2..(n.saturating_sub(3)),
        _      => 0..0,
    }
}

fn draw_seg_handle(painter: &Painter, sp: Pos2, zoom: f32) {
    let half = (3.5 * zoom).max(4.0);
    let rect = egui::Rect::from_center_size(sp, egui::Vec2::splat(half * 2.0));
    painter.rect_filled(rect, 1.0, Color32::from_rgba_unmultiplied(180, 220, 255, 210));
    painter.rect_stroke(rect, 1.0, Stroke::new(1.5, Color32::WHITE), egui::StrokeKind::Inside);
}

// ── Dessin flèches ─────────────────────────────────────────────────────────────

/// Résout les waypoints d'une transition.
/// Priorité : waypoints stockés > static_gemma_route > fallback L.
fn resolve_waypoints(t: &GemmaTransition, states: &[GemmaState]) -> Vec<[f32; 2]> {
    if !t.waypoints.is_empty() {
        return t.waypoints.clone();
    }
    // Fallback : table statique centralisée dans gemma/mod.rs
    let pts = crate::gemma::static_gemma_waypoints(&t.from, &t.to);
    if !pts.is_empty() {
        return pts;
    }
    // Fallback coude L
    let src = states.iter().find(|s| s.id == t.from);
    let dst = states.iter().find(|s| s.id == t.to);
    if let (Some(s), Some(d)) = (src, dst) {
        let hw_s = if s.w > 0.0 { s.w } else { NODE_W } / 2.0;
        let hh_s = if s.h > 0.0 { s.h } else { NODE_H } / 2.0;
        let hw_d = if d.w > 0.0 { d.w } else { NODE_W } / 2.0;
        let hh_d = if d.h > 0.0 { d.h } else { NODE_H } / 2.0;
        let dx = d.pos[0] - s.pos[0];
        let dy = d.pos[1] - s.pos[1];
        if dx.abs() >= dy.abs() {
            let (ex, ax) = if dx >= 0.0 {
                (s.pos[0] + hw_s, d.pos[0] - hw_d)
            } else {
                (s.pos[0] - hw_s, d.pos[0] + hw_d)
            };
            let mid = (ex + ax) / 2.0;
            return vec![[ex, s.pos[1]], [mid, s.pos[1]], [mid, d.pos[1]], [ax, d.pos[1]]];
        } else {
            let (ey, ay) = if dy >= 0.0 {
                (s.pos[1] + hh_s, d.pos[1] - hh_d)
            } else {
                (s.pos[1] - hh_s, d.pos[1] + hh_d)
            };
            let mid = (ey + ay) / 2.0;
            return vec![[s.pos[0], ey], [s.pos[0], mid], [d.pos[0], mid], [d.pos[0], ay]];
        }
    }
    vec![]
}

/// Id de la transition dont un segment est proche du point canvas `cv`.
fn hit_trans(
    transitions: &[GemmaTransition],
    states:      &[GemmaState],
    cv:          Pos2,
) -> Option<u32> {
    for t in transitions {
        let pts = resolve_waypoints(t, states);
        for w in pts.windows(2) {
            if dist_point_seg(cv, w[0], w[1]) < ARROW_HIT {
                return Some(t.id);
            }
        }
    }
    None
}

fn dist_point_seg(p: Pos2, a: [f32; 2], b: [f32; 2]) -> f32 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    if len2 < 0.001 {
        return ((p.x - a[0]).powi(2) + (p.y - a[1]).powi(2)).sqrt();
    }
    let t = ((p.x - a[0]) * dx + (p.y - a[1]) * dy) / len2;
    let t = t.clamp(0.0, 1.0);
    let cx = a[0] + t * dx;
    let cy = a[1] + t * dy;
    ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt()
}

/// Dessine une transition avec sa tête de flèche et son label de condition.
fn draw_arrow(
    painter:  &Painter,
    t:        &GemmaTransition,
    states:   &[GemmaState],
    offset:   Vec2,
    zoom:     f32,
    origin:   Pos2,
    selected: bool,
) {
    let pts = resolve_waypoints(t, states);
    if pts.len() < 2 { return; }

    // Rouge si la condition n'est pas définie (TRUE = valeur par défaut = non renseignée)
    let has_cond = !matches!(t.condition, Expr::True | Expr::False);
    let col = if selected {
        Color32::from_rgb(241, 196, 15)
    } else if !has_cond {
        Color32::from_rgb(210, 60, 60)   // rouge = sans condition
    } else {
        Color32::from_rgb(175, 185, 205)
    };
    let stroke = Stroke::new(1.5 * zoom, col);

    let sp: Vec<Pos2> = pts.iter()
        .map(|p| canvas_to_screen(*p, offset, zoom, origin))
        .collect();
    for w in sp.windows(2) {
        painter.line_segment([w[0], w[1]], stroke);
    }

    // Tête de flèche
    let n = sp.len();
    if n >= 2 {
        let tip  = sp[n - 1];
        let prev = sp[n - 2];
        let dv   = (tip - prev).normalized();
        let perp = Vec2::new(-dv.y, dv.x);
        let sz   = 6.0 * zoom;
        painter.line_segment([tip, tip - dv * sz + perp * (sz * 0.45)], stroke);
        painter.line_segment([tip, tip - dv * sz - perp * (sz * 0.45)], stroke);
    }
    // Pas de label de condition sur le canvas — affichage en hover uniquement
}

// ── Sauvegarde des routes ───────────────────────────────────────────────────────

/// Exporte les waypoints et conditions de toutes les transitions dans `data/gemma_routes.json`.
fn save_routes(gemma: &Gemma) -> Result<String, String> {
    use std::fs;
    #[derive(serde::Serialize)]
    struct RouteEntry<'a> {
        from:      &'a str,
        to:        &'a str,
        points:    &'a [[f32; 2]],
        condition: String,
    }
    let entries: Vec<RouteEntry<'_>> = gemma.transitions.iter()
        .filter(|t| !t.waypoints.is_empty())
        .map(|t| RouteEntry {
            from:      &t.from,
            to:        &t.to,
            points:    &t.waypoints,
            condition: t.condition.to_display(),
        })
        .collect();
    let json = serde_json::to_string_pretty(&entries)
        .map_err(|e| e.to_string())?;
    fs::create_dir_all("data").map_err(|e| e.to_string())?;
    let path = "data/gemma_routes.json";
    fs::write(path, json.as_bytes()).map_err(|e| e.to_string())?;
    Ok(path.to_string())
}

// ── Dessin des états ────────────────────────────────────────────────────────────

fn draw_gemma_node(
    painter: &Painter,
    state: &GemmaState,
    offset: Vec2,
    zoom: f32,
    origin: Pos2,
    selected: bool,
    sim_active: bool,
) {
    let eff_w = if state.w > 0.0 { state.w } else { NODE_W };
    let eff_h = if state.h > 0.0 { state.h } else { NODE_H };
    let cx = state.pos[0] * zoom + offset.x + origin.x;
    let cy = state.pos[1] * zoom + offset.y + origin.y;
    let hw = eff_w * zoom / 2.0;
    let hh = eff_h * zoom / 2.0;
    let rect = Rect::from_min_max(Pos2::new(cx - hw, cy - hh), Pos2::new(cx + hw, cy + hh));

    let base = state.state_type.color();
    let bg = Color32::from_rgba_unmultiplied(base.r() / 2, base.g() / 2, base.b() / 2, 240);
    let border_color = if sim_active  { Color32::from_rgb(255, 220, 60) }
                       else if selected { Color32::from_rgb(241, 196, 15) }
                       else             { base };
    let rounding = CornerRadius::same((4.0 * zoom).round().clamp(0.0, 255.0) as u8);

    // Aura dorée pour l'état actif en simulation
    if sim_active {
        let aura = 6.0 * zoom;
        let aura_rect = Rect::from_min_max(
            Pos2::new(cx - hw - aura, cy - hh - aura),
            Pos2::new(cx + hw + aura, cy + hh + aura),
        );
        painter.rect_filled(
            aura_rect,
            CornerRadius::same((7.0 * zoom).round().clamp(0.0, 255.0) as u8),
            Color32::from_rgba_unmultiplied(255, 220, 60, 55),
        );
        painter.rect_stroke(
            aura_rect,
            CornerRadius::same((7.0 * zoom).round().clamp(0.0, 255.0) as u8),
            Stroke::new(2.5 * zoom, Color32::from_rgb(255, 220, 60)),
            egui::StrokeKind::Outside,
        );
    }

    painter.rect_filled(rect, rounding, bg);
    painter.rect_stroke(rect, rounding,
        Stroke::new(if selected { 2.5 } else { 1.5 } * zoom, border_color),
        egui::StrokeKind::Outside,
    );

    // A1 = état initial → double bord intérieur
    if state.id == "A1" {
        let pad = 3.0 * zoom;
        let inner = Rect::from_min_max(
            Pos2::new(cx - hw + pad, cy - hh + pad),
            Pos2::new(cx + hw - pad, cy + hh - pad),
        );
        painter.rect_stroke(inner, rounding,
            Stroke::new(1.0 * zoom, border_color),
            egui::StrokeKind::Outside,
        );
    }

    // ── Cercle (contour seulement) avec l'ID ──────────────────────────────
    // Police UNIFORME pour tous les états : cercles et textes de même taille.
    // Ligne imaginaire = circle_cy.  Le centre du cercle ET le centre de la
    // première ligne de texte sont positionnés exactement sur cette ligne.
    let narrow    = eff_w < 65.0;
    let margin    = 4.0 * zoom;
    let font_sz   = 7.5 * zoom;   // identique pour tous les états
    let circle_r  = font_sz * 0.85;
    let circle_cx = cx - hw + margin + circle_r;
    let circle_cy = cy - hh + margin + circle_r;
    // Haut du bloc texte tel que son centre (½ hauteur de ligne) = circle_cy
    let text_top  = circle_cy - font_sz * 0.5;

    painter.circle_stroke(
        Pos2::new(circle_cx, circle_cy), circle_r,
        Stroke::new(0.9 * zoom, Color32::WHITE),
    );
    painter.text(
        Pos2::new(circle_cx, circle_cy),
        egui::Align2::CENTER_CENTER,
        &state.id,
        FontId::proportional(font_sz),
        Color32::WHITE,
    );

    // ── Nom de l'état ─────────────────────────────────────────────────────
    let name = if !state.description.is_empty() {
        state.description.as_str()
    } else {
        state_name(&state.id)
    };
    if !name.is_empty() {
        let x_after_circle = circle_cx + circle_r + 2.0 * zoom;

        if !narrow {
            // Rectangles larges/moyens : tout le texte à droite,
            // première ligne centrée sur la ligne du cercle.
            painter.text(
                Pos2::new(x_after_circle, text_top),
                egui::Align2::LEFT_TOP,
                name,
                FontId::proportional(font_sz),
                Color32::WHITE,
            );
        } else {
            // Rectangles étroits :
            //   - premier mot à droite du cercle, sur la ligne imaginaire
            //   - suite depuis le bord gauche, juste sous le cercle
            let (first, rest) = state_name_split(name);
            if !first.is_empty() {
                painter.text(
                    Pos2::new(x_after_circle, text_top),
                    egui::Align2::LEFT_TOP,
                    first,
                    FontId::proportional(font_sz),
                    Color32::WHITE,
                );
            }
            if !rest.is_empty() {
                let rest_y = circle_cy + circle_r + 2.0 * zoom;
                painter.text(
                    Pos2::new(cx - hw + margin, rest_y),
                    egui::Align2::LEFT_TOP,
                    rest,
                    FontId::proportional(font_sz),
                    Color32::WHITE,
                );
            }
        }
    }

    // ── Action associée (texte vert, centré en bas du rectangle) ──────────
    if !state.action.is_empty() {
        let action_y = cy + hh - margin - font_sz * 0.55;
        painter.text(
            Pos2::new(cx, action_y),
            egui::Align2::CENTER_BOTTOM,
            &state.action,
            FontId::proportional(font_sz * 0.95),
            Color32::from_rgb(80, 220, 100),
        );
    }
}

fn state_name_split(name: &str) -> (&str, &str) {
    // Cherche le premier \n ou espace pour couper après le premier mot
    if let Some(pos) = name.find('\n') {
        (&name[..pos], name[pos + 1..].trim_start_matches('\n'))
    } else if let Some(pos) = name.find(' ') {
        (&name[..pos], name[pos + 1..].trim_start())
    } else {
        (name, "")
    }
}

fn state_name(id: &str) -> &'static str {
    match id {
        // Larges (≥ 100px) : tout à droite du cercle
        "A1" => "Arrêt dans\nl'état initial",
        "A5" => "Préparation pour\nremise en route\naprès défaillance",
        "A6" => "Mise en route\nnormale",
        "D1" => "Arrêt d'urgence",
        "D3" => "Production tout de même",
        "F1" => "Production normale",
        // Moyens (83-98px) : tout à droite du cercle
        "A4" => "Arrêt obtenu\ndans état\ndéterminé",
        "A7" => "Mise en route\naprès défaillance",
        "D2" => "Diagnostic et/ou\ntraitement de\ndéfaillance",
        // Étroits w≈47 (A2/A3/F2/F3) :
        //   1re ligne → droite cercle  |  suite → dessous bord gauche
        //   À zoom=2 (fit standard), inner_w ≈ 78px, avg char ≈ 8px → ≤9 chars/ligne
        "A2" => "Arrêt\ndemandé\nen fin\nde cycle",
        "A3" => "Arrêt\ndemandé\ndans état\ndéterminé",
        "F2" => "Marches\nde\npréparation",
        "F3" => "Marches\nde clôture",
        // Étroits w=64 (F4/F5/F6) :
        //   À zoom=2, inner_w ≈ 112px ≤ 14 chars/ligne
        "F4" => "Marches de\nvérification\ndans le\ndésordre",
        "F5" => "Marches de\nvérification\ndans l'ordre",
        "F6" => "Marches\nde test",
        _    => "",
    }
}

fn draw_gemma_link(
    painter: &Painter,
    from_id: &str,
    from: [f32; 2], from_w: f32, from_h: f32,
    to_id: &str,
    to: [f32; 2], to_w: f32, to_h: f32,
    offset: Vec2,
    zoom: f32,
    origin: Pos2,
    cond: &str,
    selected: bool,
) {
    if (from[0] - to[0]).abs() < 2.0 && (from[1] - to[1]).abs() < 2.0 {
        return;
    }

    let color  = if selected { Color32::from_rgb(241, 196, 15) } else { Color32::from_rgb(180, 190, 210) };
    let stroke = Stroke::new(1.5 * zoom, color);

    let to_s = |cx: f32, cy: f32| -> Pos2 {
        Pos2::new(cx * zoom + offset.x + origin.x, cy * zoom + offset.y + origin.y)
    };

    // ── Routes statiques GEMMA ────────────────────────────────────────────
    if let Some(pts) = static_gemma_route(from_id, to_id) {
        let sp: Vec<Pos2> = pts.iter().map(|p| to_s(p[0], p[1])).collect();
        for w in sp.windows(2) {
            painter.line_segment([w[0], w[1]], stroke);
        }
        // Tête de flèche au dernier segment
        let n = sp.len();
        if n >= 2 {
            let p_to   = sp[n - 1];
            let p_prev = sp[n - 2];
            let dv = (p_to - p_prev).normalized();
            let perp = Vec2::new(-dv.y, dv.x);
            let sz = 6.0 * zoom;
            painter.line_segment([p_to, p_to - dv * sz + perp * (sz * 0.45)], stroke);
            painter.line_segment([p_to, p_to - dv * sz - perp * (sz * 0.45)], stroke);
        }
        // Label condition si renseigné
        if !cond.is_empty() && cond != "FALSE" && cond != "TRUE" {
            let n = sp.len();
            if n >= 2 {
                let mid_idx = n / 2;
                let lp = Pos2::new(
                    (sp[mid_idx - 1].x + sp[mid_idx].x) * 0.5 + 3.0,
                    (sp[mid_idx - 1].y + sp[mid_idx].y) * 0.5 - 8.0 * zoom,
                );
                painter.text(lp, egui::Align2::LEFT_CENTER, cond,
                    FontId::proportional(9.0 * zoom), Color32::from_rgb(200, 160, 230));
            }
        }
        return;
    }

    // ── Fallback : coude L automatique ─────────────────────────────────
    let hw_src = (if from_w > 0.0 { from_w } else { NODE_W }) / 2.0;
    let hh_src = (if from_h > 0.0 { from_h } else { NODE_H }) / 2.0;
    let hw_dst = (if to_w   > 0.0 { to_w   } else { NODE_W }) / 2.0;
    let hh_dst = (if to_h   > 0.0 { to_h   } else { NODE_H }) / 2.0;

    let dx = to[0] - from[0];
    let dy = to[1] - from[1];

    let (dir_x, dir_y, ex, ey, ax, ay) = if dx.abs() >= dy.abs() {
        if dx >= 0.0 {
            (1.0_f32,  0.0_f32, from[0] + hw_src, from[1], to[0] - hw_dst, to[1])
        } else {
            (-1.0_f32, 0.0_f32, from[0] - hw_src, from[1], to[0] + hw_dst, to[1])
        }
    } else if dy >= 0.0 {
        (0.0_f32,  1.0_f32, from[0], from[1] + hh_src, to[0], to[1] - hh_dst)
    } else {
        (0.0_f32, -1.0_f32, from[0], from[1] - hh_src, to[0], to[1] + hh_dst)
    };

    let p_from = to_s(ex, ey);
    let p_to   = to_s(ax, ay);

    let (p_mid1, p_mid2, label_pos) = if dx.abs() >= dy.abs() {
        let mid_x = (ex + ax) / 2.0;
        (to_s(mid_x, ey), to_s(mid_x, ay), to_s(mid_x, ey.min(ay) - 12.0 / zoom))
    } else {
        let mid_y = (ey + ay) / 2.0;
        (to_s(ex, mid_y), to_s(ax, mid_y), to_s(ex.min(ax) + 3.0 / zoom, mid_y - 10.0 / zoom))
    };

    painter.line_segment([p_from, p_mid1], stroke);
    painter.line_segment([p_mid1, p_mid2], stroke);
    painter.line_segment([p_mid2, p_to],   stroke);

    let sz   = 7.0 * zoom;
    let dir  = Vec2::new(dir_x, dir_y);
    let perp = Vec2::new(-dir_y, dir_x);
    painter.line_segment([p_to, p_to - dir * sz + perp * (sz / 2.0)], stroke);
    painter.line_segment([p_to, p_to - dir * sz - perp * (sz / 2.0)], stroke);

    painter.text(label_pos, egui::Align2::LEFT_CENTER, cond,
        FontId::proportional(10.0 * zoom), Color32::from_rgb(200, 160, 230));
}

fn static_gemma_route(from_id: &str, to_id: &str) -> Option<Vec<[f32; 2]>> {
    // Bords des états :
    //  A1: L=221 R=336 T=60  B=100 CX=278 CY=80
    //  A2: L=219 R=266 T=230 B=345 CX=242 CY=287
    //  A3: L=291 R=337 T=230 B=317 CX=314 CY=273
    //  A4: L=255 R=338 T=142 B=194 CX=296 CY=168
    //  A5: L=64  R=181 T=230 B=345 CX=122 CY=287
    //  A6: L=64  R=181 T=58  B=103 CX=122 CY=80
    //  A7: L=83  R=181 T=142 B=194 CX=132 CY=168
    //  D1: L=63  R=337 T=452 B=497 CX=200 CY=474
    //  D2: L=83  R=181 T=369 B=416 CX=132 CY=392
    //  D3: L=219 R=337 T=369 B=416 CX=278 CY=392
    //  F1: L=409 R=564 T=252 B=417 CX=486 CY=334
    //  F2: L=446 R=493 T=144 B=227 CX=469 CY=185
    //  F3: L=517 R=564 T=144 B=227 CX=540 CY=185
    //  F4: L=617 R=681 T=45  B=123 CX=649 CY=84
    //  F5: L=617 R=681 T=158 B=366 CX=649 CY=262
    //  F6: L=617 R=681 T=393 B=497 CX=649 CY=445
    //
    // Corridors (espacement 8px entre fleches paralleles) :
    //   x_lft1=40  gauche  D2->A5
    //   x_lft2=48  gauche  D1->A5
    //   x_A5A6=55  gauche  A5->A6 (plus proche d'A5/A6)
    //   x_RA1=352  droite zone A  F1->A2
    //   x_RA2=361  droite zone A  F1->A3 / F1->D3
    //   x_mid=395  couloir central A<->F
    //   x_sep1=583 couloir droit F  aller (F1->F4, F5->F1, F6->F1)
    //   x_sep2=593 couloir droit F  retour (A1->F5, F1->F5, F1->F6)
    //   y_top1=22  corridor superieur  F3->A1 / F4->A6
    //   y_top2=32  corridor superieur  A1->F4
    //   y_dg1=350  entre A5.B=345 et D2.T=369  F1->D2
    //   y_dg2=358  entre A5.B=345 et D2.T=369  D3->D2 deja horizontal
    //   y_d1g=437  entre D2.B=416 et D1.T=452  F1->D1

    Some(match (from_id, to_id) {
        // == Zone A -> zone F (demandes de marche) ==
        // A1 -> F1 : droite A1 -> couloir x=395 -> descend -> gauche F1
        ("A1", "F1") => vec![[336.,  80.], [395.,  80.], [395., 334.], [409., 334.]],
        // A1 -> F2 : droite A1 -> couloir x=395 -> monte -> gauche F2
        ("A1", "F2") => vec![[336.,  80.], [395.,  80.], [395., 144.], [446., 144.]],
        // A1 -> F4 : haut A1 -> couloir y=32 (au-dessus F2/F3) -> droite -> bas F4
        ("A1", "F4") => vec![[278.,  60.], [278.,  32.], [649.,  32.], [649.,  45.]],
        // A1 -> F5 : droite A1 -> couloir x=593 -> descend -> gauche F5
        ("A1", "F5") => vec![[336.,  80.], [593.,  80.], [593., 262.], [617., 262.]],
        // A4 -> F1 : droite A4 -> couloir x=395 -> descend -> gauche F1
        ("A4", "F1") => vec![[338., 168.], [395., 168.], [395., 334.], [409., 334.]],

        // == Zone A interne ==
        // A2 -> A1 : haut A2 -> bas A1 (x=242 ne traverse pas A4)
        ("A2", "A1") => vec![[242., 230.], [242., 100.]],
        // A3 -> A4 : haut A3 -> bas A4 (x=314 dans largeur de A4)
        ("A3", "A4") => vec![[314., 230.], [314., 194.]],
        // A5 -> A6 : couloir x=55, seul sur ce couloir
        ("A5", "A6") => vec![[ 64., 287.], [ 55., 287.], [ 55.,  80.], [ 64.,  80.]],
        // A5 -> A7 : haut direct (x=122 dans largeur A7)
        ("A5", "A7") => vec![[122., 230.], [122., 194.]],
        // A6 -> A1 : horizontal droite (y=80)
        ("A6", "A1") => vec![[181.,  80.], [221.,  80.]],
        // A7 -> A4 : horizontal droite (y=168)
        ("A7", "A4") => vec![[181., 168.], [255., 168.]],
        // A7 -> A6 : haut direct (x=132 dans largeur A6)
        ("A7", "A6") => vec![[132., 142.], [132., 103.]],

        // == Zone F -> zone A (demandes d'arret) ==
        // F1 -> A2 : gauche F1 -> couloir x=352 -> monte -> droite A2
        ("F1", "A2") => vec![[409., 334.], [352., 334.], [352., 287.], [266., 287.]],
        // F1 -> A3 : gauche F1 -> couloir x=361 -> monte -> droite A3
        ("F1", "A3") => vec![[409., 334.], [361., 334.], [361., 273.], [337., 273.]],

        // == Zone F -> zone D (detections defaillances) ==
        // F1 -> D1 : bas F1 (x=486) -> couloir y=437 -> horizontal -> haut D1
        ("F1", "D1") => vec![[486., 417.], [486., 437.], [200., 437.], [200., 452.]],
        // F1 -> D2 : gauche F1 -> couloir x=352 -> descend -> couloir y=350 -> droite D2
        ("F1", "D2") => vec![[409., 334.], [352., 334.], [352., 350.], [181., 350.], [181., 369.]],
        // F1 -> D3 : gauche F1 -> couloir x=361 -> descend -> droite D3
        ("F1", "D3") => vec![[409., 334.], [361., 334.], [361., 392.], [337., 392.]],

        // == Zone F interne ==
        // F1 -> F3 : haut F1 a x=540 -> bas F3
        ("F1", "F3") => vec![[540., 252.], [540., 227.]],
        // F1 -> F4 : droite F1 -> couloir x=583 -> monte -> gauche F4
        ("F1", "F4") => vec![[564., 334.], [583., 334.], [583.,  84.], [617.,  84.]],
        // F1 -> F5 : droite F1 -> couloir x=593 -> monte -> gauche F5
        ("F1", "F5") => vec![[564., 334.], [593., 334.], [593., 262.], [617., 262.]],
        // F1 -> F6 : bas F1 (x=564) -> couloir x=593 -> descend -> gauche F6
        ("F1", "F6") => vec![[564., 417.], [593., 417.], [593., 445.], [617., 445.]],
        // F2 -> F1 : bas F2 -> haut F1 (x=469)
        ("F2", "F1") => vec![[469., 227.], [469., 252.]],
        // F3 -> A1 : haut F3 -> couloir y=22 -> gauche -> haut A1
        ("F3", "A1") => vec![[540., 144.], [540.,  22.], [278.,  22.], [278.,  60.]],
        // F4 -> A6 : haut F4 -> couloir y=22 -> gauche -> droite A6
        ("F4", "A6") => vec![[649.,  45.], [649.,  22.], [181.,  22.], [181.,  80.]],
        // F5 -> F1 : gauche F5 -> couloir x=583 -> descend -> droite F1
        ("F5", "F1") => vec![[617., 262.], [583., 262.], [583., 334.], [564., 334.]],
        // F5 -> F4 : haut F5 -> bas F4 (x=649)
        ("F5", "F4") => vec![[649., 158.], [649., 123.]],
        // F6 -> F1 : gauche F6 -> couloir x=593 -> monte -> droite F1
        ("F6", "F1") => vec![[617., 445.], [593., 445.], [593., 334.], [564., 334.]],
        // F6 -> D1 : bas F6 -> couloir y=510 (sous D1) -> gauche -> droite D1
        ("F6", "D1") => vec![[649., 497.], [649., 510.], [337., 510.], [337., 497.]],

        // == Zone D ==
        // D1 -> A5 : gauche D1 -> couloir x=48 -> monte -> gauche A5
        ("D1", "A5") => vec![[ 63., 474.], [ 48., 474.], [ 48., 287.], [ 64., 287.]],
        // D1 -> D2 : haut D1 a x=132 -> bas D2
        ("D1", "D2") => vec![[132., 452.], [132., 416.]],
        // D2 -> A5 : gauche D2 -> couloir x=40 (ecarte de D1->A5) -> monte -> gauche A5
        ("D2", "A5") => vec![[ 83., 392.], [ 40., 392.], [ 40., 287.], [ 64., 287.]],
        // D3 -> D2 : horizontal gauche (y=392)
        ("D3", "D2") => vec![[219., 392.], [181., 392.]],
        // D3 -> A2 : haut direct (x=242)
        ("D3", "A2") => vec![[242., 369.], [242., 345.]],
        // D3 -> A3 : haut direct (x=314)
        ("D3", "A3") => vec![[314., 369.], [314., 317.]],

        _ => return None,
    })
}

fn fit_states_to_rect(states: &[GemmaState], rect: egui::Rect) -> (f32, f32, f32) {
    let pad = 24.0_f32;
    let min_x = states.iter()
        .map(|s| s.pos[0] - (if s.w > 0.0 { s.w } else { NODE_W }) / 2.0)
        .fold(f32::MAX, f32::min);
    let min_y = states.iter()
        .map(|s| s.pos[1] - (if s.h > 0.0 { s.h } else { NODE_H }) / 2.0)
        .fold(f32::MAX, f32::min);
    let max_x = states.iter()
        .map(|s| s.pos[0] + (if s.w > 0.0 { s.w } else { NODE_W }) / 2.0)
        .fold(f32::MIN, f32::max);
    let max_y = states.iter()
        .map(|s| s.pos[1] + (if s.h > 0.0 { s.h } else { NODE_H }) / 2.0)
        .fold(f32::MIN, f32::max);

    let data_w = (max_x - min_x + 2.0 * pad).max(1.0);
    let data_h = (max_y - min_y + 2.0 * pad).max(1.0);
    let canvas_w = rect.width();
    let canvas_h = rect.height();

    let zoom = (canvas_w / data_w).min(canvas_h / data_h).clamp(0.2, 4.0);

    let cx_data = (min_x + max_x) / 2.0;
    let cy_data = (min_y + max_y) / 2.0;
    let offset_x = canvas_w / 2.0 - cx_data * zoom;
    let offset_y = canvas_h / 2.0 - cy_data * zoom;

    (offset_x, offset_y, zoom)
}

fn draw_grid(painter: &Painter, rect: Rect, offset: Vec2, zoom: f32) {
    let sz = 24.0 * zoom;
    let color = Stroke::new(0.5, Color32::from_gray(45));
    let x0 = rect.min.x + (offset.x % sz + sz) % sz;
    let y0 = rect.min.y + (offset.y % sz + sz) % sz;
    let mut x = x0;
    while x <= rect.max.x { painter.line_segment([Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)], color); x += sz; }
    let mut y = y0;
    while y <= rect.max.y { painter.line_segment([Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)], color); y += sz; }
}

fn tool_btn(ui: &mut egui::Ui, label: &str, tool: GemmaTool, current: &mut GemmaTool) {
    let active = *current == tool;
    let resp = ui.selectable_label(active, label);
    if resp.clicked() {
        *current = tool;
    }
}
