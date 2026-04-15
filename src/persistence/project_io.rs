// persistence/project_io.rs — save/load dans data/projets/{name}/
//
// Structure sur disque :
//   data/projets/{name}/project.json   ← métadonnées
//   data/projets/{name}/gemma.json     ← GEMMA (états + transitions)
//   data/projets/{name}/grafcets/GS.json
//   data/projets/{name}/grafcets/GC.json
//   data/projets/{name}/grafcets/GPN.json

use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

use crate::gemma::Gemma;
use crate::grafcet::Grafcet;
use crate::project::{NamedGrafcet, Project};

// ── Utilitaires chemins ────────────────────────────────────────────────────────

/// Sanitise un nom pour en faire un nom de dossier valide.
fn sanitize(name: &str) -> String {
    let s: String = name.trim()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    s.trim_matches('_').to_string()
}

/// Retourne `data/projets/{name}/`
pub fn project_dir(name: &str) -> PathBuf {
    PathBuf::from("data").join("projets").join(sanitize(name))
}

// ── Métadonnées du projet ──────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct ProjectMeta {
    name: String,
    description: String,
}

// ── Sauvegarde ─────────────────────────────────────────────────────────────────

pub fn save_project(project: &Project, dir: &Path) -> Result<(), String> {
    let grafcets_dir = dir.join("grafcets");

    std::fs::create_dir_all(dir)
        .map_err(|e| format!("Création du dossier projet : {e}"))?;
    std::fs::create_dir_all(&grafcets_dir)
        .map_err(|e| format!("Création grafcets/ : {e}"))?;

    // Métadonnées
    let meta = ProjectMeta { name: project.name.clone(), description: project.description.clone() };
    let json = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Sérialisation meta : {e}"))?;
    std::fs::write(dir.join("project.json"), json)
        .map_err(|e| format!("Écriture project.json : {e}"))?;

    // GEMMA — directement dans le dossier projet
    let gemma_json = serde_json::to_string_pretty(&project.gemma)
        .map_err(|e| format!("Sérialisation GEMMA : {e}"))?;
    std::fs::write(dir.join("gemma.json"), gemma_json)
        .map_err(|e| format!("Écriture gemma.json : {e}"))?;

    // Grafcets — un fichier par grafcet dans grafcets/
    for ng in &project.grafcets {
        let fname = format!("{}.json", sanitize(&ng.name));
        let json = serde_json::to_string_pretty(&ng.grafcet)
            .map_err(|e| format!("Sérialisation {} : {e}", ng.name))?;
        std::fs::write(grafcets_dir.join(&fname), json)
            .map_err(|e| format!("Écriture {fname} : {e}"))?;
    }

    Ok(())
}

// ── Chargement ─────────────────────────────────────────────────────────────────

pub fn load_project(path: &Path) -> Result<Project, String> {
    // Accepter un fichier project.json ou directement un dossier
    let dir: &Path = if path.is_file() {
        path.parent().ok_or("Chemin invalide")?
    } else {
        path
    };

    // Métadonnées
    let meta_path = dir.join("project.json");
    let meta_json = std::fs::read_to_string(&meta_path)
        .map_err(|e| format!("Lecture project.json : {e}"))?;
    let meta: ProjectMeta = serde_json::from_str(&meta_json)
        .map_err(|e| format!("Parsing project.json : {e}"))?;

    // GEMMA — cherche d'abord le nouvel emplacement plat, puis l'ancien (gemmas/)
    let gemma_path_flat = dir.join("gemma.json");
    let gemma_path_old  = dir.join("gemmas").join("gemma.json");
    let gemma: Gemma = if gemma_path_flat.exists() {
        let json = std::fs::read_to_string(&gemma_path_flat)
            .map_err(|e| format!("Lecture gemma.json : {e}"))?;
        serde_json::from_str(&json).map_err(|e| format!("Parsing gemma.json : {e}"))?
    } else if gemma_path_old.exists() {
        let json = std::fs::read_to_string(&gemma_path_old)
            .map_err(|e| format!("Lecture gemma.json (legacy) : {e}"))?;
        serde_json::from_str(&json).map_err(|e| format!("Parsing gemma.json (legacy) : {e}"))?
    } else {
        Gemma::new()
    };

    // Grafcets
    let grafcets_dir = dir.join("grafcets");
    let mut grafcets: Vec<NamedGrafcet> = Vec::new();
    if grafcets_dir.exists() {
        let mut entries: Vec<_> = std::fs::read_dir(&grafcets_dir)
            .map_err(|e| format!("Lecture grafcets/ : {e}"))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
            .collect();

        entries.sort_by_key(|e| {
            let n = e.path().file_stem().unwrap_or_default().to_string_lossy().to_string();
            let order: u32 = match n.as_str() { "GS" => 0, "GC" => 1, "GPN" => 2, _ => 99 };
            (order, n)
        });

        for entry in entries {
            let path = entry.path();
            let name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
            let json = std::fs::read_to_string(&path)
                .map_err(|e| format!("Lecture {name}.json : {e}"))?;
            let grafcet: Grafcet = serde_json::from_str(&json)
                .map_err(|e| format!("Parsing {name}.json : {e}"))?;
            grafcets.push(NamedGrafcet { name, short_name: None, grafcet, generated: false });
        }
    }



    Ok(Project { name: meta.name, description: meta.description, gemma, grafcets })
}
