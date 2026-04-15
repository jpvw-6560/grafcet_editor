// gemma/mod.rs — Modèle AST GEMMA (Guide d'Étude des Modes de Marches et d'Arrêts)
//
// Calqué sur le document "generateur_rust_complet.md"
// Trois types d'états : Safety / Command / Production
// Les transitions portent des Expr (DSL booléen) plutôt que de simples chaînes.

pub mod questionnaire;

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// ── DSL conditions (§3) ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "args")]
pub enum Expr {
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Var(String),
    True,
    False,
    /// Timer terminé : `T#<nom>`
    TimerDone(String),
}

impl Expr {
    /// Convertit une chaîne simple en `Var(s)` ; façade pratique pour l'UI.
    pub fn from_str(s: &str) -> Self {
        match s.trim() {
            "" | "false" | "FALSE" => Expr::False,
            "true" | "TRUE" | "1" => Expr::True,
            other => Expr::Var(other.to_string()),
        }
    }

    /// Retourne une représentation texte (pour affichage / édition dans l'UI).
    pub fn to_display(&self) -> String {
        match self {
            Expr::And(a, b) => format!("({}) AND ({})", a.to_display(), b.to_display()),
            Expr::Or(a, b) => format!("({}) OR ({})", a.to_display(), b.to_display()),
            Expr::Not(e) => format!("NOT ({})", e.to_display()),
            Expr::Var(v) => v.clone(),
            Expr::True => "TRUE".into(),
            Expr::False => "FALSE".into(),
            Expr::TimerDone(t) => format!("T#{t}"),
        }
    }

    /// Génération Structured Text (§15).
    pub fn to_st(&self) -> String {
        match self {
            Expr::And(a, b) => format!("({}) AND ({})", a.to_st(), b.to_st()),
            Expr::Or(a, b) => format!("({}) OR ({})", a.to_st(), b.to_st()),
            Expr::Not(e) => format!("NOT ({})", e.to_st()),
            Expr::Var(v) => v.clone(),
            Expr::True => "TRUE".into(),
            Expr::False => "FALSE".into(),
            Expr::TimerDone(t) => format!("TON_{t}.Q"),
        }
    }
}

// ── Types d'états (§2) ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StateType {
    Safety,     // GS — Grafcet de Sécurité
    Command,    // GC — Grafcet de Commande
    Production, // GPN — Grafcet de Production Normale
}

impl StateType {
    pub fn label(self) -> &'static str {
        match self {
            StateType::Safety => "Sécurité",
            StateType::Command => "Commande",
            StateType::Production => "Production",
        }
    }

    pub fn color(self) -> egui::Color32 {
        match self {
            StateType::Safety     => egui::Color32::from_rgb(180, 50, 50),
            StateType::Command    => egui::Color32::from_rgb(50, 100, 180),
            StateType::Production => egui::Color32::from_rgb(40, 140, 70),
        }
    }
}

// ── État GEMMA (§2) ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GemmaState {
    pub id: String,
    pub state_type: StateType,
    /// Position centre sur le canvas GEMMA [x, y]
    #[serde(default)]
    pub pos: [f32; 2],
    /// Largeur canonique (0 = utiliser NODE_W par défaut)
    #[serde(default)]
    pub w: f32,
    /// Hauteur canonique (0 = utiliser NODE_H par défaut)
    #[serde(default)]
    pub h: f32,
    /// Description courte
    #[serde(default)]
    pub description: String,
}

// ── Transition GEMMA (§2) ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GemmaTransition {
    pub id: u32,
    pub from: String,
    pub to: String,
    pub condition: Expr,
    /// Points du chemin orthogonal en coordonnées canvas.
    /// Vide = routage automatique (static_gemma_route ou fallback L).
    #[serde(default)]
    pub waypoints: Vec<[f32; 2]>,
}

// ── GEMMA complet (§2) ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Gemma {
    pub states: Vec<GemmaState>,
    pub transitions: Vec<GemmaTransition>,
    #[serde(default)]
    pub next_trans_id: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
}

impl Gemma {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_transition(&mut self, from: String, to: String, cond: Expr) -> u32 {
        let id = self.next_trans_id;
        self.next_trans_id += 1;
        self.transitions.push(GemmaTransition { id, from, to, condition: cond, waypoints: Vec::new() });
        id
    }

    pub fn state(&self, id: &str) -> Option<&GemmaState> {
        self.states.iter().find(|s| s.id == id)
    }

