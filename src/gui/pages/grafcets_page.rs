// gui/pages/grafcets_page.rs — Page « Grafcets »
//
// Onglets : GS | GC | GPN | + extras
// Chaque onglet embarque le canvas GRAFCET existant (editor.rs).
// Un bouton « + Nouveau grafcet » ajoute un onglet supplémentaire.
// Un bouton « ⚙ Depuis GEMMA » génère automatiquement les grafcets
// correspondant aux circuits fermés du GEMMA.

use egui::Vec2;

use crate::grafcet::{Grafcet, StepKind};
use crate::gemma::Gemma;
use crate::gui::canvas_editor::CanvasEditor;
use crate::project::{NamedGrafcet, Project};

pub struct GrafcetsPage {
    /// Index de l'onglet affiché
    active_tab: usize,
    /// Décalage de la fenêtre d'onglets visibles (navigation ◀ / ▶)
    tab_offset: usize,
    /// Un CanvasEditor par grafcet (indexé pareil que project.grafcets)
    editors: Vec<CanvasEditor>,
    /// Tampon pour le nom d'un nouveau grafcet
    new_grafcet_name: String,
    show_add_popup: bool,
    /// Signale à app.rs de (re)générer tous les grafcets GEMMA au prochain frame
    pub needs_full_generate: bool,
    /// Index du grafcet à supprimer (traité hors closure de panel)
    pending_delete: Option<usize>,
    /// Pour chaque onglet : vue partagée JSON + canvas active
    graphic_active: Vec<bool>,
    /// Pour chaque onglet : canvas maximisé (JSON caché)
    canvas_only: Vec<bool>,
}

impl Default for GrafcetsPage {
    fn default() -> Self {
        Self {
            active_tab: 0,
            tab_offset: 0,
            editors: Vec::new(),
            new_grafcet_name: String::new(),
            show_add_popup: false,
            needs_full_generate: false,
            pending_delete: None,
            graphic_active: Vec::new(),
            canvas_only: Vec::new(),
        }
    }
}

impl GrafcetsPage {
    /// Réinitialise les éditeurs quand le projet change.
    pub fn reset(&mut self) {
        self.editors.clear();
        self.graphic_active.clear();
        self.canvas_only.clear();
        self.active_tab = 0;
        self.tab_offset = 0;
    }

