// gui/canvas_editor.rs — Éditeur de canvas GRAFCET réutilisable
//
// Refactorisé depuis editor.rs : n'est plus un eframe::App mais un composant
// appelable depuis GrafcetsPage via canvas_editor.show(ui, grafcet).
//
// Gère : outils (Select/AddStep/AddTransition/Delete), pan/zoom, drag,
// panneau de propriétés interne, save/load par onglet.

use egui::{Key, Modifiers, Pos2, Vec2};

use crate::grafcet::{Grafcet, StepKind};
use crate::gui::canvas::{
    draw_links, draw_step_ghost, draw_steps, draw_transitions, hit_transition, STEP_H, STEP_W,
};

#[derive(Debug, Clone, PartialEq)]
enum Tool {
    Select,
    AddStep,
    AddTransition,
    Delete,
}

impl Default for Tool {
    fn default() -> Self { Tool::Select }
}

pub struct CanvasEditor {
    tool: Tool,
    offset: Vec2,
    zoom: f32,
    dragging_step: Option<u32>,
    drag_offset: Vec2,
    dragging_trans: Option<u32>,
    conn_from: Option<u32>,
    selected_step: Option<u32>,
    selected_trans: Option<u32>,
    /// Drag du handle de routage Y (segment horizontal src→barre)
    dragging_route_y: Option<u32>,
    /// Drag du handle de décrochage X (boucle en retour)
    dragging_route_x: Option<u32>,
    /// Chemin de sauvegarde propre à cet onglet
    current_path: Option<std::path::PathBuf>,    /// Demande un fit-to-content au prochain frame
    pub pending_fit: bool,}

impl Default for CanvasEditor {
    fn default() -> Self {
        Self {
            tool: Tool::default(),
            offset: Vec2::ZERO,
            zoom: 1.0,
            dragging_step: None,
            drag_offset: Vec2::ZERO,
            dragging_trans: None,
            conn_from: None,
            selected_step: None,
            selected_trans: None,
            dragging_route_y: None,
            dragging_route_x: None,
            current_path: None,
            pending_fit: false,
        }
    }
}

impl CanvasEditor {
    /// Calcule offset + zoom pour centrer tout le contenu dans `canvas_size`.
    pub fn fit_to_content(&mut self, grafcet: &Grafcet, canvas_size: egui::Vec2) {
        if grafcet.steps.is_empty() || canvas_size.x <= 0.0 || canvas_size.y <= 0.0 { return; }
        let min_x = grafcet.steps.iter().map(|s| s.pos[0]).fold(f32::INFINITY,    f32::min);
        let max_x = grafcet.steps.iter().map(|s| s.pos[0]).fold(f32::NEG_INFINITY, f32::max);
        let min_y = grafcet.steps.iter().map(|s| s.pos[1]).fold(f32::INFINITY,    f32::min);
        let max_y = grafcet.steps.iter().map(|s| s.pos[1]).fold(f32::NEG_INFINITY, f32::max);
        // Bornes de contenu en coordonnées canvas (marges : routing + labels + dernière transition)
        let left  = min_x - STEP_W / 2.0 - 90.0; // réserve pour la route de loopback
        let right = max_x + STEP_W / 2.0 + 120.0; // réserve pour labels de conditions
        let top   = min_y - STEP_H / 2.0 - 20.0;
        let bot   = max_y + STEP_H / 2.0 + 80.0;  // dernière transition + mèches
        let cw = right - left;
        let ch = bot   - top;
        if cw <= 0.0 || ch <= 0.0 { return; }
        let pad  = 24.0;
        let zoom = ((canvas_size.x - 2.0 * pad) / cw)
            .min((canvas_size.y - 2.0 * pad) / ch)
            .clamp(0.15, 1.5);
        self.zoom   = zoom;
        self.offset = egui::Vec2::new(
            canvas_size.x / 2.0 - (left + cw / 2.0) * zoom,
            canvas_size.y / 2.0 - (top  + ch / 2.0) * zoom,
        );
    }
}

