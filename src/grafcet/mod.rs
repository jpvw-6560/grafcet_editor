pub mod step;
pub mod transition;

pub use step::{Step, StepKind};
pub use transition::Transition;

use serde::{Deserialize, Serialize};

/// Modèle de données complet d'un grafcet
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Grafcet {
    pub steps: Vec<Step>,
    pub transitions: Vec<Transition>,
    #[serde(default)]
    pub next_step_id: u32,
    #[serde(default)]
    pub next_trans_id: u32,
    /// Compteur pour créer des groupes ET uniques.
    #[serde(default)]
    pub next_and_group_id: u32,
}

impl Grafcet {
    pub fn new() -> Self {
        Self::default()
    }

    // ── Étapes ─────────────────────────────────────────────────────────────

    pub fn add_step(&mut self, pos: [f32; 2]) -> u32 {
        let id = self.next_step_id;
        self.next_step_id += 1;
        self.steps.push(Step::new(id, pos));
        id
    }

    pub fn remove_step(&mut self, id: u32) {
        self.steps.retain(|s| s.id != id);
        self.transitions
            .retain(|t| t.from_step != id && t.to_step != id);
    }

    pub fn step_mut(&mut self, id: u32) -> Option<&mut Step> {
        self.steps.iter_mut().find(|s| s.id == id)
    }

    pub fn step(&self, id: u32) -> Option<&Step> {
        self.steps.iter().find(|s| s.id == id)
    }

    // ── Transitions ────────────────────────────────────────────────────────

    pub fn add_transition(&mut self, from_step: u32, to_step: u32) -> u32 {
        let id = self.next_trans_id;
        self.next_trans_id += 1;
        // Divergence en OU : si d'autres transitions partent déjà de cette étape,
        // on décale en X de 100 px par transition existante.
        let siblings = self.transitions.iter().filter(|t| t.from_step == from_step).count();
        let x_off = siblings as f32 * 100.0;
        let pos = if let Some(src) = self.step(from_step) {
            [src.pos[0] + x_off, src.pos[1] + 95.0]
        } else {
            [200.0 + x_off, 200.0]
        };
        self.transitions.push(Transition::new(id, from_step, to_step, pos));
        id
    }

    pub fn transition(&self, id: u32) -> Option<&Transition> {
        self.transitions.iter().find(|t| t.id == id)
    }

    pub fn remove_transition(&mut self, id: u32) {
        self.transitions.retain(|t| t.id != id);
    }

    pub fn transition_mut(&mut self, id: u32) -> Option<&mut Transition> {
        self.transitions.iter_mut().find(|t| t.id == id)
    }

    /// Crée un nouvel identifiant de groupe ET (unique dans ce grafcet).
    pub fn new_and_group(&mut self) -> u32 {
        let id = self.next_and_group_id;
        self.next_and_group_id += 1;
        id
    }
}