    pub fn show(&mut self, ui: &mut egui::Ui, project: &mut Project) -> Option<String> {
        let mut status_out: Option<String> = None;

        // Synchronise le nombre d'éditeurs avec le projet
        while self.editors.len() < project.grafcets.len() {
            self.editors.push(CanvasEditor::default());
        }
        while self.graphic_active.len() < project.grafcets.len() {
            self.graphic_active.push(false);
        }
        while self.canvas_only.len() < project.grafcets.len() {
            self.canvas_only.push(false);
        }

        // ── Ligne 1 : boutons d'action ─────────────────────────────────────
        egui::Panel::top("grafcets_actions")
            .exact_size(34.0)
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(20, 30, 40)).inner_margin(egui::Margin::same(4)))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    // Bouton + nouveau
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("＋ Nouveau").size(12.0).color(egui::Color32::WHITE),
                            )
                            .fill(egui::Color32::from_rgb(39, 60, 78))
                            .min_size(Vec2::new(85.0, 26.0)),
                        )
                        .on_hover_text("Ajouter un grafcet utilisateur")
                        .clicked()
                    {
                        self.show_add_popup = true;
                    }

                    ui.add_space(6.0);

                    // Bouton ⚙ Depuis GEMMA
                    {
                        let gemma_ready = !project.gemma.states.is_empty();
                        let btn_color = if gemma_ready {
                            egui::Color32::from_rgb(90, 50, 130)
                        } else {
                            egui::Color32::from_rgb(55, 45, 65)
                        };
                        let text_color = if gemma_ready {
                            egui::Color32::WHITE
                        } else {
                            egui::Color32::from_rgb(110, 100, 120)
                        };
                        let resp = ui.add_enabled(
                            gemma_ready,
                            egui::Button::new(
                                egui::RichText::new("⚙ Depuis GEMMA")
                                    .size(11.0)
                                    .color(text_color),
                            )
                            .fill(btn_color)
                            .min_size(Vec2::new(118.0, 26.0)),
                        );
                        let resp = resp.on_hover_text(if gemma_ready {
                            "Regénérer TOUS les grafcets GEMMA (GS, GC, GPN, G_*, GG) — grafcets utilisateur conservés"
                        } else {
                            "GEMMA non encore généré — remplissez le questionnaire d'abord"
                        });
                        if resp.clicked() {
                            self.needs_full_generate = true;
                        }
                    }

                    // Bouton supprimer l'onglet actif (à droite)
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !project.grafcets.is_empty() {
                            let del = ui.add(
                                egui::Button::new(
                                    egui::RichText::new("🗑 Supprimer").size(11.0).color(egui::Color32::WHITE),
                                )
                                .fill(egui::Color32::from_rgb(150, 40, 40))
                                .min_size(Vec2::new(90.0, 26.0)),
                            );
                            if del.clicked() {
                                self.pending_delete = Some(
                                    self.active_tab.min(project.grafcets.len().saturating_sub(1)),
                                );
                            }
                        }
                    });
                });
            });

        // ── Ligne 2 : onglets avec navigation ◀ / ▶ ──────────────────────
        const TABS_PAGE: usize = 8; // nombre max d'onglets visibles à la fois
        let total_tabs = project.grafcets.len();
        // Auto-scroll : si l'onglet actif sort de la fenêtre, ajuster l'offset
        if self.active_tab < self.tab_offset {
            self.tab_offset = self.active_tab;
        } else if total_tabs > 0 && self.active_tab >= self.tab_offset + TABS_PAGE {
            self.tab_offset = self.active_tab.saturating_sub(TABS_PAGE - 1);
        }
        // Clamp l'offset
        if total_tabs <= TABS_PAGE {
            self.tab_offset = 0;
        } else {
            self.tab_offset = self.tab_offset.min(total_tabs - TABS_PAGE);
        }
        let show_left  = self.tab_offset > 0;
        let show_right = total_tabs > TABS_PAGE && self.tab_offset + TABS_PAGE < total_tabs;
        let mut inc_offset: i32 = 0;

        egui::Panel::top("grafcets_tabs")
            .exact_size(34.0)
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(26, 37, 47)).inner_margin(egui::Margin::same(3)))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    // Flèche gauche
                    if show_left {
                        if ui.add(
                            egui::Button::new(egui::RichText::new("◀").size(13.0).color(egui::Color32::from_rgb(170, 200, 230)))
                                .fill(egui::Color32::from_rgb(26, 37, 47))
                                .min_size(Vec2::new(22.0, 26.0)),
                        ).on_hover_text("Onglets précédents").clicked() {
                            inc_offset = -1;
                        }
                        ui.add_space(2.0);
                    }

                    // Onglets visibles
                    let first = self.tab_offset;
                    let last  = (self.tab_offset + TABS_PAGE).min(total_tabs);
                    for i in first..last {
                        if let Some(ng) = project.grafcets.get(i) {
                            let active = self.active_tab == i;
                            let (bg, fg) = if active {
                                (egui::Color32::from_rgb(41, 128, 185), egui::Color32::WHITE)
                            } else {
                                (egui::Color32::from_rgb(26, 37, 47), egui::Color32::from_rgb(170, 190, 210))
                            };
                            let display = ng.short_name.as_deref().unwrap_or(&ng.name);
                            let btn = egui::Button::new(
                                egui::RichText::new(display).size(12.0).color(fg),
                            )
                            .fill(bg)
                            .min_size(Vec2::new(55.0, 26.0));
                            let resp = ui.add(btn);
                            let resp = if let Some(desc) = ng.description.as_deref() {
                                resp.on_hover_text(desc)
                            } else if ng.short_name.is_some() {
                                resp.on_hover_text(&ng.name)
                            } else {
                                resp
                            };
                            if resp.clicked() {
                                self.active_tab = i;
                            }
                            ui.add_space(2.0);
                        }
                    }

                    // Flèche droite
                    if show_right {
                        ui.add_space(2.0);
                        if ui.add(
                            egui::Button::new(egui::RichText::new("▶").size(13.0).color(egui::Color32::from_rgb(170, 200, 230)))
                                .fill(egui::Color32::from_rgb(26, 37, 47))
                                .min_size(Vec2::new(22.0, 26.0)),
                        ).on_hover_text("Onglets suivants").clicked() {
                            inc_offset = 1;
                        }
                    }
                });
            });
        if inc_offset != 0 {
            let new_off = self.tab_offset as i32 + inc_offset;
            self.tab_offset = new_off.clamp(0, (total_tabs.saturating_sub(TABS_PAGE)) as i32) as usize;
        }

        // ── Popup nouveau grafcet ──────────────────────────────────────────
        if self.show_add_popup {
            egui::Window::new("Nouveau grafcet")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ui.ctx(), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Nom :");
                        ui.text_edit_singleline(&mut self.new_grafcet_name);
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Créer").clicked() {
                            let name = self.new_grafcet_name.trim().to_string();
                            if !name.is_empty() {
                                project.add_grafcet(name.clone());
                                self.editors.push(CanvasEditor::default());
                                self.active_tab = project.grafcets.len() - 1;
                                status_out = Some(format!("Grafcet « {name} » créé"));
                            }
                            self.new_grafcet_name.clear();
                            self.show_add_popup = false;
                        }
                        if ui.button("Annuler").clicked() {
                            self.show_add_popup = false;
                        }
                    });
                });
        }

        // ── Suppression différée ───────────────────────────────────────────
        if let Some(del_idx) = self.pending_delete.take() {
            if del_idx < project.grafcets.len() {
                let name = project.grafcets[del_idx].name.clone();
                project.grafcets.remove(del_idx);
                if del_idx < self.editors.len()      { self.editors.remove(del_idx); }
                if del_idx < self.graphic_active.len() { self.graphic_active.remove(del_idx); }
                if del_idx < self.canvas_only.len()  { self.canvas_only.remove(del_idx); }
                self.active_tab = self.active_tab.saturating_sub(if del_idx <= self.active_tab { 1 } else { 0 });
                status_out = Some(format!("Grafcet « {name} » supprimé"));
            }
        }

        // ── Contenu de l'onglet actif ──────────────────────────────────────
        let idx = self.active_tab.min(project.grafcets.len().saturating_sub(1));
        if let Some(ng) = project.grafcets.get_mut(idx) {
            let is_split  = self.graphic_active.get(idx).copied().unwrap_or(false);
            let is_canvas_only = self.canvas_only.get(idx).copied().unwrap_or(false);

            if ng.generated && is_split && is_canvas_only {
                // ── Canvas seul (maximisé, JSON caché) ─────────────────────
                let mut restore = false;
                egui::Panel::top(egui::Id::new("canvas_restore_bar").with(idx))
                    .exact_size(26.0)
                    .frame(egui::Frame::new()
                        .fill(egui::Color32::from_rgb(20, 30, 40))
                        .inner_margin(egui::Margin::same(3)))
                    .show_inside(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui.add(egui::Button::new(
                                    egui::RichText::new("◧ Vue partagée JSON+Canvas").size(11.0)
                                        .color(egui::Color32::WHITE))
                                .fill(egui::Color32::from_rgb(35, 60, 80))
                                .min_size(Vec2::new(0.0, 20.0)))
                                .on_hover_text("Restaurer le panneau JSON")
                                .clicked()
                            { restore = true; }
                        });
                    });
                if restore { if let Some(a) = self.canvas_only.get_mut(idx) { *a = false; } }
                if let Some(editor) = self.editors.get_mut(idx) {
                    if let Some(msg) = editor.show(ui, &mut ng.grafcet) {
                        status_out = Some(msg);
                    }
                }

            } else if ng.generated && is_split {
                // ── Vue partagée : JSON à gauche | Canvas à droite ────────
                let buf = grafcet_summary(&ng.grafcet);
                let mut close_graphic  = false;
                let mut maximize_canvas = false;

                egui::Panel::left(egui::Id::new("json_split").with(idx))
                    .resizable(true)
                    .default_width(320.0)
                    .frame(
                        egui::Frame::new()
                            .fill(egui::Color32::from_rgb(14, 20, 28))
                            .inner_margin(egui::Margin::same(10)),
                    )
                    .show_inside(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("JSON — depuis GEMMA")
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(120, 170, 220)),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.add(egui::Button::new(
                                            egui::RichText::new("⛶ Agrandir canvas").size(11.0))
                                            .fill(egui::Color32::from_rgb(35, 60, 80)))
                                        .on_hover_text("Masquer le JSON et maximiser le canvas")
                                        .clicked()
                                    { maximize_canvas = true; }
                                    ui.add_space(4.0);
                                    if ui.add(egui::Button::new(
                                            egui::RichText::new("◀ JSON seul").size(11.0)))
                                        .on_hover_text("Fermer la vue graphique")
                                        .clicked()
                                    { close_graphic = true; }
                                },
                            );
                        });
                        ui.add_space(4.0);
                        let mut b = buf;
                        egui::ScrollArea::both()
                            .auto_shrink([false; 2])
                            .show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut b)
                                        .code_editor()
                                        .desired_width(f32::INFINITY)
                                        .interactive(false),
                                );
                            });
                    });

                if close_graphic   { if let Some(a) = self.graphic_active.get_mut(idx) { *a = false; } }
                if maximize_canvas { if let Some(a) = self.canvas_only.get_mut(idx)    { *a = true; } }

                // Canvas (panneau central restant)
                if let Some(editor) = self.editors.get_mut(idx) {
                    if let Some(msg) = editor.show(ui, &mut ng.grafcet) {
                        status_out = Some(msg);
                    }
                }

            } else if ng.generated {
                // ── JSON seul (avant génération graphique) ────────────────
                let buf = grafcet_summary(&ng.grafcet);
                let mut open_canvas = false;
                let mut do_layout = false;
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(14, 20, 28))
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Grafcet généré depuis le GEMMA")
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(120, 170, 220)),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .button(egui::RichText::new("→ Ouvrir dans le canvas").size(11.0))
                                        .on_hover_text("Bascule vers l'éditeur graphique (sans mise en page auto)")
                                        .clicked()
                                    {
                                        open_canvas = true;
                                    }
                                    ui.add_space(6.0);
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                egui::RichText::new("🎨 Générer graphique")
                                                    .size(11.0)
                                                    .color(egui::Color32::WHITE),
                                            )
                                            .fill(egui::Color32::from_rgb(30, 100, 60))
                                            .min_size(Vec2::new(145.0, 24.0)),
                                        )
                                        .on_hover_text("Calcule le placement BFS et affiche JSON + canvas côte à côte")
                                        .clicked()
                                    {
                                        do_layout = true;
                                    }
                                },
                            );
                        });
                        ui.add_space(6.0);
                        let mut b = buf;
                        egui::ScrollArea::both()
                            .auto_shrink([false; 2])
                            .show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut b)
                                        .code_editor()
                                        .desired_width(f32::INFINITY)
                                        .interactive(false),
                                );
                            });
                    });
                if do_layout {
                    auto_layout(&mut ng.grafcet);
                    if let Some(a) = self.graphic_active.get_mut(idx) { *a = true; }
                    if let Some(ed) = self.editors.get_mut(idx) { ed.pending_fit = true; }
                    status_out = Some(format!("Layout généré : {} étapes placées", ng.grafcet.steps.len()));
                } else if open_canvas {
                    ng.generated = false;
                }

            } else if let Some(editor) = self.editors.get_mut(idx) {
                if let Some(msg) = editor.show(ui, &mut ng.grafcet) {
                    status_out = Some(msg);
                }
            }
        } else {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new("Aucun grafcet dans ce projet.\nCliquez sur ＋ pour en créer un.")
                        .size(14.0)
                        .color(egui::Color32::from_rgb(100, 120, 140)),
                );
            });
        }

        status_out
    }
}

