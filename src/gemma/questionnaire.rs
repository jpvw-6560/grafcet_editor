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
    pub fn apply_to_gemma(&self, gemma: &mut Gemma) {
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
                });
            }
        }

        // 3. Ajouter les transitions (pas de doublon from+to)
        for (from, to, cond) in &trans_to_add {
            let already = gemma.transitions.iter().any(|t| &t.from == from && &t.to == to);
            if !already {
                let id = gemma.add_transition(from.clone(), to.clone(), Expr::from_str(cond));
                // Pré-remplir les waypoints depuis la table statique validée
                let wpts = super::static_gemma_waypoints(from, to);
                if !wpts.is_empty() {
                    if let Some(t) = gemma.transitions.iter_mut().find(|t| t.id == id) {
                        t.waypoints = wpts;
                    }
                }
            }
        }
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
