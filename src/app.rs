// app.rs — Application principale
//
// Structure :  sidebar (140 px) | page centrale
//
// La sidebar expose 3 sections  :
//   📁 Projet   →  ProjectPage  (nouveau / charger / récents)
//   🔷 GEMMA    →  GemmaPage    (canvas GEMMA avec états Safety/Command/Production)
//   📊 Grafcets →  GrafcetsPage (onglets GS / GC / GPN + extras)

use egui::Vec2;

use crate::gui::pages::gemma_page::GemmaPage;
use crate::gui::pages::grafcets_page::GrafcetsPage;
use crate::gui::pages::project_page::ProjectPage;
use crate::gui::pages::doc_page::DocPage;
use crate::project::Project;

// ── Section active ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Section {
    Project,
    Gemma,
    Grafcets,
    Doc,
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub section: Section,
    pub project: Option<Project>,
    pub current_path: Option<std::path::PathBuf>,
    pub status: String,

    // Pages (état UI persistant)
    pub project_page: ProjectPage,
    pub gemma_page: GemmaPage,
    pub grafcets_page: GrafcetsPage,
    pub doc_page: DocPage,
}

impl Default for App {
    fn default() -> Self {
        Self {
            section: Section::Project,
            project: None,
            current_path: None,
            status: "Bienvenue dans Gemma Suite".to_string(),
            project_page: ProjectPage::default(),
            gemma_page: GemmaPage::default(),
            grafcets_page: GrafcetsPage::default(),
            doc_page: DocPage::default(),
        }
    }
}

impl App {
    /// Crée l'app et charge automatiquement le dernier projet ouvert.
    pub fn new() -> Self {
        let mut app = Self::default();
        if let Some(last) = last_project_path() {
            if let Ok(p) = crate::persistence::project_io::load_project(&last) {
                let name = p.name.clone();
                // Normaliser : current_path est toujours le dossier projet
                let dir = if last.is_file() {
                    last.parent().map(|p| p.to_path_buf()).unwrap_or(last)
                } else {
                    last
                };
                // Déclencher fit-to-canvas si le projet a des états GEMMA
                if !p.gemma.states.is_empty() {
                    app.gemma_page.pending_fit = true;
                }
                app.project = Some(p);
                app.current_path = Some(dir);
                app.status = format!("Dernier projet « {name} » rechargé");
                app.section = Section::Gemma;
            }
        }
        app
    }
}

// ── Persistance du dernier projet ─────────────────────────────────────────────

fn last_project_file() -> std::path::PathBuf {
    std::path::PathBuf::from("data").join("last_project.txt")
}

fn save_last_project_path(path: &std::path::Path) {
    let _ = std::fs::create_dir_all("data");
    // Stocke le chemin absolu pour que ça fonctionne peu importe le cwd
    let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let _ = std::fs::write(last_project_file(), abs.to_string_lossy().as_ref());
}