// ── Nommage des circuits ───────────────────────────────────────────────────────

/// Retourne un nom court symbolique pour l'onglet, ex : "A1→F1→A2".
/// Pour les circuits > 4 états, affiche le premier et le dernier : "A1→…→F1".
pub fn circuit_short_name(circuit: &[String]) -> String {
    let n = circuit.len();
    if n <= 4 {
        circuit.join("→")
    } else {
        format!("{}→…→{}", circuit[0], circuit[n - 1])
    }
}

/// Retourne un nom descriptif pour un circuit GEMMA,
/// selon les correspondances définies dans circuits_fermes_possibles.md.
pub fn circuit_name(circuit: &[String]) -> String {
    let s: std::collections::HashSet<&str> = circuit.iter().map(|s| s.as_str()).collect();
    let has = |id: &str| s.contains(id);
    let n = circuit.len();

    if      n == 2 && has("A1") && has("F1")                               { "Production continue".into() }
    else if n == 3 && has("A1") && has("F1") && has("A2")                  { "Cycle principal de production".into() }
    else if n == 3 && has("A1") && has("F1") && has("F3")                  { "Clôture de cycle".into() }
    else if n == 3 && has("F1") && has("A3") && has("A4")                  { "Arrêt immédiat sécurisé".into() }
    else if n == 2 && has("F1") && has("D3")                               { "Défaut mineur".into() }
    else if n == 2 && has("A7") && has("F6")                               { "Mode test".into() }
    else if n == 2 && has("A1") && has("F4")                               { "Vérification libre".into() }
    else if n == 2 && has("A1") && has("F5")                               { "Vérification séquentielle".into() }
    else if n == 4 && has("A5") && has("A7") && has("A4") && has("F1")    { "Mode réglage via A4".into() }
    else if n == 4 && has("A1") && has("F2") && has("F1") && has("A2")    { "Préparation machine".into() }
    else if has("F1") && has("D1") && has("D2") && has("A5") && has("A6") { "Gestion des défauts".into() }
    else if has("A5") && has("A7") && has("A6") && has("A1") && has("F1") { "Mode réglage via A6".into() }
    else {
        // Fallback : catégorie + chemin lisible
        let prefix = if circuit.iter().any(|s| s.starts_with('D')) {
            "Défaut"
        } else if circuit.iter().any(|s| matches!(s.as_str(), "A3" | "A4")) {
            "Arrêt"
        } else if circuit.iter().any(|s| s.starts_with('F')) {
            "Fonctionnement"
        } else {
            "Cycle"
        };
        format!("{prefix}: {}", circuit.join(" → "))
    }
}