    pub fn state_mut(&mut self, id: &str) -> Option<&mut GemmaState> {
        self.states.iter_mut().find(|s| s.id == id)
    }

    // ── Validation (§5) ──────────────────────────────────────────────────────

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        let ids: HashSet<&str> = self.states.iter().map(|s| s.id.as_str()).collect();

        for t in &self.transitions {
            if !ids.contains(t.from.as_str()) {
                errors.push(format!("Transition {}: état source '{}' inconnu", t.id, t.from));
            }
            if !ids.contains(t.to.as_str()) {
                errors.push(format!("Transition {}: état destination '{}' inconnu", t.id, t.to));
            }
        }

        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    // ── Partitionnement (§6) ──────────────────────────────────────────────────

    pub fn safety_states(&self) -> Vec<&GemmaState> {
        self.states.iter().filter(|s| s.state_type == StateType::Safety).collect()
    }

    pub fn command_states(&self) -> Vec<&GemmaState> {
        self.states.iter().filter(|s| s.state_type == StateType::Command).collect()
    }

    pub fn production_states(&self) -> Vec<&GemmaState> {
        self.states.iter().filter(|s| s.state_type == StateType::Production).collect()
    }
}

// ── Routes statiques (waypoints pré-validés visuellement) ────────────────────

/// Retourne les waypoints canvas pré-validés pour la transition `from_id`→`to_id`.
/// Vecteur vide si la route n'est pas connue.
/// Ces coordonnées correspondent au canvas Rust calibré à 700×550
/// (facteurs 0.4321×0.5392 depuis le canvas Python 1620×1020).
pub fn static_gemma_waypoints(from_id: &str, to_id: &str) -> Vec<[f32; 2]> {
    match (from_id, to_id) {
        // Zone A → zone F (demandes de marche)
        ("A1", "F1") => vec![[336., 80.],[395., 80.],[395.,334.],[409.,334.]],
        ("A1", "F2") => vec![[336., 80.],[395., 80.],[395.,144.],[446.,144.]],
        ("A1", "F4") => vec![[278., 60.],[278., 32.],[649., 32.],[649., 45.]],
        ("A1", "F5") => vec![[336., 80.],[593., 80.],[593.,262.],[617.,262.]],
        ("A4", "F1") => vec![[338.,168.],[395.,168.],[395.,334.],[409.,334.]],
        // Zone A interne
        ("A2", "A1") => vec![[242.,230.],[242.,100.]],
        ("A3", "A4") => vec![[314.,230.],[314.,194.]],
        ("A4", "A6") => vec![[255.,168.],[191.,168.],[191., 80.],[181., 80.]],
        ("A5", "A6") => vec![[ 64.,287.],[ 55.,287.],[ 55., 80.],[ 64., 80.]],
        ("A5", "A7") => vec![[122.,230.],[122.,194.]],
        ("A6", "A1") => vec![[181., 80.],[221., 80.]],
        ("A7", "A4") => vec![[181.,168.],[255.,168.]],
        ("A7", "A6") => vec![[132.,142.],[132.,103.]],
        // Zone F → zone A (demandes d'arrêt)
        ("F1", "A2") => vec![[409.,334.],[352.,334.],[352.,287.],[266.,287.]],
        ("F1", "A3") => vec![[409.,334.],[361.,334.],[361.,273.],[337.,273.]],
        // Zone F → zone D (détections défaillances)
        ("F1", "D1") => vec![[486.,417.],[486.,437.],[200.,437.],[200.,452.]],
        ("F1", "D2") => vec![[409.,334.],[352.,334.],[352.,350.],[181.,350.],[181.,369.]],
        ("F1", "D3") => vec![[409.,334.],[361.,334.],[361.,392.],[337.,392.]],
        // Zone F interne
        ("F1", "F3") => vec![[540.,252.],[540.,227.]],
        ("F1", "F4") => vec![[564.,334.],[583.,334.],[583., 84.],[617., 84.]],
        ("F1", "F5") => vec![[564.,334.],[593.,334.],[593.,262.],[617.,262.]],
        ("F1", "F6") => vec![[564.,417.],[593.,417.],[593.,445.],[617.,445.]],
        ("F2", "F1") => vec![[469.,227.],[469.,252.]],
        ("F3", "A1") => vec![[540.,144.],[540., 22.],[278., 22.],[278., 60.]],
        ("F4", "A6") => vec![[649., 45.],[649., 22.],[181., 22.],[181., 80.]],
        ("F5", "F1") => vec![[617.,262.],[583.,262.],[583.,334.],[564.,334.]],
        ("F5", "F4") => vec![[649.,158.],[649.,123.]],
        ("F6", "F1") => vec![[617.,445.],[593.,445.],[593.,334.],[564.,334.]],
        ("F6", "D1") => vec![[649.,497.],[649.,510.],[337.,510.],[337.,497.]],
        // Zone D
        ("D1", "A5") => vec![[ 63.,474.],[ 48.,474.],[ 48.,287.],[ 64.,287.]],
        ("D1", "D2") => vec![[132.,452.],[132.,416.]],
        ("D2", "A5") => vec![[ 83.,392.],[ 40.,392.],[ 40.,287.],[ 64.,287.]],
        ("D3", "D2") => vec![[219.,392.],[181.,392.]],
        ("D3", "A2") => vec![[242.,369.],[242.,345.]],
        ("D3", "A3") => vec![[314.,369.],[314.,317.]],
        _ => vec![],
    }
}