fn last_project_path() -> Option<std::path::PathBuf> {
    let txt = std::fs::read_to_string(last_project_file()).ok()?;
    let p = std::path::PathBuf::from(txt.trim());
    if p.exists() { Some(p) } else { None }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Raccourci Ctrl+S
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::S)) {
            self.save_project();
        }

        // Couleurs globales
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(22, 32, 42);
        ctx.set_visuals(visuals);

        // ── Sidebar gauche ────────────────────────────────────────────────
        egui::Panel::left("sidebar")
            .exact_size(145.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(26, 37, 47)))
            .show_inside(ui, |ui| {
                self.draw_sidebar(ui);
            });

        // ── Barre de statut en bas ────────────────────────────────────────
        egui::Panel::bottom("status_bar")
            .exact_size(24.0)
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(15, 20, 28)))
            .show_inside(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(&self.status)
                            .size(11.0)
                            .color(egui::Color32::from_rgb(127, 176, 211)),
                    );
                });
            });

        // ── Page centrale ─────────────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(18, 26, 35)))
            .show_inside(ui, |ui| {
                match self.section {
                    Section::Project => {
                        if let Some(action) = self.project_page.show(ui, self.project.as_ref()) {
                            self.handle_project_action(action, &ctx);
                        }
                    }
                    Section::Gemma => {
                        if let Some(project) = self.project.as_mut() {
                            if let Some(msg) = self.gemma_page.show(ui, &mut project.gemma) {
                                self.status = msg;
                            }
                        } else {
                            ui.centered_and_justified(|ui| {
                                ui.label(
                                    egui::RichText::new("Aucun projet ouvert.\nCréez ou chargez un projet.")
                                        .size(14.0)
                                        .color(egui::Color32::from_rgb(100, 120, 140)),
                                );
                            });
                        }
                        // Auto-save si le questionnaire a modifié le GEMMA
                        if self.gemma_page.needs_save {
                            self.gemma_page.needs_save = false;
                            self.save_project();
                        }
                    }
                    Section::Grafcets => {
                        if let Some(project) = self.project.as_mut() {
                            if let Some(msg) = self.grafcets_page.show(ui, project) {
                                self.status = msg;
                            }
                        } else {
                            ui.centered_and_justified(|ui| {
                                ui.label(
                                    egui::RichText::new("Aucun projet ouvert.\nCréez ou chargez un projet.")
                                        .size(14.0)
                                        .color(egui::Color32::from_rgb(100, 120, 140)),
                                );
                            });
                        }
                        // Signal différé hors borrow de project
                        if self.grafcets_page.needs_full_generate {
                            self.grafcets_page.needs_full_generate = false;
                            self.generate_grafcets_from_gemma();
                        }
                    }
                    Section::Doc => {
                        if let Some(project) = self.project.as_mut() {
                            if self.doc_page.show(ui, &mut project.documentation) {
                                self.save_project();
                            }
                        } else {
                            ui.centered_and_justified(|ui| {
                                ui.label(
                                    egui::RichText::new("Aucun projet ouvert.\nCréez ou chargez un projet.")
                                        .size(14.0)
                                        .color(egui::Color32::from_rgb(100, 120, 140)),
                                );
                            });
                        }
                    }
                }
            });
    }
}

// ── Sidebar ───────────────────────────────────────────────────────────────────

impl App {
    fn draw_sidebar(&mut self, ui: &mut egui::Ui) {
        let w = ui.available_width();
        ui.set_min_width(w);

        ui.add_space(12.0);

        // Titre
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("Gemma Suite")
                    .size(14.0)
                    .strong()
                    .color(egui::Color32::from_rgb(200, 220, 240)),
            );
        });

        // Nom du projet courant
        let proj_name = self
            .project
            .as_ref()
            .map(|p| p.name.as_str())
            .unwrap_or("— aucun projet —");
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new(proj_name)
                    .size(10.0)
                    .italics()
                    .color(egui::Color32::from_rgb(46, 204, 113)),
            );
        });

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        // Boutons de section
        let buttons: &[(&str, Section)] = &[
            ("📁  Projet",        Section::Project),
            ("🔷  GEMMA",         Section::Gemma),
            ("📊  Grafcets",      Section::Grafcets),
            ("📝  Documentation", Section::Doc),
        ];

        for (label, sec) in buttons {
            let is_active = &self.section == sec;
            let (bg, fg) = if is_active {
                (
                    egui::Color32::from_rgb(41, 128, 185),
                    egui::Color32::WHITE,
                )
            } else {
                (
                    egui::Color32::TRANSPARENT,
                    egui::Color32::from_rgb(189, 195, 199),
                )
            };

            let btn = egui::Button::new(
                egui::RichText::new(*label).size(13.0).color(fg),
            )
            .fill(bg)
            .min_size(Vec2::new(w - 16.0, 36.0));

            ui.add_space(4.0);
            ui.vertical_centered(|ui| {
                let resp = ui.add(btn);
                if resp.clicked() {
                    self.section = sec.clone();
                }
            });
        }

        ui.add_space(16.0);
        ui.separator();
    }
}

// ── Actions projet ─────────────────────────────────────────────────────────────

pub enum ProjectAction {
    New(String),                               // nom
    Load(std::path::PathBuf),                  // chemin .project.json
    LoadDir(std::path::PathBuf),               // dossier → cherche project.json dedans
}

