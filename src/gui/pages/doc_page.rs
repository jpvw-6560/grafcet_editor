// gui/pages/doc_page.rs — Page de documentation du projet (texte libre)
//
// Affiche un éditeur de texte multiligne où l'utilisateur peut rédiger
// toute documentation utile sur le programme PRP :
//   - Description de la machine
//   - Liste des capteurs / actionneurs
//   - Variables PLC, remarques de sécurité, versions…
//
// Le contenu est stocké dans Project::documentation et sauvegardé avec
// le reste du projet (Ctrl+S / auto-save).

use egui::Vec2;

// ── Page ─────────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct DocPage;

impl DocPage {
    /// Affiche la page de documentation.
    /// Retourne `true` si le contenu a été modifié (déclenche une sauvegarde).
    pub fn show(&mut self, ui: &mut egui::Ui, documentation: &mut String) -> bool {
        let mut modified = false;

        // ── Entête ────────────────────────────────────────────────────────
        egui::Panel::top("doc_header")
            .exact_size(44.0)
            .frame(egui::Frame::new()
                .fill(egui::Color32::from_rgb(20, 30, 40))
                .inner_margin(egui::Margin::same(8)))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("📝  Documentation du projet")
                            .size(15.0)
                            .strong()
                            .color(egui::Color32::from_rgb(200, 220, 240)),
                    );
                    ui.add_space(16.0);
                    ui.label(
                        egui::RichText::new(
                            "Décrivez ici la machine, les variables PLC, les remarques de sécurité…"
                        )
                        .size(11.0)
                        .italics()
                        .color(egui::Color32::from_rgb(110, 130, 150)),
                    );
                });
            });

        // ── Zone d'édition ────────────────────────────────────────────────
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(14, 20, 28))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                let available = ui.available_size();
                let resp = ui.add(
                    egui::TextEdit::multiline(documentation)
                        .font(egui::FontId::monospace(13.0))
                        .desired_width(available.x)
                        .desired_rows(30)
                        .hint_text(concat!(
                            "Saisissez ici la documentation du projet…\n\n",
                            "Exemples :\n",
                            "  - Description de la machine\n",
                            "  - Liste des capteurs / actionneurs\n",
                            "  - Variables PLC (M0, M1…)\n",
                            "  - Remarques de sécurité\n",
                            "  - Historique des versions\n",
                        ))
                        .min_size(Vec2::new(available.x, available.y - 4.0)),
                );
                if resp.changed() {
                    modified = true;
                }
            });

        modified
    }
}
