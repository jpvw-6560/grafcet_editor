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
    /// Un CanvasEditor par grafcet (indexé pareil que project.grafcets)
    editors: Vec<CanvasEditor>,
    /// Tampon pour le nom d'un nouveau grafcet
    new_grafcet_name: String,
    show_add_popup: bool,
    /// Déclenche la génération depuis le GEMMA au prochain frame
    generate_from_gemma: bool,
    /// Index du grafcet à supprimer (traité hors closure de panel)
    pending_delete: Option<usize>,
    /// Pour chaque onglet : vue partagée JSON + canvas active
    graphic_active: Vec<bool>,
}

impl Default for GrafcetsPage {
    fn default() -> Self {
        Self {
            active_tab: 0,
            editors: Vec::new(),
            new_grafcet_name: String::new(),
            show_add_popup: false,
            generate_from_gemma: false,
            pending_delete: None,
            graphic_active: Vec::new(),
        }
    }
}

impl GrafcetsPage {
    /// Réinitialise les éditeurs quand le projet change.
    pub fn reset(&mut self) {
        self.editors.clear();
        self.graphic_active.clear();
        self.active_tab = 0;
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

        // ── Barre d'onglets ────────────────────────────────────────────────
        egui::Panel::top("grafcets_tabs")
            .exact_size(36.0)
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(26, 37, 47)).inner_margin(egui::Margin::same(4)))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    for (i, ng) in project.grafcets.iter().enumerate() {
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
                        .min_size(Vec2::new(70.0, 28.0));
                        let resp = ui.add(btn);
                        let resp = if ng.short_name.is_some() {
                            resp.on_hover_text(&ng.name)
                        } else {
                            resp
                        };
                        if resp.clicked() {
                            self.active_tab = i;
                        }
                        ui.add_space(2.0);
                    }

                    // Bouton + nouveau
                    ui.add_space(8.0);
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("＋").size(14.0).color(egui::Color32::WHITE),
                            )
                            .fill(egui::Color32::from_rgb(39, 60, 78))
                            .min_size(Vec2::new(32.0, 28.0)),
                        )
                        .on_hover_text("Ajouter un grafcet")
                        .clicked()
                    {
                        self.show_add_popup = true;
                    }

                    // Bouton ⚙ Depuis GEMMA (toujours visible, grisé si GEMMA vide)
                    {
                        let gemma_ready = !project.gemma.states.is_empty();
                        ui.add_space(6.0);
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
                        let resp = ui
                            .add_enabled(
                                gemma_ready,
                                egui::Button::new(
                                    egui::RichText::new("⚙ Depuis GEMMA")
                                        .size(11.0)
                                        .color(text_color),
                                )
                                .fill(btn_color)
                                .min_size(Vec2::new(115.0, 28.0)),
                            );
                        let resp = resp.on_hover_text(if gemma_ready {
                            "Générer les grafcets depuis les circuits fermés du GEMMA"
                        } else {
                            "GEMMA non encore généré — remplissez le questionnaire d'abord"
                        });
                        if resp.clicked() {
                            self.generate_from_gemma = true;
                        }
                    }

                    // Bouton supprimer (à droite) — flag différé hors closure
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !project.grafcets.is_empty() {
                            let del = ui.add(
                                egui::Button::new(
                                    egui::RichText::new("🗑 Supprimer").size(11.0).color(egui::Color32::WHITE),
                                )
                                .fill(egui::Color32::from_rgb(150, 40, 40))
                                .min_size(Vec2::new(90.0, 28.0)),
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
                if del_idx < self.editors.len() {
                    self.editors.remove(del_idx);
                }
                self.active_tab = self.active_tab.saturating_sub(if del_idx <= self.active_tab { 1 } else { 0 });
                status_out = Some(format!("Grafcet « {name} » supprimé"));
            }
        }

        // ── Génération depuis GEMMA ────────────────────────────────────────
        if self.generate_from_gemma {
            self.generate_from_gemma = false;
            let circuits = crate::gemma::extract_closed_circuits(&project.gemma);
            let mut count = 0usize;
            for circuit in &circuits {
                let name = circuit_name(circuit);
                if !project.grafcets.iter().any(|ng| ng.name == name) {
                    let grafcet = circuit_to_grafcet(&project.gemma, circuit);
                    project.grafcets.push(NamedGrafcet {
                        name: name.clone(),
                        short_name: Some(circuit_short_name(circuit)),
                        grafcet,
                        generated: true,
                    });
                    self.editors.push(CanvasEditor::default());
                    count += 1;
                }
            }
            status_out = if count > 0 {
                Some(format!("{count} grafcet(s) générés depuis les circuits GEMMA"))
            } else if circuits.is_empty() {
                Some("Aucun circuit fermé trouvé dans le GEMMA".to_string())
            } else {
                Some("Tous les circuits GEMMA sont déjà présents".to_string())
            };
            if count > 0 {
                self.active_tab = project.grafcets.len().saturating_sub(count);
            }
        }

        // ── Contenu de l'onglet actif ──────────────────────────────────────
        let idx = self.active_tab.min(project.grafcets.len().saturating_sub(1));
        if let Some(ng) = project.grafcets.get_mut(idx) {
            let is_split = self.graphic_active.get(idx).copied().unwrap_or(false);

            if ng.generated && is_split {
                // ── Vue partagée : JSON à gauche | Canvas à droite ────────
                let buf = grafcet_summary(&ng.grafcet);
                let mut close_graphic = false;

                egui::Panel::left(egui::Id::new("json_split").with(idx))
                    .resizable(true)
                    .default_width(340.0)
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
                                    if ui
                                        .add(egui::Button::new(
                                            egui::RichText::new("◀ JSON seul").size(11.0),
                                        ))
                                        .on_hover_text("Fermer la vue graphique")
                                        .clicked()
                                    {
                                        close_graphic = true;
                                    }
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

                if close_graphic {
                    if let Some(a) = self.graphic_active.get_mut(idx) { *a = false; }
                }

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
                                egui::RichText::new("Grafcet généré depuis le GEMMA — lecture seule")
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
fn circuit_short_name(circuit: &[String]) -> String {
    let n = circuit.len();
    if n <= 4 {
        circuit.join("→")
    } else {
        format!("{}→…→{}", circuit[0], circuit[n - 1])
    }
}

/// Retourne un nom descriptif pour un circuit GEMMA,
/// selon les correspondances définies dans circuits_fermes_possibles.md.
fn circuit_name(circuit: &[String]) -> String {
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
fn circuit_to_grafcet(gemma: &Gemma, circuit: &[String]) -> Grafcet {
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
/// à partir d'un parcours BFS depuis l'étape initiale.
///
/// Formules (STEP_H=80, STEP_WICK=30, TRANS_WICK=25) :
///   sy_anchor = step_y + STEP_H/2 + STEP_WICK  = step_y + 70
///   bar_y     = step_y + 70 + TRANS_WICK        = step_y + 95  (connexion haute seamless)
///   dy_anchor = next_y - 70
///   t_bot_y   = bar_y + TRANS_WICK              = step_y + 120
///   → pour goes_up = false : next_y - 70 >= step_y + 120 → Y_STEP >= 190
///   → avec Y_STEP = 190 : next_y - 70 = step_y + 120, connexion basse seamless
pub fn auto_layout(grafcet: &mut Grafcet) {
    use std::collections::{HashMap, VecDeque};

    if grafcet.steps.is_empty() { return; }

    const X_CENTER: f32 = 400.0;
    const X_GAP: f32    = 180.0;  // écart horizontal entre colonnes parallèles
    const Y_START: f32  = 100.0;  // y de l'étape initiale
    const Y_STEP: f32   = 190.0;  // distance centre-à-centre = STEP_H/2 + STEP_WICK + TRANS_WICK*2 + STEP_WICK + STEP_H/2
    const STEP_H2: f32  = 40.0;   // STEP_H / 2
    const WICK: f32     = 30.0;   // STEP_WICK
    const TWCK: f32     = 25.0;   // TRANS_WICK

    // ── 1. Étape initiale ──────────────────────────────────────────────────
    let start_id = grafcet.steps.iter()
        .find(|s| s.kind == StepKind::Initial)
        .or_else(|| grafcet.steps.first())
        .map(|s| s.id)
        .unwrap();

    // ── 2. BFS → niveau de chaque étape ───────────────────────────────────
    let adj: HashMap<u32, Vec<u32>> = {
        let mut m: HashMap<u32, Vec<u32>> = HashMap::new();
        for t in &grafcet.transitions {
            m.entry(t.from_step).or_default().push(t.to_step);
        }
        m
    };

    let mut levels: HashMap<u32, usize> = HashMap::new();
    let mut queue: VecDeque<(u32, usize)> = VecDeque::new();
    queue.push_back((start_id, 0));
    while let Some((sid, lv)) = queue.pop_front() {
        if levels.contains_key(&sid) { continue; }
        levels.insert(sid, lv);
        if let Some(nexts) = adj.get(&sid) {
            for &nxt in nexts {
                if !levels.contains_key(&nxt) {
                    queue.push_back((nxt, lv + 1));
                }
            }
        }
    }
    // Étapes non atteignables → niveaux supplémentaires
    let max_lv = levels.values().max().copied().unwrap_or(0);
    let mut extra = 1;
    for step in &grafcet.steps {
        if !levels.contains_key(&step.id) {
            levels.insert(step.id, max_lv + extra);
            extra += 1;
        }
    }

    // ── 3. Grouper par niveau ──────────────────────────────────────────────
    let mut by_level: HashMap<usize, Vec<u32>> = HashMap::new();
    for (&id, &lv) in &levels {
        by_level.entry(lv).or_default().push(id);
    }
    for ids in by_level.values_mut() { ids.sort(); }

    let mut sorted_lvs: Vec<usize> = by_level.keys().cloned().collect();
    sorted_lvs.sort();

    // niveau → y
    let level_y: HashMap<usize, f32> = sorted_lvs.iter().enumerate()
        .map(|(i, &lv)| (lv, Y_START + i as f32 * Y_STEP))
        .collect();

    // ── 4. Assigner positions aux étapes ──────────────────────────────────
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
            if let Some(step) = grafcet.step_mut(sid) {
                step.pos = [x, y];
            }
        }
    }

    // ── 5. Positionner les barres de transition ────────────────────────────
    // Calculer le X minimum (pour le décrochage des boucles de retour)
    let min_step_x = grafcet.steps.iter()
        .map(|s| s.pos[0])
        .fold(f32::INFINITY, f32::min);
    let loop_route_x = min_step_x - 40.0 /*STEP_W/2*/ - 65.0;

    let step_pos: HashMap<u32, [f32; 2]> = grafcet.steps.iter()
        .map(|s| (s.id, s.pos))
        .collect();

    for t in &mut grafcet.transitions {
        let [fx, fy] = step_pos.get(&t.from_step).copied().unwrap_or([X_CENTER, Y_START]);
        let [_tx, ty] = step_pos.get(&t.to_step).copied().unwrap_or([X_CENTER, Y_START]);

        // Barre centrée entre ancre-basse source et ancre-haute destination
        let bar_y = fy + STEP_H2 + WICK + TWCK; // = fy + 95
        let bar_x = fx;

        let is_loop = ty <= fy; // la destination est au-dessus (boucle de fermeture)
        t.pos = [bar_x, bar_y];
        t.route_y = None;
        if is_loop {
            // Route latérale gauche calculée depuis le X le plus à gauche
            t.dst_route_x = Some(loop_route_x);
        } else {
            t.dst_route_x = None;
        }
    }
}