impl App {
    fn handle_project_action(&mut self, action: ProjectAction, _ctx: &egui::Context) {
        match action {
            ProjectAction::New(name) => {
                self.project = Some(Project::new(name.clone()));
                let dir = crate::persistence::project_io::project_dir(&name);
                self.current_path = Some(dir.clone());
                self.grafcets_page.reset();
                // Auto-save
                if let Some(ref project) = self.project {
                    match crate::persistence::project_io::save_project(project, &dir) {
                        Ok(()) => self.status = format!("Projet « {name} » créé"),
                        Err(e) => self.status = format!("Projet créé (erreur sauvegarde: {e})"),
                    }
                }
                save_last_project_path(&dir.join("project.json"));
                self.section = Section::Gemma;
            }
            ProjectAction::Load(path) => {
                match crate::persistence::project_io::load_project(&path) {
                    Ok(p) => {
                        let name = p.name.clone();
                        // Normaliser : current_path est toujours le dossier projet
                        let dir = if path.is_file() {
                            path.parent().map(|p| p.to_path_buf()).unwrap_or(path.clone())
                        } else {
                            path.clone()
                        };
                        save_last_project_path(&dir.join("project.json"));
                        if !p.gemma.states.is_empty() {
                            self.gemma_page.pending_fit = true;
                            self.status = format!("Projet « {name} » chargé");
                        } else {
                            self.status = format!(
                                "Projet « {name} » chargé — GEMMA vide, utilisez le Questionnaire pour générer"
                            );
                        }
                        self.project = Some(p);
                        self.current_path = Some(dir);
                        self.grafcets_page.reset();
                        self.section = Section::Gemma;
                    }
                    Err(e) => self.status = format!("Erreur chargement : {e}"),
                }
            }
            ProjectAction::LoadDir(dir) => {
                let path = dir.join("project.json");
                if path.exists() {
                    self.handle_project_action(ProjectAction::Load(path), _ctx);
                } else {
                    self.status = format!("Aucun project.json dans {:?}", dir);
                }
            }
        }
    }

    fn save_project(&mut self) {
        let (name, dir) = match &self.project {
            None => {
                self.status = "Aucun projet à sauvegarder".to_string();
                return;
            }
            Some(p) => {
                let dir = self.current_path.clone()
                    .unwrap_or_else(|| crate::persistence::project_io::project_dir(&p.name));
                (p.name.clone(), dir)
            }
        };
        let Some(project) = self.project.as_ref() else { return; };
        match crate::persistence::project_io::save_project(project, &dir) {
            Ok(()) => {
                self.current_path = Some(dir.clone());
                self.status = format!("Sauvegardé : {}", dir.display());
            }
            Err(e) => self.status = format!("Erreur sauvegarde : {e}"),
        }
        let _ = name; // éviter warning unused
    }

