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
    next_step_id: u32,
    #[serde(default)]
    next_trans_id: u32,
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
        self.transitions.push(Transition::new(id, from_step, to_step));
        id
    }

    pub fn remove_transition(&mut self, id: u32) {
        self.transitions.retain(|t| t.id != id);
    }

    pub fn transition_mut(&mut self, id: u32) -> Option<&mut Transition> {
        self.transitions.iter_mut().find(|t| t.id == id)
    }
}
