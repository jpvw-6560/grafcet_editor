// gemma/questionnaire.rs — Questionnaire GEMMA guidé (13 questions Oui/Non/?)
//
// Charge gemma_questionnaire.json (embarqué à la compilation via include_str!),
// permet de répondre à chaque question, puis applique les états/transitions
// résultants au modèle Gemma.

use serde::{Deserialize, Serialize};

use super::{Expr, Gemma, GemmaState, StateType};

// JSON embarqué à la compilation
const QUESTIONNAIRE_JSON: &str =
    include_str!("../core/data/gemma_questionnaire.json");

// ── Structures de désérialisation du JSON ─────────────────────────────────────

#[derive(Debug, Deserialize)]
struct QTransitionData {
    de: String,
    vers: String,
    condition: String,
}

#[derive(Debug, Deserialize)]
struct QuestionData {
    id: u32,
    titre: String,
    question: String,
    #[serde(default)]
    conditions: Vec<String>,
    #[serde(default)]
    etats_si_oui: Vec<String>,
    #[serde(default)]
    etats_si_non: Vec<String>,
    #[serde(default)]
    transitions_si_oui: Vec<QTransitionData>,
    #[serde(default)]
    transitions_si_non: Vec<QTransitionData>,
}

#[derive(Debug, Deserialize)]
struct QuestionnaireFile {
    questions: Vec<QuestionData>,
}

// ── Réponse possible ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum Answer {
    Yes,
    No,
    #[default]
    Unanswered,
}

// ── Question UI ───────────────────────────────────────────────────────────────

pub struct Question {
    pub id: u32,
    pub titre: String,
    pub question: String,
    pub conditions: Vec<String>,
    pub answer: Answer,
    // données internes (pas affichées directement)
    etats_si_oui: Vec<String>,
    etats_si_non: Vec<String>,
    transitions_si_oui: Vec<(String, String, String)>, // (from, to, cond)
    transitions_si_non: Vec<(String, String, String)>,
}

// ── Questionnaire principal ───────────────────────────────────────────────────

pub struct Questionnaire {
    pub questions: Vec<Question>,
}

impl Default for Questionnaire {
    fn default() -> Self {
        Self::load()
    }
}

impl Questionnaire {
    /// Charge les questions depuis le JSON embarqué.
    pub fn load() -> Self {
        let file: QuestionnaireFile =
            serde_json::from_str(QUESTIONNAIRE_JSON).expect("gemma_questionnaire.json invalide");

        let questions = file
            .questions
            .into_iter()
            .map(|q| Question {
                id: q.id,
                titre: q.titre,
                question: q.question,
                conditions: q.conditions,
                answer: Answer::Unanswered,
                etats_si_oui: q.etats_si_oui,
                etats_si_non: q.etats_si_non,
                transitions_si_oui: q
                    .transitions_si_oui
                    .into_iter()
                    .map(|t| (t.de, t.vers, t.condition))
                    .collect(),
                transitions_si_non: q
                    .transitions_si_non
                    .into_iter()
                    .map(|t| (t.de, t.vers, t.condition))
                    .collect(),
            })
            .collect();

        Self { questions }
    }

    /// Réinitialise toutes les réponses.
    pub fn reset_answers(&mut self) {
        for q in &mut self.questions {
            q.answer = Answer::Unanswered;
        }
    }

    /// Retourne le nombre de questions répondues.
    pub fn answered_count(&self) -> usize {
        self.questions.iter().filter(|q| q.answer != Answer::Unanswered).count()
    }