    /// Regénère TOUS les grafcets issus du GEMMA.
    /// Structure cible :
    ///   GS (complet), GC (stub), puis un grafcet par état Command (F1..F6, stubs).
    /// Les grafcets utilisateur (generated=false) sont conservés à la fin.
    fn generate_grafcets_from_gemma(&mut self) {
        let Some(project) = self.project.as_mut() else {
            self.status = "Aucun projet ouvert".to_string();
            return;
        };

        if let Err(errors) = project.gemma.validate() {
            self.status = format!("GEMMA invalide : {}", errors.join(" | "));
            return;
        }

        use crate::gemma::StateType;
        use crate::grafcet::StepKind;
        use crate::project::NamedGrafcet;

        // ── 1. Supprimer TOUS les grafcets existants ─────────────────────────
        project.grafcets.clear();

        // ── Extraction owned des données GEMMA ───────────────────────────────
        struct StateInfo { id: String, stype: StateType, action: String }
        struct TransInfo { from: String, from_type: StateType, to: String, to_type: StateType, cond: String }

        let state_infos: Vec<StateInfo> = project.gemma.states.iter()
            .map(|s| StateInfo { id: s.id.clone(), stype: s.state_type, action: s.action.clone() })
            .collect();

        let trans_infos: Vec<TransInfo> = project.gemma.transitions.iter()
            .map(|t| {
                let from_type = state_infos.iter().find(|s| s.id == t.from)
                    .map_or(StateType::Command, |s| s.stype);
                let to_type = state_infos.iter().find(|s| s.id == t.to)
                    .map_or(StateType::Command, |s| s.stype);
                TransInfo {
                    from: t.from.clone(), from_type,
                    to:   t.to.clone(),   to_type,
                    cond: t.condition.to_display(),
                }
            })
            .collect();

        fn stub(name: &str) -> NamedGrafcet {
            let mut ng = NamedGrafcet::new(name);
            ng.generated = true;
            if let Some(s) = ng.grafcet.steps.first_mut() {
                s.actions.push("— pas encore implémenté —".to_string());
            }
            ng
        }

        // ── 2. GS : Grafcet de Sécurité — génération complète ───────────────
        {
            let mut sorted: Vec<&StateInfo> = state_infos.iter()
                .filter(|s| s.stype == StateType::Safety).collect();

            if !sorted.is_empty() {
                sorted.sort_by(|a, b| {
                    // A-états en premier, puis D-états ; alphabétique dans chaque groupe
                    let ga = u8::from(!a.id.starts_with('A'));
                    let gb = u8::from(!b.id.starts_with('A'));
                    ga.cmp(&gb).then(a.id.cmp(&b.id))
                });

                let mut ng = NamedGrafcet::new("GS");
                ng.generated = true;
                ng.grafcet = crate::grafcet::Grafcet::new();

                let mut id_map = std::collections::HashMap::<String, u32>::new();
                for (i, state) in sorted.iter().enumerate() {
                    let sid = ng.grafcet.add_step([200.0, 80.0 + i as f32 * 150.0]);
                    if let Some(s) = ng.grafcet.step_mut(sid) {
                        s.label = state.id.clone();
                        // Action du GEMMA → actions de l'étape
                        if !state.action.is_empty() {
                            s.actions.push(state.action.clone());
                        }
                        if i == 0 { s.kind = StepKind::Initial; }
                    }
                    id_map.insert(state.id.clone(), sid);
                }

                // Transitions GEMMA impliquant un état Safety en destination
                for t in &trans_infos {
                    if t.to_type != StateType::Safety { continue; }
                    let to_sid = match id_map.get(&t.to) { Some(&v) => v, None => continue };
                    let nc = |c: &str| if c == "TRUE" || c.is_empty() { "1".to_string() } else { c.to_string() };

                    if t.from_type == StateType::Safety {
                        // Safety → Safety : transition directe
                        if let Some(&from_sid) = id_map.get(&t.from) {
                            let tid = ng.grafcet.add_transition(from_sid, to_sid);
                            if let Some(tr) = ng.grafcet.transition_mut(tid) { tr.condition = nc(&t.cond); }
                        }
                    } else {
                        // Cross-catégorie (ex : F1→D1 via ARU) : accrochée au 1er A-état
                        // avec la condition "X_{source} AND {cond}" pour garder la sémantique GEMMA
                        let anchor = sorted.iter().find(|s| s.id.starts_with('A'))
                            .and_then(|s| id_map.get(&s.id)).copied();
                        if let Some(from_sid) = anchor {
                            let cond = if t.cond == "TRUE" || t.cond.is_empty() {
                                format!("X_{}", t.from)
                            } else {
                                format!("X_{} AND {}", t.from, t.cond)
                            };
                            if !ng.grafcet.transitions.iter()
                                .any(|tr| tr.from_step == from_sid && tr.to_step == to_sid)
                            {
                                let tid = ng.grafcet.add_transition(from_sid, to_sid);
                                if let Some(tr) = ng.grafcet.transition_mut(tid) { tr.condition = cond; }
                            }
                        }
                    }
                }

                // ARU automatique : de chaque A-état vers le 1er D-état
                let first_d = sorted.iter().find(|s| s.id.starts_with('D'))
                    .and_then(|s| id_map.get(&s.id)).copied();
                if let Some(d_sid) = first_d {
                    for state in &sorted {
                        if !state.id.starts_with('A') { continue; }
                        if let Some(&a_sid) = id_map.get(&state.id) {
                            if !ng.grafcet.transitions.iter()
                                .any(|t| t.from_step == a_sid && t.to_step == d_sid)
                            {
                                let tid = ng.grafcet.add_transition(a_sid, d_sid);
                                if let Some(tr) = ng.grafcet.transition_mut(tid) {
                                    tr.condition = "ARU".to_string();
                                }
                            }
                        }
                    }
                }

                project.grafcets.push(ng);
            }
        }

        // ── 3. GC : Grafcet de Commandes — stub ────────────────────────────
        if state_infos.iter().any(|s| s.stype == StateType::Command) {
            project.grafcets.push(stub("GC"));
        }

        // ── 4. Réintégrer les grafcets utilisateur à la fin ——— SUPPRIMÉ ────
        // Les grafcets F1-F6 (GPN, G_PREP, etc.) sont créés manuellement
        // par l'utilisateur via "+ Nouveau".

        self.grafcets_page.reset();
        self.status = "Grafcets regénérés : GS (complet) + GC ✓".to_string();
        self.section = Section::Grafcets;
    }
}
