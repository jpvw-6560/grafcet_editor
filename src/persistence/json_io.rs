use crate::grafcet::Grafcet;
use std::path::Path;

/// Sauvegarde le grafcet dans un fichier JSON.
pub fn save_json(grafcet: &Grafcet, path: &Path) -> Result<(), String> {
    let json = serde_json::to_string_pretty(grafcet)
        .map_err(|e| format!("Erreur sérialisation : {e}"))?;
    std::fs::write(path, json).map_err(|e| format!("Erreur écriture fichier : {e}"))
}

/// Charge un grafcet depuis un fichier JSON.
pub fn load_json(path: &Path) -> Result<Grafcet, String> {
    let data = std::fs::read_to_string(path)
        .map_err(|e| format!("Erreur lecture fichier : {e}"))?;
    serde_json::from_str(&data).map_err(|e| format!("Erreur parsing JSON : {e}"))
}