    /// Applique les réponses au GEMMA : ajoute états + transitions manquants.
    /// Les états déjà présents ne sont pas dupliqués.
    /// Utilise les positions canoniques du modèle Python (canvas 1620×1020 → 700×550).
    /// `saved_routes` : table chargée depuis `data/gemma_routes.json` — utilisée
    /// en priorité absolue pour les waypoints.  Les conditions viennent du questionnaire.
    pub fn apply_to_gemma(
        &self,
        gemma: &mut Gemma,
        saved_routes: &super::SavedRoutes,
    ) {
        // 1. Collecter tous les states/transitions à ajouter
        let mut states_to_add: Vec<(String, StateType)> = Vec::new();
        let mut trans_to_add: Vec<(String, String, String)> = Vec::new();

        for q in &self.questions {
            let (states, trans) = match q.answer {
                Answer::Yes => (&q.etats_si_oui, &q.transitions_si_oui),
                Answer::No  => (&q.etats_si_non, &q.transitions_si_non),
                Answer::Unanswered => continue,
            };

            for sid in states {
                let stype = state_type_from_id(sid);
                if !states_to_add.iter().any(|(id, _)| id == sid) {
                    states_to_add.push((sid.clone(), stype));
                }
            }
            for (from, to, cond) in trans {
                for sid in [from, to] {
                    let stype = state_type_from_id(sid);
                    if !states_to_add.iter().any(|(id, _)| id == sid) {
                        states_to_add.push((sid.clone(), stype));
                    }
                }
                trans_to_add.push((from.clone(), to.clone(), cond.clone()));
            }
        }

        // 2. Placer les états à leur position canonique (ou en grille de fallback)
        let mut col_counters: std::collections::HashMap<StateType, u32> =
            std::collections::HashMap::new();
        let col_x = |stype: StateType| match stype {
            StateType::Safety     => 80.0_f32,
            StateType::Command    => 280.0,
            StateType::Production => 480.0,
        };

        for (sid, stype) in &states_to_add {
            if gemma.state(sid).is_some() {
                continue;
            }
            if let Some([cx, cy, w, h]) = canonical_geom(sid) {
                gemma.states.push(GemmaState {
                    id: sid.clone(),
                    state_type: *stype,
                    pos: [cx, cy],
                    w,
                    h,
                    description: String::new(),
                    action: String::new(),
                });
            } else {
                // Grille de fallback pour états non-canoniques
                let row = col_counters.entry(*stype).or_insert(0);
                let pos = [col_x(*stype), 80.0 + (*row as f32) * 90.0];
                *row += 1;
                gemma.states.push(GemmaState {
                    id: sid.clone(),
                    state_type: *stype,
                    pos,
                    w: 0.0,
                    h: 0.0,
                    description: String::new(),
                    action: String::new(),
                });
            }
        }

        // 3. Ajouter/mettre à jour les transitions
        // Waypoints : routes sauvegardées (immuables, priorité absolue).
        // Conditions : questionnaire JSON.
        for (from, to, cond) in &trans_to_add {
            let key = (from.clone(), to.clone());
            let saved_wpts: Option<Vec<[f32; 2]>> = saved_routes.get(&key)
                .and_then(|(wpts, _)| if !wpts.is_empty() { Some(wpts.clone()) } else { None });

            if let Some(t) = gemma.transitions.iter_mut()
                    .find(|t| &t.from == from && &t.to == to) {
                // Transition existante : mettre à jour condition + waypoints
                t.condition = Expr::from_str(cond);
                if let Some(wpts) = saved_wpts {
                    t.waypoints = wpts;
                }
            } else {
                // Nouvelle transition
                let id = gemma.add_transition(from.clone(), to.clone(), Expr::from_str(cond));
                if let Some(wpts) = saved_wpts {
                    if let Some(t) = gemma.transitions.iter_mut().find(|t| t.id == id) {
                        t.waypoints = wpts;
                    }
                }
            }
        }

        // 4. Fermeture des circuits ouverts (règle GEMMA)
        close_open_circuits(gemma, saved_routes);
    }
}