// ── Conversion circuit GEMMA → Grafcet ────────────────────────────────────────

/// Convertit un circuit GEMMA (liste ordonnée de state_id, boucle implicite)
/// en un `Grafcet` avec étapes disposées verticalement.
/// - La première étape est marquée `Initial`.
/// - La condition de chaque transition est issue du GEMMA ; "1" si absente.
pub fn circuit_to_grafcet(gemma: &Gemma, circuit: &[String]) -> Grafcet {
    let mut g = Grafcet::new();
    let n = circuit.len();
    if n == 0 {
        return g;
    }

    // Création des étapes (disposition verticale, 120 px entre chaque)
    for (i, state_id) in circuit.iter().enumerate() {
        let pos = [400.0, 80.0 + i as f32 * 120.0];
        let step_id = g.add_step(pos);
        if let Some(step) = g.step_mut(step_id) {
            step.label = state_id.clone();
            if i == 0 {
                step.kind = StepKind::Initial;
            }
        }
    }

    // Création des transitions (y compris la boucle de fermeture n-1 → 0)
    for i in 0..n {
        let from_state = &circuit[i];
        let to_state = &circuit[(i + 1) % n];
        let cond = gemma
            .transitions
            .iter()
            .find(|t| t.from == *from_state && t.to == *to_state)
            .map(|t| t.condition.to_display())
            .unwrap_or_else(|| "1".to_string());

        let from_id = g.steps[i].id;
        let to_id = g.steps[(i + 1) % n].id;
        let trans_id = g.add_transition(from_id, to_id);
        if let Some(t) = g.transitions.iter_mut().find(|t| t.id == trans_id) {
            t.condition = cond;
        }
    }

    g
}

