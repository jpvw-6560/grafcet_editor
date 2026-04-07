use egui::{Key, Modifiers, Pos2, Vec2};

use crate::grafcet::{Grafcet, StepKind};
use crate::gui::canvas::{draw_links, draw_step_ghost, draw_steps, draw_transitions, hit_transition, STEP_H, STEP_W};
use crate::persistence::{load_json, save_json};

/// Outil actif dans l'éditeur
#[derive(Debug, Clone, PartialEq)]
enum Tool {
    Select,
    AddStep,
    AddTransition,
    Delete,
}

impl Default for Tool {
    fn default() -> Self {
        Tool::Select
    }
}

/// État de l'éditeur graphique (eframe App)
pub struct GrafcetEditor {
    grafcet: Grafcet,
    tool: Tool,

    // Pan & zoom
    offset: Vec2,
    zoom: f32,
    last_drag_pos: Option<Pos2>,

    // Drag d'une étape ou d'une transition
    dragging_step: Option<u32>,
    drag_offset: Vec2,
    dragging_trans: Option<u32>,

    // Connexion en cours (AddTransition)
    conn_from: Option<u32>,

    // Propriétés panel : étape sélectionnée
    selected_step: Option<u32>,

    // Popups / feedback
    status_msg: String,

    // Chemin fichier courant
    current_path: Option<std::path::PathBuf>,
}

impl Default for GrafcetEditor {
    fn default() -> Self {
        let mut grafcet = Grafcet::new();
        // Étape initiale par défaut
        let id = grafcet.add_step([400.0, 100.0]);
        if let Some(s) = grafcet.step_mut(id) {
            s.kind = StepKind::Initial;
        }
        Self {
            grafcet,
            tool: Tool::default(),
            offset: Vec2::new(0.0, 0.0),
            zoom: 1.0,
            last_drag_pos: None,
            dragging_step: None,
            drag_offset: Vec2::ZERO,
            dragging_trans: None,
            conn_from: None,
            selected_step: None,
            status_msg: "Bienvenue dans l'éditeur GRAFCET".to_string(),
            current_path: None,
        }
    }
}

impl eframe::App for GrafcetEditor {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // ── Barre de menu ──────────────────────────────────────────────────
        egui::Panel::top("menu_bar").show_inside(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("Fichier", |ui| {
                    if ui.button("Nouveau").clicked() {
                        self.grafcet = Grafcet::new();
                        let id = self.grafcet.add_step([400.0, 100.0]);
                        if let Some(s) = self.grafcet.step_mut(id) {
                            s.kind = StepKind::Initial;
                        }
                        self.current_path = None;
                        self.selected_step = None;
                        self.status_msg = "Nouveau grafcet créé".to_string();
                        ui.close();
                    }
                    if ui.button("Ouvrir…").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("GRAFCET JSON", &["json"])
                            .pick_file()
                        {
                            match load_json(&path) {
                                Ok(g) => {
                                    self.grafcet = g;
                                    self.current_path = Some(path.clone());
                                    self.selected_step = None;
                                    self.status_msg = format!(
                                        "Ouvert : {}",
                                        path.file_name()
                                            .unwrap_or_default()
                                            .to_string_lossy()
                                    );
                                }
                                Err(e) => self.status_msg = format!("Erreur : {e}"),
                            }
                        }
                        ui.close();
                    }
                    if ui.button("Enregistrer").clicked() {
                        self.save();
                        ui.close();
                    }
                    if ui.button("Enregistrer sous…").clicked() {
                        self.save_as();
                        ui.close();
                    }
                });

                ui.separator();

                // Sélection de l'outil
                ui.selectable_value(&mut self.tool, Tool::Select, "↖ Sélect.");
                ui.selectable_value(&mut self.tool, Tool::AddStep, "⬛ Étape");
                ui.selectable_value(&mut self.tool, Tool::AddTransition, "↕ Transition");
                ui.selectable_value(&mut self.tool, Tool::Delete, "🗑 Suppr.");