/// Ferme les circuits ouverts du GEMMA selon les règles normalisées
/// (voir circuits_fermes_possibles.md).
///
/// Deux passes itératives jusqu'à stabilisation :
///   1. États drain  (aucune sortie)  → ajouter la transition sortante manquante
///   2. États source non-initiaux (aucune entrée, sauf A1/A6) → ajouter l'entrée manquante
///
/// Transitions INTERDITES (jamais générées automatiquement) :
///   D1→F1  (court-circuit sécurité), A3→F1  (saut arrêt contrôlé),
///   A7→F1  (saut mode réglage),      A1→F6  (accès test hors réglage)
///
/// Seules les transitions entre états DÉJÀ présents sont ajoutées.
fn close_open_circuits(gemma: &mut Gemma, saved_routes: &super::SavedRoutes) {

    // ── Applique les waypoints sur une transition nouvellement créée ──────────
    fn set_wpts(gemma: &mut Gemma, id: u32, from: &str, to: &str,
                saved_routes: &super::SavedRoutes) {
        let key = (from.to_string(), to.to_string());
        let wpts = saved_routes
            .get(&key)
            .and_then(|(w, _)| if !w.is_empty() { Some(w.clone()) } else { None })
            .unwrap_or_else(|| super::static_gemma_waypoints(from, to));
        if !wpts.is_empty() {
            if let Some(t) = gemma.transitions.iter_mut().find(|t| t.id == id) {
                t.waypoints = wpts;
            }
        }
    }

    // ── Transitions interdites ────────────────────────────────────────────────
    const FORBIDDEN: &[(&str, &str)] = &[
        ("D1", "F1"),   // court-circuit sécurité
        ("A3", "F1"),   // saut de l'arrêt contrôlé
        ("A7", "F1"),   // accès production depuis maintenance
        ("A1", "F6"),   // accès test hors mode réglage
    ];

    // ── Règles drain ─────────────────────────────────────────────────────────
    // Pour chaque état sans transition sortante : liste ordonnée de (cible, condition).
    // La première cible existante dans le GEMMA (et non interdite) est choisie.
    //
    // Fondées sur les circuits autorisés :
    //   A2→A1 (circuit 1/10), A3→A4 (circuit 3), A4→F1/A6 (circuit 3),
    //   A5→A6 (circuit 4/6),  A7→A4/A6 (circuit 6),
    //   D1→D2/A5 (circuit 4), D2→A5/A1 (circuit 4),
    //   D3→F1 (circuit 5),    F2→F1 (circuit 10), F3→A1 (circuit 11),
    //   F4→A6/A1 (circuit 8), F5→F1/F4 (circuit 9), F6→F1 (circuit 7)
    let drain_rules: &[(&str, &[(&str, &str)])] = &[
        ("A2", &[("A1", "Fin_cycle")]),
        ("A3", &[("A4", "Arret_obtenu")]),               // A3→F1 interdit
        ("A4", &[("F1", "Remise_en_marche"),
                 ("A6", "Init_position"),
                 ("A1", "Retour_initial")]),
        ("A5", &[("A6", "Reprise_apres_defaut"),
                 ("A1", "Reprise_apres_defaut")]),
        ("A7", &[("A4", "Arret_obtenu"),                 // A7→F1 interdit
                 ("A6", "Quitter_reglage")]),
        ("D1", &[("D2", "EU_relachee"),                  // D1→F1 interdit
                 ("A5", "Reset_direct")]),
        ("D2", &[("A5", "Acquit_defaut"),
                 ("A1", "Acquit_defaut")]),
        ("D3", &[("F1", "Defaut_resolu"),
                 ("A1", "Defaut_resolu")]),
        ("F2", &[("F1", "Preparation_ok"),
                 ("A1", "Fin_preparation")]),
        ("F3", &[("A1", "Cloture_ok")]),
        ("F4", &[("A6", "CI"),
                 ("A1", "CI")]),
        ("F5", &[("F1", "Fin_verif"),
                 ("F4", "Passage_verif_libre"),
                 ("A1", "Fin_verif")]),
        ("F6", &[("F1", "Fin_test"),                     // F6 sort vers F1 ou rentre dans A7
                 ("A7", "Retour_reglage")]),
    ];

    // ── Règles source ─────────────────────────────────────────────────────────
    // Pour chaque état sans transition entrante : liste ordonnée de (source, condition).
    // A1 et A6 sont exemptés (états initiaux légitimes sans entrée obligatoire).
    //
    // Fondées sur les circuits autorisés :
    //   D1←F1/F6 (circuits 4/7), D2←D1/F1 (circuit 4),
    //   D3←F1 (circuit 5),       A5←D1/D2 (circuit 4),
    //   A7←A5 (circuit 6),       F2←A1 (circuit 10), F3←F1 (circuit 11),
    //   F4←A1/F1 (circuit 8),    F5←A1/F1 (circuit 9),
    //   F6←A7/F1 (circuits 7) — jamais A1→F6 (interdit)
    let source_rules: &[(&str, &[(&str, &str)])] = &[
        ("A5", &[("D1", "Reset_direct"),
                 ("D2", "Acquit_defaut"),
                 ("F1", "Defaut")]),
        ("A7", &[("A5", "Mode_reglage"),
                 ("A1", "Mode_reglage")]),
        ("D1", &[("F1", "Defaut"),
                 ("F6", "AU")]),
        ("D2", &[("D1", "EU_relachee"),
                 ("F1", "Defaut_direct")]),
        ("D3", &[("F1", "Defaut_mineur")]),
        ("F2", &[("A1", "Mode_preparation")]),
        ("F3", &[("F1", "Mode_cloture")]),
        ("F4", &[("A1", "Mode_verif_libre"),
                 ("F1", "Mode_verif_libre")]),
        ("F5", &[("A1", "Mode_verif_seq"),
                 ("F1", "Mode_verif_seq")]),
        // F6 : entrée depuis A7 (préféré, circuit 7) ou F1 — jamais depuis A1 (interdit)
        ("F6", &[("A7", "Mode_test"),
                 ("F1", "Mode_test")]),
    ];

    let is_forbidden = |from: &str, to: &str| -> bool {
        FORBIDDEN.iter().any(|(f, t)| *f == from && *t == to)
    };

    // ── Passe 1 : états drain ─────────────────────────────────────────────────
    loop {
        let states_with_out: std::collections::HashSet<String> =
            gemma.transitions.iter().map(|t| t.from.clone()).collect();

        let drain = gemma.states.iter()
            .find(|s| !states_with_out.contains(&s.id))
            .map(|s| s.id.clone());

        let Some(sid) = drain else { break };

        let mut fixed = false;

        if let Some((_, candidates)) = drain_rules.iter().find(|(s, _)| *s == sid.as_str()) {
            for (target, cond) in *candidates {
                if is_forbidden(&sid, target) { continue; }
                if !gemma.states.iter().any(|s| s.id == *target) { continue; }
                let id = gemma.add_transition(sid.clone(), target.to_string(), Expr::from_str(cond));
                set_wpts(gemma, id, &sid, target, saved_routes);
                fixed = true;
                break;
            }
        }

        if !fixed { break; } // état drain sans règle applicable → stabilisation
    }

    // ── Passe 2 : états source non-initiaux ───────────────────────────────────
    loop {
        let states_with_in: std::collections::HashSet<String> =
            gemma.transitions.iter().map(|t| t.to.clone()).collect();

        // A1 et A6 sont des états de démarrage valides sans entrée obligatoire
        let source = gemma.states.iter()
            .find(|s| s.id != "A1" && s.id != "A6" && !states_with_in.contains(&s.id))
            .map(|s| s.id.clone());

        let Some(sid) = source else { break };

        let mut fixed = false;

        if let Some((_, candidates)) = source_rules.iter().find(|(s, _)| *s == sid.as_str()) {
            for (from, cond) in *candidates {
                if is_forbidden(from, &sid) { continue; }
                if !gemma.states.iter().any(|s| s.id == *from) { continue; }
                let id = gemma.add_transition(from.to_string(), sid.clone(), Expr::from_str(cond));
                set_wpts(gemma, id, from, &sid, saved_routes);
                fixed = true;
                break;
            }
        }

        if !fixed { break; } // état source sans règle applicable → stabilisation
    }
}