// ── Résumé lisible d'un Grafcet ───────────────────────────────────────────────

/// Construit une chaîne lisible pour la vue « généré » :
/// - ÉTAPES : label (initial ?), actions
/// - TRANSITIONS : de → vers  [condition]
fn grafcet_summary(g: &Grafcet) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    // ── Étapes ──
    let _ = writeln!(out, "═══ ÉTAPES ══════════════════════════════");
    for step in &g.steps {
        let init = if step.kind == StepKind::Initial { "  ◉ initiale" } else { "" };
        let _ = writeln!(out, "  [{}]  {}{}", step.id, step.label, init);
        for action in &step.actions {
            let _ = writeln!(out, "         → {action}");
        }
    }

    let _ = writeln!(out);
    let _ = writeln!(out, "═══ TRANSITIONS ═════════════════════════");
    for tr in &g.transitions {
        let from_label = g.step(tr.from_step)
            .map(|s| s.label.as_str())
            .unwrap_or("?");
        let to_label = g.step(tr.to_step)
            .map(|s| s.label.as_str())
            .unwrap_or("?");
        let _ = writeln!(
            out,
            "  {from_label}->{to_label} :[{}]",
            tr.condition
        );
    }

    out
}

// ── Layout automatique (BFS) ──────────────────────────────────────────────────