                // Zoom
                ui.separator();
                if ui.button("🔍+").clicked() {
                    self.zoom = (self.zoom * 1.2).min(5.0);
                }
                if ui.button("🔍-").clicked() {
                    self.zoom = (self.zoom / 1.2).max(0.2);
                }
                if ui.button("100%").clicked() {
                    self.zoom = 1.0;
                }
                if ui.button("Centrer").clicked() {
                    self.offset = Vec2::ZERO;
                }
                if ui.button("🗑 Vider").on_hover_text("Vide le canvas (supprime toutes les étapes et transitions)").clicked() {
                    self.grafcet.steps.clear();
                    self.grafcet.transitions.clear();
                    self.grafcet.next_step_id = 0;
                    self.grafcet.next_trans_id = 0;
                    self.selected_step = None;
                    self.dragging_step = None;
                    self.dragging_trans = None;
                    self.conn_from = None;
                    self.status_msg = "Canvas vidé".to_string();
                }

                // Raccourcis clavier
                if ctx.input(|i| i.key_pressed(Key::Escape)) {
                    self.tool = Tool::Select;
                    self.conn_from = None;
                }
                if ctx.input(|i| {
                    i.modifiers.contains(Modifiers::CTRL) && i.key_pressed(Key::S)
                }) {
                    self.save();
                }
            });
        });

        // ── Panneau de propriétés (droite) ─────────────────────────────────
        egui::Panel::right("props_panel")
            .min_size(200.0)
            .show_inside(ui, |ui| {
                ui.heading("Propriétés");
                ui.separator();
                if let Some(sel_id) = self.selected_step {
                    if let Some(step) = self.grafcet.step_mut(sel_id) {
                        ui.label(format!("Étape #{}", step.id));
                        ui.horizontal(|ui| {
                            ui.label("Label :");
                            ui.text_edit_singleline(&mut step.label);
                        });
                        ui.label("Type :");
                        ui.radio_value(&mut step.kind, StepKind::Normal, "Normal");
                        ui.radio_value(&mut step.kind, StepKind::Initial, "Initial");
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
                } else {
                    ui.label("Cliquez sur une étape pour la sélectionner.");
                }

                // Propriétés transition sélectionnée (à étendre en phase 2)
                ui.separator();
                ui.label("Transitions :");
                // Afficher condition de toutes les transitions
                // pour la sélection courante
                if let Some(sid) = self.selected_step {
                    let trans: Vec<(u32, String)> = self
                        .grafcet
                        .transitions
                        .iter()
                        .filter(|t| t.from_step == sid || t.to_step == sid)
                        .map(|t| (t.id, t.condition.clone()))
                        .collect();
                    for (tid, cond) in trans {
                        ui.horizontal(|ui| {
                            ui.label(format!("T{tid} :"));
                            let mut c = cond.clone();
                            if ui.text_edit_singleline(&mut c).changed() {
                                if let Some(t) = self.grafcet.transition_mut(tid) {
                                    t.condition = c;
                                }
                            }
                        });
                    }
                }
            });

        // ── Barre de statut (bas) ──────────────────────────────────────────
        egui::Panel::bottom("status_bar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                let tool_str = match self.tool {
                    Tool::Select => "Outil : Sélect.",
                    Tool::AddStep => "Outil : Ajouter étape — cliquez sur le canvas",
                    Tool::AddTransition => {
                        if self.conn_from.is_some() {
                            "Outil : Transition — cliquez sur l'étape destination"
                        } else {
                            "Outil : Transition — cliquez sur l'étape source"
                        }
                    }
                    Tool::Delete => "Outil : Supprimer — cliquez sur un élément",
                };
                ui.label(tool_str);
                ui.separator();
                ui.label(&self.status_msg);
                ui.separator();
                ui.label(format!("Zoom : {:.0}%", self.zoom * 100.0));
                ui.label(format!(
                    " | Étapes : {}  Transitions : {}",
                    self.grafcet.steps.len(),
                    self.grafcet.transitions.len()
                ));
            });
        });

        // ── Canvas principal ───────────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(egui::Color32::from_rgb(30, 32, 36)))
            .show_inside(ui, |ui| {
                let resp = ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::click_and_drag());
                let painter = ui.painter_at(resp.rect);

                // Grille de fond
                self.draw_grid(&painter, resp.rect);

                // Calcul du survol de transition pour feedback visuel
                let hover_trans: Option<u32> = if self.tool == Tool::Select {
                    ctx.pointer_hover_pos()
                        .filter(|&p| resp.rect.contains(p))
                        .and_then(|p| {
                            let cv = egui::Pos2::new(
                                (p.x - resp.rect.min.x - self.offset.x) / self.zoom,
                                (p.y - resp.rect.min.y - self.offset.y) / self.zoom,
                            );
                            hit_transition(cv, &self.grafcet)
                        })
                } else {
                    None
                };
                if hover_trans.is_some() {
                    ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
                }

                // Passe 1 : corps des étapes (fond)
                draw_steps(&painter, &self.grafcet, self.offset, self.zoom, self.dragging_step);
                // Passe 2 : segments de liaison (par-dessus les mèches d'étapes)
                draw_links(&painter, &self.grafcet, self.offset, self.zoom);
                // Passe 3 : barres de transition (tout au-dessus)
                draw_transitions(&painter, &self.grafcet, self.offset, self.zoom, self.dragging_trans, hover_trans);

                // Ghost step : preview de placement si outil AddStep actif
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
                    if let Some(src) = self.grafcet.step(from_id) {
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

                // Gestion clics / drag
                let pointer = ctx.pointer_interact_pos();
                let origin = resp.rect.min;

                // Scroll de pan (bouton central)
                if resp.dragged_by(egui::PointerButton::Middle) {
                    self.offset += resp.drag_delta();
                }

                // Détection appui primaire (fonctionne pour clic simple ET clic+drag)
                let just_pressed = ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
                if just_pressed && resp.contains_pointer() {
                    if let Some(p) = pointer {
                        // Coordonnées canvas (logique) — même espace que step.pos
                        let cv = Pos2::new(
                            (p.x - origin.x - self.offset.x) / self.zoom,
                            (p.y - origin.y - self.offset.y) / self.zoom,
                        );

                        // En mode Select : priorité aux transitions (zone de clic petite)
                        let trans_grabbed = if self.tool == Tool::Select {
                            if let Some(tid) = hit_transition(cv, &self.grafcet) {
                                self.dragging_trans = Some(tid);
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        };

                        if !trans_grabbed {
                        if let Some(id) = self.hit_step(cv) {
                            match self.tool {
                                Tool::Select => {
                                    self.selected_step = Some(id);
                                    self.dragging_step = Some(id);
                                    if let Some(s) = self.grafcet.step(id) {
                                        self.drag_offset = Vec2::new(
                                            cv.x - s.pos[0],
                                            cv.y - s.pos[1],
                                        );
                                    }
                                }
                                Tool::AddTransition => {
                                    if self.conn_from.is_none() {
                                        self.conn_from = Some(id);
                                        self.status_msg =
                                            format!("Transition depuis E{id} — cliquez la destination");
                                    } else if Some(id) != self.conn_from {
                                        let from = self.conn_from.take().unwrap();
                                        self.grafcet.add_transition(from, id);
                                        self.status_msg =
                                            format!("Transition E{from} → E{id} ajoutée");
                                    }
                                }
                                Tool::Delete => {
                                    self.grafcet.remove_step(id);
                                    if self.selected_step == Some(id) {
                                        self.selected_step = None;
                                    }
                                    self.status_msg = format!("Étape E{id} supprimée");
                                }
                                _ => {}
                            }
                        } else {
                            // Appui sur zone vide
                            match self.tool {
                                Tool::AddStep => {
                                    let id = self.grafcet.add_step([cv.x, cv.y]);
                                    self.selected_step = Some(id);
                                    // Démarre le drag immédiatement : glisser pour repositionner
                                    self.dragging_step = Some(id);
                                    self.drag_offset = Vec2::ZERO;
                                    self.status_msg = format!("Étape E{id} créée");
                                }
                                Tool::Select => {
                                    self.selected_step = None;
                                    self.conn_from = None;
                                }
                                _ => {}
                            }
                        }
                        } // end !trans_grabbed
                    }
                }

                // Drag d'une étape ou d'une transition
                // On utilise pointer.delta() (raw input) pour la transition — plus fiable
                // que resp.dragged_by() qui peut rater si le claim n'est pas tenu.
                let primary_down = ctx.input(|i| i.pointer.button_down(egui::PointerButton::Primary));
                let primary_released = ctx.input(|i| i.pointer.button_released(egui::PointerButton::Primary));
                let ptr_delta = ctx.input(|i| i.pointer.delta());

                if primary_down {
                    if let Some(tid) = self.dragging_trans {
                        if let Some(t) = self.grafcet.transition_mut(tid) {
                            // Drag 2D : X et Y
                            t.pos[0] += ptr_delta.x / self.zoom;
                            t.pos[1] += ptr_delta.y / self.zoom;
                        }
                    } else if let Some(id) = self.dragging_step {
                        if let Some(p) = pointer {
                            let cv = Pos2::new(
                                (p.x - origin.x - self.offset.x) / self.zoom,
                                (p.y - origin.y - self.offset.y) / self.zoom,
                            );
                            if let Some(s) = self.grafcet.step_mut(id) {
                                s.pos = [
                                    cv.x - self.drag_offset.x,
                                    cv.y - self.drag_offset.y,
                                ];
                            }
                        }
                    } else if resp.dragged_by(egui::PointerButton::Middle) {
                        self.offset += resp.drag_delta();
                    }
                }

                if primary_released {
                    self.dragging_step = None;
                    self.dragging_trans = None;
                }

                // Zoom molette
                let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
                if resp.contains_pointer() && scroll != 0.0 {
                    let factor = if scroll > 0.0 { 1.1_f32 } else { 1.0 / 1.1 };
                    self.zoom = (self.zoom * factor).clamp(0.2, 5.0);
                }
            });

        ctx.request_repaint();
    }
}

