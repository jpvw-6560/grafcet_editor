// gui/pages/grafcets_page.rs — Page « Grafcets »
//
// Onglets : GS | GC | GPN | + extras
// Chaque onglet embarque le canvas GRAFCET existant (editor.rs).
// Un bouton « + Nouveau grafcet » ajoute un onglet supplémentaire.

use egui::Vec2;

use crate::gui::canvas_editor::CanvasEditor;
use crate::project::Project;

pub struct GrafcetsPage {
    /// Index de l'onglet affiché
    active_tab: usize,
    /// Un CanvasEditor par grafcet (indexé pareil que project.grafcets)
    editors: Vec<CanvasEditor>,
    /// Tampon pour le nom d'un nouveau grafcet
    new_grafcet_name: String,
    show_add_popup: bool,
}

impl Default for GrafcetsPage {
    fn default() -> Self {
        Self {
            active_tab: 0,
            editors: Vec::new(),
            new_grafcet_name: String::new(),
            show_add_popup: false,
        }
    }
}

impl GrafcetsPage {
    /// Réinitialise les éditeurs quand le projet change.
    pub fn reset(&mut self) {
        self.editors.clear();
        self.active_tab = 0;
    }

    pub fn show(&mut self, ui: &mut egui::Ui, project: &mut Project) -> Option<String> {
        let mut status_out: Option<String> = None;

        // Synchronise le nombre d'éditeurs avec le projet
        while self.editors.len() < project.grafcets.len() {
            self.editors.push(CanvasEditor::default());
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
                        let btn = egui::Button::new(
                            egui::RichText::new(&ng.name).size(12.0).color(fg),
                        )
                        .fill(bg)
                        .min_size(Vec2::new(70.0, 28.0));
                        if ui.add(btn).clicked() {
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

                    // Bouton renommer (à droite)
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !project.grafcets.is_empty() {
                            if ui.button("🗑 Supprimer").clicked() {
                                let idx = self.active_tab.min(project.grafcets.len().saturating_sub(1));
                                if idx < project.grafcets.len() {
                                    let name = project.grafcets[idx].name.clone();
                                    project.grafcets.remove(idx);
                                    self.editors.remove(idx);
                                    self.active_tab = self.active_tab.saturating_sub(1);
                                    status_out = Some(format!("Grafcet « {name} » supprimé"));
                                }
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

        // ── Canvas de l'onglet actif ───────────────────────────────────────
        let idx = self.active_tab.min(project.grafcets.len().saturating_sub(1));
        if let Some(ng) = project.grafcets.get_mut(idx) {
            if let Some(editor) = self.editors.get_mut(idx) {
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