/// Positionne automatiquement les étapes et les barres de transition d'un grafcet
/// à partir d'un parcours topologique (DFS back-edges + BFS sur DAG) depuis l'étape initiale.
///
/// Règles de routage back-edge :
///   - chaque boucle de retour reçoit une route_x unique (échelonnée vers la gauche)
///   - pas de flèche sur la ligne latérale (notation GRAFCET standard)
pub fn auto_layout(grafcet: &mut Grafcet) {
    use std::collections::{HashMap, HashSet, VecDeque};
    use crate::gui::canvas::{STEP_H, STEP_W, STEP_WICK, TRANS_WICK};

    if grafcet.steps.is_empty() { return; }

    const X_CENTER: f32 = 400.0;
    const X_GAP: f32    = 180.0;
    const Y_START: f32  = 100.0;
    const LOOP_X_BASE_OFFSET: f32 = 80.0;  // recul depuis le step le plus à gauche
    const LOOP_X_STEP: f32        = 22.0;  // écartement entre routes de retour successives
    let y_step       = STEP_H / 2.0 + STEP_WICK + TRANS_WICK + TRANS_WICK + STEP_WICK + STEP_H / 2.0;
    let bar_y_offset = STEP_H / 2.0 + STEP_WICK + TRANS_WICK;

    // ── 1. Étape initiale ──────────────────────────────────────────────────
    let start_id = grafcet.steps.iter()
        .find(|s| s.kind == StepKind::Initial)
        .or_else(|| grafcet.steps.first())
        .map(|s| s.id)
        .unwrap();

    // ── 2. DFS pour détecter les back-edges (arêtes de retour → cycles) ───
    let adj: HashMap<u32, Vec<(u32, u32)>> = {   // from → [(to, tid)]
        let mut m: HashMap<u32, Vec<(u32, u32)>> = HashMap::new();
        for t in &grafcet.transitions {
            m.entry(t.from_step).or_default().push((t.to_step, t.id));
        }
        m
    };

    let mut back_edge_tids: HashSet<u32> = HashSet::new();
    {
        let mut color: HashMap<u32, u8> = HashMap::new(); // 0=white,1=grey,2=black
        let mut stk: Vec<(u32, usize)> = vec![(start_id, 0)];
        // Initialise blancs
        for s in &grafcet.steps { color.insert(s.id, 0); }

        while let Some((node, child_idx)) = stk.last_mut() {
            let node = *node;
            if color[&node] == 0 {
                *color.get_mut(&node).unwrap() = 1;
            }
            let children = adj.get(&node).map(|v| v.as_slice()).unwrap_or(&[]);
            if *child_idx >= children.len() {
                *color.get_mut(&node).unwrap() = 2;
                stk.pop();
            } else {
                let (to, tid) = children[*child_idx];
                *child_idx += 1;
                let c = *color.get(&to).unwrap_or(&2);
                if c == 1 {
                    back_edge_tids.insert(tid); // back-edge détectée
                } else if c == 0 {
                    stk.push((to, 0));
                }
            }
        }
        // Étapes non rejointes depuis start
        for s in &grafcet.steps {
            if *color.get(&s.id).unwrap_or(&0) == 0 {
                let mut stk2: Vec<(u32, usize)> = vec![(s.id, 0)];
                while let Some((node, child_idx)) = stk2.last_mut() {
                    let node = *node;
                    if color[&node] == 0 { *color.get_mut(&node).unwrap() = 1; }
                    let children = adj.get(&node).map(|v| v.as_slice()).unwrap_or(&[]);
                    if *child_idx >= children.len() {
                        *color.get_mut(&node).unwrap() = 2;
                        stk2.pop();
                    } else {
                        let (to, tid) = children[*child_idx];
                        *child_idx += 1;
                        let c = *color.get(&to).unwrap_or(&2);
                        if c == 1 { back_edge_tids.insert(tid); }
                        else if c == 0 { stk2.push((to, 0)); }
                    }
                }
            }
        }
    }

    // ── 3. BFS sur DAG (sans back-edges) → niveau de chaque étape ─────────
    let fwd_adj: HashMap<u32, Vec<u32>> = {
        let mut m: HashMap<u32, Vec<u32>> = HashMap::new();
        for t in &grafcet.transitions {
            if !back_edge_tids.contains(&t.id) {
                m.entry(t.from_step).or_default().push(t.to_step);
            }
        }
        m
    };

    let mut levels: HashMap<u32, usize> = HashMap::new();
    let mut queue: VecDeque<(u32, usize)> = VecDeque::new();
    queue.push_back((start_id, 0));
    while let Some((sid, lv)) = queue.pop_front() {
        if levels.contains_key(&sid) { continue; }
        levels.insert(sid, lv);
        if let Some(nexts) = fwd_adj.get(&sid) {
            for &nxt in nexts {
                if !levels.contains_key(&nxt) {
                    queue.push_back((nxt, lv + 1));
                }
            }
        }
    }
    let max_lv = levels.values().max().copied().unwrap_or(0);
    let mut extra = 1;
    for step in &grafcet.steps {
        if !levels.contains_key(&step.id) {
            levels.insert(step.id, max_lv + extra);
            extra += 1;
        }
    }

    // ── 4. Grouper par niveau et assigner positions ─────────────────────
    let mut by_level: HashMap<usize, Vec<u32>> = HashMap::new();
    for (&id, &lv) in &levels { by_level.entry(lv).or_default().push(id); }
    for ids in by_level.values_mut() { ids.sort(); }

    let mut sorted_lvs: Vec<usize> = by_level.keys().cloned().collect();
    sorted_lvs.sort();

    let level_y: HashMap<usize, f32> = sorted_lvs.iter().enumerate()
        .map(|(i, &lv)| (lv, Y_START + i as f32 * y_step))
        .collect();

    for (&lv, ids) in &by_level {
        let y = level_y[&lv];
        let n = ids.len();
        for (j, &sid) in ids.iter().enumerate() {
            let x = if n == 1 {
                X_CENTER
            } else {
                let total_w = (n as f32 - 1.0) * X_GAP;
                X_CENTER - total_w / 2.0 + j as f32 * X_GAP
            };
            if let Some(step) = grafcet.step_mut(sid) { step.pos = [x, y]; }
        }
    }

    // ── 5. Positionner les barres de transition ────────────────────────────
    let min_step_x = grafcet.steps.iter().map(|s| s.pos[0]).fold(f32::INFINITY, f32::min);
    let loop_route_base = min_step_x - STEP_W / 2.0 - LOOP_X_BASE_OFFSET;

    let step_pos: HashMap<u32, [f32; 2]> = grafcet.steps.iter().map(|s| (s.id, s.pos)).collect();

    // Trier les back-edges par destination y décroissante pour échelonnement lisible
    let mut back_sorted: Vec<u32> = back_edge_tids.iter().cloned().collect();
    back_sorted.sort_by(|&a, &b| {
        let ya = grafcet.transition(a).and_then(|t| step_pos.get(&t.to_step)).map_or(0.0, |p| p[1]);
        let yb = grafcet.transition(b).and_then(|t| step_pos.get(&t.to_step)).map_or(0.0, |p| p[1]);
        ya.partial_cmp(&yb).unwrap_or(std::cmp::Ordering::Equal)
    });
    let back_route_x: HashMap<u32, f32> = back_sorted.iter().enumerate()
        .map(|(i, &tid)| (tid, loop_route_base - i as f32 * LOOP_X_STEP))
        .collect();

    // Détection des divergences OU (≥2 transitions non-back-edge depuis la même étape source)
    // → ces transitions seront placées à l'X de leur étape DESTINATION (colinéarité verticale)
    let mut fwd_from_groups: HashMap<u32, Vec<u32>> = HashMap::new();
    for t in &grafcet.transitions {
        if !back_edge_tids.contains(&t.id) {
            fwd_from_groups.entry(t.from_step).or_default().push(t.id);
        }
    }
    let is_div_trans: HashSet<u32> = fwd_from_groups.values()
        .filter(|v| v.len() >= 2)
        .flat_map(|v| v.iter().cloned())
        .collect();

    for t in &mut grafcet.transitions {
        let [fx, fy] = step_pos.get(&t.from_step).copied().unwrap_or([X_CENTER, Y_START]);
        let [dx, _dy] = step_pos.get(&t.to_step).copied().unwrap_or([X_CENTER, Y_START]);
        let bar_y = fy + bar_y_offset;
        // Divergence : placer à l'X destination (colinéaire avec l'étape conditionnée)
        // Back-edge ou transition simple : placer à l'X source
        let bar_x = if !back_edge_tids.contains(&t.id) && is_div_trans.contains(&t.id) {
            dx
        } else {
            fx
        };
        t.pos     = [bar_x, bar_y];
        t.route_y = None;
        if back_edge_tids.contains(&t.id) {
            t.dst_route_x = Some(*back_route_x.get(&t.id).unwrap_or(&loop_route_base));
        } else {
            t.dst_route_x = None;
        }
    }
}
