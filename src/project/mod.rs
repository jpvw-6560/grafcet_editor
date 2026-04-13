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
    pub grafcet: Grafcet,
}

impl NamedGrafcet {
    pub fn new(name: impl Into<String>) -> Self {
        use crate::grafcet::StepKind;
        let mut grafcet = Grafcet::new();
        let id = grafcet.add_step([400.0, 100.0]);
        if let Some(s) = grafcet.step_mut(id) {
            s.kind = StepKind::Initial;
        }
        Self { name: name.into(), grafcet }
    }
}

/// Projet complet.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub description: String,
    pub gemma: Gemma,
    pub grafcets: Vec<NamedGrafcet>,
}

impl Project {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let mut p = Self {
            name: name.clone(),
            description: String::new(),
            gemma: Gemma { name: name.clone(), ..Gemma::new() },
            grafcets: Vec::new(),
        };
        // Trois grafcets canoniques vides
        p.grafcets.push(NamedGrafcet::new("GS"));
        p.grafcets.push(NamedGrafcet::new("GC"));
        p.grafcets.push(NamedGrafcet::new("GPN"));
        p
    }

    pub fn add_grafcet(&mut self, name: impl Into<String>) -> usize {
        self.grafcets.push(NamedGrafcet::new(name));
        self.grafcets.len() - 1
    }

    pub fn grafcet_mut(&mut self, idx: usize) -> Option<&mut NamedGrafcet> {
        self.grafcets.get_mut(idx)
    }
}
