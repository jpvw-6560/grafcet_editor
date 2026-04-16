// project/mod.rs — Modèle de projet
//
// Un projet contient :
//  - un nom, un dossier de persistance
//  - un GEMMA (diagramme de modes)
//  - une liste de Grafcets nommés (GS, GC, GPN + extras)

use serde::{Deserialize, Serialize};
use crate::grafcet::Grafcet;
use crate::gemma::Gemma;

/// Un Grafcet nommé dans le projet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedGrafcet {
    pub name: String,
    /// Nom court symbolique affiché dans l'onglet (ex : "A1→F1").
    /// `None` pour les grafcets manuels (nom complet affiché).
    #[serde(default)]
    pub short_name: Option<String>,
    /// Description complète du circuit (affichée en hover sur l'onglet).
    /// Pour les grafcets générés depuis le GEMMA : "Production continue | A1→F1"
    #[serde(default)]
    pub description: Option<String>,
    pub grafcet: Grafcet,
    /// Vrai si généré automatiquement depuis le GEMMA :
    /// affiché en JSON pur, pas encore de canvas.
    #[serde(default)]
    pub generated: bool,
}

impl NamedGrafcet {
    pub fn new(name: impl Into<String>) -> Self {
        use crate::grafcet::StepKind;
        let mut grafcet = Grafcet::new();
        let id = grafcet.add_step([400.0, 100.0]);
        if let Some(s) = grafcet.step_mut(id) {
            s.kind = StepKind::Initial;
        }
        Self { name: name.into(), short_name: None, description: None, grafcet, generated: false }
    }
}

/// Projet complet.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub description: String,
    /// Documentation libre du projet (page Markdown).
    #[serde(default)]
    pub documentation: String,
    pub gemma: Gemma,
    pub grafcets: Vec<NamedGrafcet>,
}

impl Project {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            name: name.clone(),
            description: String::new(),
            documentation: String::new(),
            gemma: Gemma { name: name.clone(), ..Gemma::new() },
            grafcets: Vec::new(),
        }
    }

    pub fn add_grafcet(&mut self, name: impl Into<String>) -> usize {
        self.grafcets.push(NamedGrafcet::new(name));
        self.grafcets.len() - 1
    }

    pub fn grafcet_mut(&mut self, idx: usize) -> Option<&mut NamedGrafcet> {
        self.grafcets.get_mut(idx)
    }
}