// ── Routes sauvegardées (gemma_routes.json) ───────────────────────────────────

/// Table (from, to) → (waypoints, condition_string).
pub type SavedRoutes = std::collections::HashMap<(String, String), (Vec<[f32; 2]>, String)>;

/// Charge `data/gemma_routes.json` et retourne la table des routes validées.
/// Retourne une table vide si le fichier est absent ou invalide.
pub fn load_saved_routes() -> SavedRoutes {
    #[derive(serde::Deserialize)]
    struct Entry {
        from: String,
        to: String,
        #[serde(default)]
        points: Vec<[f32; 2]>,
        #[serde(default)]
        condition: String,
    }
    let content = match std::fs::read_to_string("data/gemma_routes.json") {
        Ok(s)  => s,
        Err(_) => return SavedRoutes::new(),
    };
    let entries: Vec<Entry> = match serde_json::from_str(&content) {
        Ok(v)  => v,
        Err(_) => return SavedRoutes::new(),
    };
    entries.into_iter()
        .map(|e| ((e.from, e.to), (e.points, e.condition)))
        .collect()
}

// ── Extraction des circuits fermés ────────────────────────────────────────────

/// Extrait tous les circuits fermés simples du GEMMA par DFS.
/// Retourne chaque circuit comme liste ordonnée de state_id ; le retour vers
/// le premier état est implicite (circuit fermé).
/// Chaque circuit est normalisé pour démarrer par le nœud d'indice minimal
/// dans `gemma.states`, ce qui garantit l'unicité sans rotation.
pub fn extract_closed_circuits(gemma: &Gemma) -> Vec<Vec<String>> {
    let state_ids: Vec<String> = gemma.states.iter().map(|s| s.id.clone()).collect();
    let idx_map: std::collections::HashMap<String, usize> = state_ids
        .iter()
        .enumerate()
        .map(|(i, s)| (s.clone(), i))
        .collect();

    // Adjacence : from → [to, ...]
    let mut adj: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for t in &gemma.transitions {
        adj.entry(t.from.clone()).or_default().push(t.to.clone());
    }

    let mut circuits: Vec<Vec<String>> = Vec::new();

    for (start_idx, start) in state_ids.iter().enumerate() {
        let mut path = vec![start.clone()];
        circuits_dfs(start, start, start_idx, &adj, &idx_map, &mut path, &mut circuits);
    }

    circuits
}

fn circuits_dfs(
    current: &str,
    start: &str,
    start_idx: usize,
    adj: &std::collections::HashMap<String, Vec<String>>,
    idx_map: &std::collections::HashMap<String, usize>,
    path: &mut Vec<String>,
    circuits: &mut Vec<Vec<String>>,
) {
    // Cloner pour ne pas garder d'emprunt sur `adj` pendant la récursion
    let neighbors: Vec<String> = adj.get(current).cloned().unwrap_or_default();
    for next in &neighbors {
        if next.as_str() == start && path.len() >= 2 {
            // Boucle fermée → circuit trouvé
            circuits.push(path.clone());
        } else if next.as_str() != start {
            if let Some(&ni) = idx_map.get(next.as_str()) {
                // On ne visite que les nœuds d'indice > start_idx pour éviter
                // de trouver le même cycle depuis plusieurs nœuds de départ.
                if ni > start_idx && !path.contains(next) {
                    path.push(next.clone());
                    circuits_dfs(next, start, start_idx, adj, idx_map, path, circuits);
                    path.pop();
                }
            }
        }
    }
}