/// Déduit le StateType depuis l'identifiant GEMMA standard.
/// D* → Safety, A* → Command, F* → Production
fn state_type_from_id(id: &str) -> StateType {
    match id.chars().next() {
        Some('D') | Some('d') => StateType::Safety,
        Some('A') | Some('a') => StateType::Command,
        _ => StateType::Production,
    }
}

/// Géométrie canonique dérivée de states_model.py (Python canvas 1620×1020).
/// Facteurs d'échelle : sx=0.4321 (700/1620), sy=0.5392 (550/1020).
/// Retourne [centre_x, centre_y, largeur, hauteur].
fn canonical_geom(id: &str) -> Option<[f32; 4]> {
    // Format : [cx, cy, w, h]  (tout en unités canvas Rust)
    match id {
        //      cx      cy      w       h
        "A1" => Some([278.0,  80.0, 115.0,  40.0]),  // 266×75
        "A2" => Some([242.0, 287.0,  47.0, 115.0]),  // 109×213
        "A3" => Some([314.0, 273.0,  46.0,  87.0]),  // 107×161
        "A4" => Some([296.0, 168.0,  83.0,  52.0]),  // 192×96
        "A5" => Some([122.0, 287.0, 117.0, 115.0]),  // 270×213
        "A6" => Some([122.0,  80.0, 117.0,  45.0]),  // 270×84
        "A7" => Some([132.0, 168.0,  98.0,  52.0]),  // 226×96
        "D1" => Some([200.0, 474.0, 274.0,  45.0]),  // 633×84
        "D2" => Some([132.0, 392.0,  98.0,  47.0]),  // 226×87
        "D3" => Some([278.0, 392.0, 118.0,  47.0]),  // 274×87
        "F1" => Some([486.0, 334.0, 155.0, 165.0]),  // 359×305
        "F2" => Some([469.0, 185.0,  47.0,  83.0]),  // 109×153
        "F3" => Some([540.0, 185.0,  47.0,  83.0]),  // 109×153
        "F4" => Some([649.0,  84.0,  64.0,  78.0]),  // 147×144
        "F5" => Some([649.0, 262.0,  64.0, 208.0]),  // 147×386
        "F6" => Some([649.0, 445.0,  64.0, 104.0]),  // 147×192
        _    => None,
    }
}