impl GrafcetEditor {
    /// Trouve l'étape sous le point canvas donné (coordonnées canvas).
    fn hit_step(&self, cv: Pos2) -> Option<u32> {
        let dummy_offset = Vec2::ZERO;
        for step in &self.grafcet.steps {
            let hw = STEP_W / 2.0;
            let hh = STEP_H / 2.0;
            if cv.x >= step.pos[0] - hw
                && cv.x <= step.pos[0] + hw
                && cv.y >= step.pos[1] - hh
                && cv.y <= step.pos[1] + hh
            {
                let _ = dummy_offset; // silence warning
                return Some(step.id);
            }
        }
        None
    }

    /// Dessine la grille de fond du canvas.
    fn draw_grid(&self, painter: &egui::Painter, rect: egui::Rect) {
        let grid_sz = 20.0 * self.zoom;
        let color = egui::Color32::from_gray(55);
        let stroke = egui::Stroke::new(0.5, color);

        let x0 = rect.min.x + (self.offset.x % grid_sz + grid_sz) % grid_sz;
        let y0 = rect.min.y + (self.offset.y % grid_sz + grid_sz) % grid_sz;

        let mut x = x0;
        while x <= rect.max.x {
            painter.line_segment([Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)], stroke);
            x += grid_sz;
        }
        let mut y = y0;
        while y <= rect.max.y {
            painter.line_segment([Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)], stroke);
            y += grid_sz;
        }
    }

    fn save(&mut self) {
        if let Some(ref path) = self.current_path.clone() {
            match save_json(&self.grafcet, path) {
                Ok(()) => {
                    self.status_msg = format!(
                        "Enregistré : {}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    )
                }
                Err(e) => self.status_msg = format!("Erreur : {e}"),
            }
        } else {
            self.save_as();
        }
    }

    fn save_as(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("GRAFCET JSON", &["json"])
            .set_file_name("grafcet.json")
            .save_file()
        {
            match save_json(&self.grafcet, &path) {
                Ok(()) => {
                    self.status_msg = format!(
                        "Enregistré : {}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    );
                    self.current_path = Some(path);
                }
                Err(e) => self.status_msg = format!("Erreur : {e}"),
            }
        }
    }
}
