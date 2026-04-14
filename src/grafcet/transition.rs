use serde::{Deserialize, Serialize};

/// Une transition GRAFCET : relie une étape source à une étape destination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    pub id: u32,
    pub from_step: u32,   // id de l'étape source
    pub to_step: u32,     // id de l'étape destination
    pub condition: String,
    #[serde(default)]
    pub pos: [f32; 2],    // position absolue de la barre (coordonnées logiques canvas)
    /// Override de la coordonnée Y (canvas) du segment horizontal src→barre.
    /// None = automatique (bas de l'étape source + STEP_WICK).
    #[serde(default)]
    pub route_y: Option<f32>,
    /// Override de la coordonnée X (canvas) du décrochage latéral pour les
    /// liaisons en retour (boucle vers le haut).
    /// None = automatique.
    #[serde(default)]
    pub dst_route_x: Option<f32>,
}

impl Transition {
    pub fn new(id: u32, from_step: u32, to_step: u32, pos: [f32; 2]) -> Self {
        Self {
            id,
            from_step,
            to_step,
            condition: "1".to_string(),
            pos,
            route_y: None,
            dst_route_x: None,
        }
    }
}
