// gui/pages/project_page.rs — Page « Projet »
//
// Contient :
//   - Zone "Projet courant" (nom + chemin)
//   - Bouton Nouveau projet (saisie nom)
//   - Bouton Charger un projet (file dialog)
//   - Liste des projets récents (scan du dossier courant)

use crate::app::ProjectAction;
use crate::project::Project;

#[derive(Default)]
pub struct ProjectPage {
    /// Tampon de saisie pour le nom du nouveau projet
    new_name: String,
    /// Feedback de validation
    name_error: Option<String>,
}

impl ProjectPage {
    /// Retourne `Some(action)` si l'utilisateur a déclenché une action.
    pub fn show(&mut self, ui: &mut egui::Ui, project: Option<&Project>) -> Option<ProjectAction> {
        let mut action = None;

        ui.add_space(24.0);

        // ── Titre ─────────────────────────────────────────────────────────
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("📁  Projet")
                    .size(22.0)
                    .strong()
                    .color(egui::Color32::from_rgb(200, 220, 240)),
            );
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(16.0);

        // ── Projet courant ────────────────────────────────────────────────
        section_title(ui, "PROJET COURANT");
        ui.add_space(4.0);
        if let Some(p) = project {
            ui.label(
                egui::RichText::new(format!("✔  {}", p.name))
                    .size(13.0)
                    .color(egui::Color32::from_rgb(46, 204, 113)),
            );
            if !p.description.is_empty() {
                ui.label(
                    egui::RichText::new(&p.description)
                        .size(11.0)
                        .italics()
                        .color(egui::Color32::from_rgb(100, 130, 150)),
                );
            }
        } else {
            ui.label(
                egui::RichText::new("— aucun projet ouvert —")
                    .size(12.0)
                    .italics()
                    .color(egui::Color32::from_rgb(100, 110, 120)),
            );
        }

        ui.add_space(20.0);
        ui.separator();
        ui.add_space(12.0);

        // ── Nouveau projet ────────────────────────────────────────────────
        section_title(ui, "NOUVEAU PROJET");
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Nom :")
                    .size(12.0)
                    .color(egui::Color32::from_rgb(170, 190, 210)),
            );
            let te = egui::TextEdit::singleline(&mut self.new_name)
                .desired_width(200.0)
                .hint_text("Mon projet…");
            ui.add(te);
        });

        if let Some(err) = &self.name_error {
            ui.label(
                egui::RichText::new(err)
                    .size(11.0)
                    .color(egui::Color32::from_rgb(231, 76, 60)),
            );
        }

        ui.add_space(6.0);

        let create_btn = styled_button(ui, "✚  Créer le projet", egui::Color32::from_rgb(41, 128, 185));
        if create_btn.clicked() {
            let name = self.new_name.trim().to_string();
            if name.is_empty() {
                self.name_error = Some("Le nom ne peut pas être vide.".into());
            } else {
                self.name_error = None;
                self.new_name.clear();
                action = Some(ProjectAction::New(name));
            }
        }

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(12.0);

        // ── Charger un projet ─────────────────────────────────────────────
        section_title(ui, "CHARGER UN PROJET");
        ui.add_space(6.0);

        let load_btn = styled_button(ui, "📂  Ouvrir un fichier…", egui::Color32::from_rgb(39, 60, 78));
        if load_btn.clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Projet GEMMA", &["json"])
                .pick_file()
            {
                action = Some(ProjectAction::Load(path));
            }
        }

        action
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn section_title(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(10.0)
            .strong()
            .color(egui::Color32::from_rgb(127, 176, 211))
            .extra_letter_spacing(1.0),
    );
}

fn styled_button(ui: &mut egui::Ui, label: &str, bg: egui::Color32) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .size(12.0)
                .color(egui::Color32::WHITE),
        )
        .fill(bg)
        .min_size(egui::Vec2::new(220.0, 32.0)),
    )
}
