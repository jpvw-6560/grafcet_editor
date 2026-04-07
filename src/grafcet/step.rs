use serde::{Deserialize, Serialize};

/// Type d'étape GRAFCET
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StepKind {
    Normal,
    Initial,   // double-bordure
    MacroStep, // étape-macro
}

impl Default for StepKind {
    fn default() -> Self {
        StepKind::Normal
    }
}

/// Une étape GRAFCET
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub id: u32,
    pub label: String,
    pub actions: Vec<String>,
    pub kind: StepKind,
    /// Position centre en coordonnées canvas (pixels)
    pub pos: [f32; 2],
    /// Active pendant la simulation
    #[serde(default)]
    pub active: bool,
}

impl Step {
    pub fn new(id: u32, pos: [f32; 2]) -> Self {
        Self {
            id,
            label: format!("E{id}"),
            actions: Vec::new(),
            kind: StepKind::Normal,
            pos,
            active: false,
        }
    }
}