impl CanvasEditor {
    /// Affiche le canvas pour `grafcet`. Retourne Some(message de statut) si besoin.
    pub fn show(&mut self, ui: &mut egui::Ui, grafcet: &mut Grafcet) -> Option<String> {
        let ctx = ui.ctx().clone();
        let mut status_out: Option<String> = None;

        // ── Barre d'outils ──────────────────────────────────────────────────
        egui::Panel::top("canvas_toolbar")
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(26, 37, 47))
                    .inner_margin(egui::Margin::same(6)),
            )
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    tool_btn(ui, "↖ Sélect.",     Tool::Select,        &mut self.tool);
                    tool_btn(ui, "⬛ Étape",       Tool::AddStep,       &mut self.tool);
                    tool_btn(ui, "↕ Transition",   Tool::AddTransition, &mut self.tool);
                    tool_btn(ui, "🗑 Suppr.",       Tool::Delete,        &mut self.tool);
                    ui.separator();
                    if ui.button("🔍+").clicked() { self.zoom = (self.zoom * 1.2).min(5.0); }
                    if ui.button("🔍-").clicked() { self.zoom = (self.zoom / 1.2).max(0.2); }
                    if ui.button("100%").clicked() { self.zoom = 1.0; self.offset = Vec2::ZERO; }
                    if ui.button("Centrer").clicked() { self.pending_fit = true; }
                    ui.separator();

                    // Menu Fichier rapide
                    ui.menu_button("Fichier", |ui| {
                        if ui.button("Ouvrir…").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("GRAFCET JSON", &["json"])
                                .pick_file()
                            {
                                match crate::persistence::load_json(&path) {
                                    Ok(g) => {
                                        *grafcet = g;
                                        self.current_path = Some(path);
                                        status_out = Some("Grafcet chargé ✓".into());
                                    }
                                    Err(e) => status_out = Some(format!("Erreur : {e}")),
                                }
                            }
                            ui.close();
                        }
                        if ui.button("Enregistrer").clicked() {
                            status_out = self.save(grafcet);
                            ui.close();
                        }
                        if ui.button("Enregistrer sous…").clicked() {
                            status_out = self.save_as(grafcet);
                            ui.close();
                        }
                        if ui.button("🗑 Vider").clicked() {
                            grafcet.steps.clear();
                            grafcet.transitions.clear();
                            grafcet.next_step_id = 0;
                            grafcet.next_trans_id = 0;
                            self.selected_step = None;
                            self.dragging_step = None;
                            self.dragging_trans = None;
                            self.conn_from = None;
                            status_out = Some("Canvas vidé".into());
                            ui.close();
                        }
                    });

                    ui.separator();
                    ui.label(
                        egui::RichText::new(format!(
                            "Étapes : {}  Transitions : {}  Zoom : {:.0}%",
                            grafcet.steps.len(),
                            grafcet.transitions.len(),
                            self.zoom * 100.0,
                        ))
                        .size(11.0)
                        .color(egui::Color32::from_rgb(127, 176, 211)),
                    );

                    // Raccourcis clavier
                    if ctx.input(|i| i.key_pressed(Key::Escape)) {
                        self.tool = Tool::Select;
                        self.conn_from = None;
                    }
                    if ctx.input(|i| {
                        i.modifiers.contains(Modifiers::CTRL) && i.key_pressed(Key::S)
                    }) {
                        status_out = self.save(grafcet);
                    }
                });
            });

        // ── Panneau propriétés (droite) ──────────────────────────────────────
        egui::Panel::right("canvas_props")
            .min_size(200.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(22, 32, 42))
                    .inner_margin(egui::Margin::same(8)),
            )
            .show_inside(ui, |ui| {
                ui.heading("Propriétés");
                ui.separator();

                if let Some(tid) = self.selected_trans {
                    // ── Propriétés d'une transition sélectionnée ──────────
                    let trans_data = grafcet.transition(tid).map(|t| {
                        (t.from_step, t.to_step, t.condition.clone(), t.pos, t.route_y, t.dst_route_x)
                    });
                    if let Some((from_step, to_step, cond, pos, route_y, dst_route_x)) = trans_data {
                        ui.label(egui::RichText::new(format!("Transition T{tid}")).strong());
                        ui.separator();

                        ui.horizontal(|ui| {
                            ui.label("Condition :");
                        });
                        let mut c = cond.clone();
                        if ui.text_edit_singleline(&mut c).changed() {
                            if let Some(t) = grafcet.transition_mut(tid) {
                                t.condition = c;
                            }
                        }

                        ui.separator();

                        // Étape source
                        ui.horizontal(|ui| {
                            ui.label("De :");
                            let step_ids: Vec<u32> = grafcet.steps.iter().map(|s| s.id).collect();
                            let mut sel_from = from_step;
                            egui::ComboBox::new(format!("trans_from_{tid}"), "")
                                .selected_text(format!("E{sel_from}"))
                                .show_ui(ui, |ui| {
                                    for sid in &step_ids {
                                        ui.selectable_value(&mut sel_from, *sid, format!("E{sid}"));
                                    }
                                });
                            if sel_from != from_step {
                                if let Some(t) = grafcet.transition_mut(tid) {
                                    t.from_step = sel_from;
                                }
                            }
                        });

                        // Étape destination
                        ui.horizontal(|ui| {
                            ui.label("Vers :");
                            let step_ids: Vec<u32> = grafcet.steps.iter().map(|s| s.id).collect();
                            let mut sel_to = to_step;
                            egui::ComboBox::new(format!("trans_to_{tid}"), "")
                                .selected_text(format!("E{sel_to}"))
                                .show_ui(ui, |ui| {
                                    for sid in &step_ids {
                                        ui.selectable_value(&mut sel_to, *sid, format!("E{sid}"));
                                    }
                                });
                            if sel_to != to_step {
                                if let Some(t) = grafcet.transition_mut(tid) {
                                    t.to_step = sel_to;
                                }
                            }
                        });

                        ui.separator();
                        ui.label(format!("Pos barre : ({:.0}, {:.0})", pos[0], pos[1]));

                        // ── Routage des liaisons ───────────────────────────
                        ui.separator();
                        ui.label(egui::RichText::new("Routage des liaisons").color(
                            egui::Color32::from_rgb(255, 160, 50)));

                        // Handle Y (segment horizontal haut)
                        ui.horizontal(|ui| {
                            let label = if let Some(ry) = route_y {
                                format!("Y haut : {:.0}", ry)
                            } else {
                                "Y haut : auto".to_string()
                            };
                            ui.label(egui::RichText::new(label).size(11.0)
                                .color(egui::Color32::from_rgb(255, 140, 0)));
                            if route_y.is_some() {
                                if ui.small_button("↺").on_hover_text("Remettre en automatique").clicked() {
                                    if let Some(t) = grafcet.transition_mut(tid) {
                                        t.route_y = None;
                                    }
                                }
                            }
                        });
                        ui.label(egui::RichText::new(
                            "● Glisser le rond orange sur le canvas"
                        ).weak().size(10.0));

                        // Handle X (décrochage boucle retour)
                        if dst_route_x.is_some() {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(
                                    format!("X retour : {:.0}", dst_route_x.unwrap()))
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(0, 200, 180)));
                                if ui.small_button("↺").on_hover_text("Remettre en automatique").clicked() {
                                    if let Some(t) = grafcet.transition_mut(tid) {
                                        t.dst_route_x = None;
                                    }
                                }
                            });
                        }

                        ui.separator();
                        if ui.button("🗑 Supprimer cette transition").clicked() {
                            grafcet.remove_transition(tid);
                            self.selected_trans = None;
                            status_out = Some(format!("Transition T{tid} supprimée"));
                        }
                        ui.label(egui::RichText::new("Touche [Suppr] pour effacer").weak().italics().size(10.0));
                    } else {
                        self.selected_trans = None;
                    }
                } else if let Some(sel_id) = self.selected_step {
                    // ── Propriétés d'une étape sélectionnée ───────────────
                    if let Some(step) = grafcet.step_mut(sel_id) {
                        ui.label(egui::RichText::new(format!("Étape E{}", step.id)).strong());
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label("Label :");
                            ui.text_edit_singleline(&mut step.label);
                        });
                        ui.label("Type :");
                        ui.radio_value(&mut step.kind, StepKind::Normal,    "Normal");
                        ui.radio_value(&mut step.kind, StepKind::Initial,   "Initial");
                        ui.radio_value(&mut step.kind, StepKind::MacroStep, "Macro");
                        ui.separator();
                        ui.label("Actions (une par ligne) :");
                        let mut actions_text = step.actions.join("\n");
                        if ui.text_edit_multiline(&mut actions_text).changed() {
                            step.actions = actions_text
                                .lines()
                                .map(|l| l.to_string())
                                .filter(|l| !l.is_empty())
                                .collect();
                        }
                    } else {
                        self.selected_step = None;
                    }

                    ui.separator();
                    ui.label("Transitions liées :");
                    let trans: Vec<(u32, u32, u32, String)> = grafcet
                        .transitions
                        .iter()
                        .filter(|t| t.from_step == sel_id || t.to_step == sel_id)
                        .map(|t| (t.id, t.from_step, t.to_step, t.condition.clone()))
                        .collect();
                    for (tid, from, to, cond) in trans {
                        ui.horizontal(|ui| {
                            if ui.selectable_label(false, format!("T{tid} (E{from}→E{to})")).clicked() {
                                self.selected_trans = Some(tid);
                                self.selected_step = None;
                            }
                            let mut c = cond.clone();
                            if ui.text_edit_singleline(&mut c).changed() {
                                if let Some(t) = grafcet.transition_mut(tid) {
                                    t.condition = c;
                                }
                            }
                        });
                    }
                } else {
                    ui.label("Cliquez sur une étape ou une transition.");
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("• Select + clic → sélectionner\n• [Suppr] → effacer l'élément sélectionné").weak().size(11.0));
                }
            });

        // ── Canvas central ───────────────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(30, 32, 36)))
            .show_inside(ui, |ui| {
                // Auto-fit demandé (après layout automatique ou bouton Centrer)
                if self.pending_fit {
                    self.pending_fit = false;
                    self.fit_to_content(grafcet, ui.available_size());
                }
                let resp = ui.allocate_rect(
                    ui.available_rect_before_wrap(),
                    egui::Sense::click_and_drag(),
                );
                let painter = ui.painter_at(resp.rect);

                self.draw_grid(&painter, resp.rect);

                let hover_trans: Option<u32> = if self.tool == Tool::Select {
                    ctx.pointer_hover_pos()
                        .filter(|&p| resp.rect.contains(p))
                        .and_then(|p| {
                            let cv = egui::Pos2::new(
                                (p.x - resp.rect.min.x - self.offset.x) / self.zoom,
                                (p.y - resp.rect.min.y - self.offset.y) / self.zoom,
                            );
                            hit_transition(cv, grafcet)
                        })
                } else {
                    None
                };
                if hover_trans.is_some() {
                    ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
                }

                draw_steps(&painter, grafcet, self.offset, self.zoom, self.dragging_step);
                draw_links(&painter, grafcet, self.offset, self.zoom);
                draw_transitions(&painter, grafcet, self.offset, self.zoom, self.selected_trans, hover_trans);

                // ── Handles de routage via ui.interact() ─────────────────────
                // Rappel de convention :
                //   draw_*  → coordonnées écran = pos * zoom + offset  (le Painter est absolu)
                //   ui.interact() → coordonnées locales ui = écran + resp.rect.min
                // Donc il faut ajouter resp.rect.min aux positions calculées.
                let origin = resp.rect.min;
                if self.tool == Tool::Select {
                    if let Some(tid) = self.selected_trans {
                        let hdata = grafcet.transition(tid).and_then(|t| {
                            let tp   = t.pos;
                            let ry   = t.route_y;
                            let rx   = t.dst_route_x;
                            let sp   = grafcet.step(t.from_step)?.pos;
                            let dp   = grafcet.step(t.to_step)?.pos;
                            Some((tp, sp, dp, ry, rx))
                        });
                        if let Some((t_pos, src_pos, dst_pos, route_y, dst_route_x)) = hdata {
                            const R:  f32 = 8.0;
                            const HS: f32 = 22.0;  // zone cliquable (px)

                            // ── Handle Y ────────────────────────────────────
                            let auto_y = src_pos[1] + STEP_H / 2.0
                                + crate::gui::canvas::STEP_WICK;
                            let hy_cv  = route_y.unwrap_or(auto_y);
                            let hx_cv  = (src_pos[0] + t_pos[0]) / 2.0;
                            // coordonnées draw (Painter absolu)
                            let hx_draw = hx_cv * self.zoom + self.offset.x;
                            let hy_draw = hy_cv * self.zoom + self.offset.y;
                            // coordonnées ui.interact (locales = draw + origin)
                            let hy_rect = egui::Rect::from_center_size(
                                Pos2::new(hx_draw + origin.x, hy_draw + origin.y),
                                egui::Vec2::splat(HS),
                            );
                            let hy_resp = ui.interact(
                                hy_rect,
                                egui::Id::new("rh_y").with(tid),
                                egui::Sense::drag(),
                            );
                            if hy_resp.dragged() {
                                let dy = hy_resp.drag_delta().y / self.zoom;
                                if let Some(t) = grafcet.transition_mut(tid) {
                                    t.route_y = Some(hy_cv + dy);
                                }
                            }
                            let col_y = if hy_resp.is_pointer_button_down_on() || hy_resp.dragged() {
                                egui::Color32::from_rgb(255, 240, 80)
                            } else if hy_resp.hovered() {
                                egui::Color32::from_rgb(255, 200, 60)
                            } else {
                                egui::Color32::from_rgb(255, 140, 0)
                            };
                            if hy_resp.hovered() {
                                ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
                            }
                            painter.circle_filled(Pos2::new(hx_draw, hy_draw), R, col_y);
                            painter.circle_stroke(
                                Pos2::new(hx_draw, hy_draw), R,
                                egui::Stroke::new(1.5, egui::Color32::WHITE),
                            );
                            painter.text(
                                Pos2::new(hx_draw, hy_draw), egui::Align2::CENTER_CENTER, "↕",
                                egui::FontId::proportional(9.0), egui::Color32::WHITE,
                            );

                            // ── Handle X (boucle en retour) ─────────────────
                            let t_bot_cv = t_pos[1] + crate::gui::canvas::TRANS_WICK;
                            let dy_anc   = dst_pos[1]
                                - (STEP_H / 2.0 + crate::gui::canvas::STEP_WICK);
                            if dy_anc < t_bot_cv {
                                let auto_rx  = t_pos[0].min(dst_pos[0]) - (STEP_W / 2.0 + 25.0);
                                let rx_cv    = dst_route_x.unwrap_or(auto_rx);
                                let mid_y_cv = (t_bot_cv + dy_anc) / 2.0;
                                let rx_draw  = rx_cv    * self.zoom + self.offset.x;
                                let my_draw  = mid_y_cv * self.zoom + self.offset.y;
                                let rx_rect  = egui::Rect::from_center_size(
                                    Pos2::new(rx_draw + origin.x, my_draw + origin.y),
                                    egui::Vec2::splat(HS),
                                );
                                let rx_resp = ui.interact(
                                    rx_rect,
                                    egui::Id::new("rh_x").with(tid),
                                    egui::Sense::drag(),
                                );
                                if rx_resp.dragged() {
                                    let dx = rx_resp.drag_delta().x / self.zoom;
                                    if let Some(t) = grafcet.transition_mut(tid) {
                                        t.dst_route_x = Some(rx_cv + dx);
                                    }
                                }
                                let col_x = if rx_resp.is_pointer_button_down_on() || rx_resp.dragged() {
                                    egui::Color32::from_rgb(60, 255, 235)
                                } else if rx_resp.hovered() {
                                    egui::Color32::from_rgb(60, 240, 220)
                                } else {
                                    egui::Color32::from_rgb(0, 200, 180)
                                };
                                if rx_resp.hovered() {
                                    ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                                }
                                painter.circle_filled(Pos2::new(rx_draw, my_draw), R, col_x);
                                painter.circle_stroke(
                                    Pos2::new(rx_draw, my_draw), R,
                                    egui::Stroke::new(1.5, egui::Color32::WHITE),
                                );
                                painter.text(
                                    Pos2::new(rx_draw, my_draw), egui::Align2::CENTER_CENTER, "↔",
                                    egui::FontId::proportional(9.0), egui::Color32::WHITE,
                                );
                            }
                        }
                    }
                }

                if self.tool == Tool::AddStep {
                    if let Some(mouse) = ctx.pointer_interact_pos() {
                        if resp.rect.contains(mouse) {
                            let cv_x = (mouse.x - resp.rect.min.x - self.offset.x) / self.zoom;
                            let cv_y = (mouse.y - resp.rect.min.y - self.offset.y) / self.zoom;
                            draw_step_ghost(&painter, [cv_x, cv_y], self.offset, self.zoom);
                        }
                    }
                }

                // Ligne de connexion en cours
                if let Some(from_id) = self.conn_from {
                    if let Some(src) = grafcet.step(from_id) {
                        if let Some(mouse) = ctx.pointer_interact_pos() {
                            let sx = src.pos[0] * self.zoom + self.offset.x + resp.rect.min.x;
                            let sy = src.pos[1] * self.zoom + self.offset.y + resp.rect.min.y
                                + STEP_H * self.zoom / 2.0;
                            painter.line_segment(
                                [Pos2::new(sx, sy), mouse],
                                egui::Stroke::new(1.5, egui::Color32::from_rgb(36, 113, 163)),
                            );
                        }
                    }
                }

                let pointer   = ctx.pointer_interact_pos();
                let origin    = resp.rect.min;
                let just_pressed = ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
                let primary_down = ctx.input(|i| i.pointer.button_down(egui::PointerButton::Primary));
                let primary_up   = ctx.input(|i| i.pointer.button_released(egui::PointerButton::Primary));
                let ptr_delta    = ctx.input(|i| i.pointer.delta());

                if resp.dragged_by(egui::PointerButton::Middle) {
                    self.offset += resp.drag_delta();
                }

                if just_pressed && resp.contains_pointer() {
                    if let Some(p) = pointer {
                        let cv = Pos2::new(
                            (p.x - origin.x - self.offset.x) / self.zoom,
                            (p.y - origin.y - self.offset.y) / self.zoom,
                        );

                // ── Hit-test handles de routage (priorité max en mode Select) ─
                        // Les handles utilisent ui.interact() → pas de hit-test manuel ici.
                        let route_grabbed = false;

                        let trans_grabbed = route_grabbed || if self.tool == Tool::Select {
                            if let Some(tid) = hit_transition(cv, grafcet) {
                                self.selected_trans = Some(tid);
                                self.dragging_trans = Some(tid);
                                self.selected_step = None;
                                true
                            } else { false }
                        } else if self.tool == Tool::Delete {
                            if let Some(tid) = hit_transition(cv, grafcet) {
                                grafcet.remove_transition(tid);
                                if self.selected_trans == Some(tid) { self.selected_trans = None; }
                                status_out = Some(format!("Transition T{tid} supprimée"));
                                true
                            } else { false }
                        } else { false };

                        if !trans_grabbed {
                            if let Some(id) = self.hit_step(cv, grafcet) {
                                match self.tool {
                                    Tool::Select => {
                                        self.selected_step = Some(id);
                                        self.dragging_step = Some(id);
                                        if let Some(s) = grafcet.step(id) {
                                            self.drag_offset = Vec2::new(cv.x - s.pos[0], cv.y - s.pos[1]);
                                        }
                                    }
                                    Tool::AddTransition => {
                                        if self.conn_from.is_none() {
                                            self.conn_from = Some(id);
                                        } else if Some(id) != self.conn_from {
                                            let from = self.conn_from.take().unwrap();
                                            grafcet.add_transition(from, id);
                                            status_out = Some(format!("Transition E{from} → E{id} ajoutée"));
                                        }
                                    }
                                    Tool::Delete => {
                                        grafcet.remove_step(id);
                                        if self.selected_step == Some(id) { self.selected_step = None; }
                                        status_out = Some(format!("Étape E{id} supprimée"));
                                    }
                                    _ => {}
                                }
                            } else {
                                match self.tool {
                                    Tool::AddStep => {
                                        let id = grafcet.add_step([cv.x, cv.y]);
                                        self.selected_step = Some(id);
                                        self.dragging_step = Some(id);
                                        self.drag_offset = Vec2::ZERO;
                                        status_out = Some(format!("Étape E{id} créée"));
                                    }
                                    Tool::Select => {
                                        self.selected_step = None;
                                        self.selected_trans = None;
                                        self.conn_from = None;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }

                if primary_down {
                    // Drag de la barre de transition (route_y/x gérés par ui.interact() ci-dessus)
                    if let Some(tid) = self.dragging_trans {
                        if let Some(t) = grafcet.transition_mut(tid) {
                            t.pos[0] += ptr_delta.x / self.zoom;
                            t.pos[1] += ptr_delta.y / self.zoom;
                        }
                    } else if let Some(id) = self.dragging_step {
                        if let Some(p) = pointer {
                            let cv = Pos2::new(
                                (p.x - origin.x - self.offset.x) / self.zoom,
                                (p.y - origin.y - self.offset.y) / self.zoom,
                            );
                            if let Some(s) = grafcet.step_mut(id) {
                                s.pos = [cv.x - self.drag_offset.x, cv.y - self.drag_offset.y];
                            }
                        }
                    }
                }

                if primary_up {
                    self.dragging_step = None;
                    self.dragging_trans = None;
                    self.dragging_route_y = None;
                    self.dragging_route_x = None;
                }

                // Touche Delete : supprime l'élément sélectionné
                if ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
                    if let Some(tid) = self.selected_trans {
                        grafcet.remove_transition(tid);
                        self.selected_trans = None;
                        status_out = Some(format!("Transition T{tid} supprimée"));
                    } else if let Some(sid) = self.selected_step {
                        grafcet.remove_step(sid);
                        self.selected_step = None;
                        status_out = Some(format!("Étape E{sid} supprimée"));
                    }
                }

                let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
                if resp.contains_pointer() && scroll != 0.0 {
                    let factor = if scroll > 0.0 { 1.1_f32 } else { 1.0 / 1.1 };
                    self.zoom = (self.zoom * factor).clamp(0.2, 5.0);
                }
            });

        status_out
    }

    fn hit_step(&self, cv: Pos2, grafcet: &Grafcet) -> Option<u32> {
        let hw = STEP_W / 2.0;
        let hh = STEP_H / 2.0;
        for step in &grafcet.steps {
            if cv.x >= step.pos[0] - hw && cv.x <= step.pos[0] + hw
                && cv.y >= step.pos[1] - hh && cv.y <= step.pos[1] + hh
            {
                return Some(step.id);
            }
        }
        None
    }

    fn draw_grid(&self, painter: &egui::Painter, rect: egui::Rect) {
        let grid_sz = 20.0 * self.zoom;
        let color = egui::Stroke::new(0.5, egui::Color32::from_gray(55));
        let x0 = rect.min.x + (self.offset.x % grid_sz + grid_sz) % grid_sz;
        let y0 = rect.min.y + (self.offset.y % grid_sz + grid_sz) % grid_sz;
        let mut x = x0;
        while x <= rect.max.x {
            painter.line_segment([Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)], color);
            x += grid_sz;
        }
        let mut y = y0;
        while y <= rect.max.y {
            painter.line_segment([Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)], color);
            y += grid_sz;
        }
    }

    fn save(&mut self, grafcet: &Grafcet) -> Option<String> {
        if let Some(ref path) = self.current_path.clone() {
            match crate::persistence::save_json(grafcet, path) {
                Ok(()) => Some(format!("Enregistré : {}", path.file_name()?.to_string_lossy())),
                Err(e) => Some(format!("Erreur : {e}")),
            }
        } else {
            self.save_as(grafcet)
        }
    }

    fn save_as(&mut self, grafcet: &Grafcet) -> Option<String> {
        let path = rfd::FileDialog::new()
            .add_filter("GRAFCET JSON", &["json"])
            .set_file_name("grafcet.json")
            .save_file()?;
        match crate::persistence::save_json(grafcet, &path) {
            Ok(()) => {
                let name = path.file_name()?.to_string_lossy().to_string();
                self.current_path = Some(path);
                Some(format!("Enregistré : {name}"))
            }
            Err(e) => Some(format!("Erreur : {e}")),
        }
    }
}

fn tool_btn(ui: &mut egui::Ui, label: &str, tool: Tool, current: &mut Tool) {
    let active = *current == tool;
    if ui.selectable_label(active, label).clicked() {
        *current = tool;
    }
}
