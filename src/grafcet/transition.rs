use serde::{Deserialize, Serialize};

/// Une transition GRAFCET : relie une étape source à une étape destination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    pub id: u32,
    pub from_step: u32,  // id de l'étape source
    pub to_step: u32,    // id de l'étape destination
    pub condition: String,
}

impl Transition {
    pub fn new(id: u32, from_step: u32, to_step: u32) -> Self {
        Self {
            id,
            from_step,
            to_step,
            condition: "1".to_string(),
        }
    }
}
