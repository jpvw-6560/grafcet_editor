# Éditeur GRAFCET — Rust / egui

Éditeur graphique de GRAFCET (Graphe Fonctionnel de Commande Étape-Transition), développé en **Rust** avec l'interface graphique **egui/eframe**.

Ce projet est la refonte de `gemma_editor` (Python/PyQt6), abandonné en raison de la complexité du layout automatique et des limitations de performance. L'approche Rust offre : performances natives, déploiement binaire unique, et un canvas custom précis.

---

## Objectif

Créer un éditeur GRAFCET professionnel multiplateforme, inspiré de **CADEPA**, permettant :

- L'édition graphique complète d'un GRAFCET (étapes, transitions, liaisons)
- La simulation en temps réel (franchissement de transitions, activation d'étapes)
- La génération de GEMMA (Guide d'Étude des Modes de Marches et d'Arrêts)
- La communication avec des automates (ESP32, PLC) via WebSocket / TCP

---

## Feuille de route

### ✅ Phase 1 — Éditeur de base (en cours)
- [x] Projet Rust initialisé (egui 0.34 / eframe)
- [x] Modèle de données : `Step`, `Transition`, `Grafcet`
- [x] Canvas interactif avec grille
- [x] Création d'étapes par clic
- [x] Drag & drop des étapes
- [x] Création de transitions (clic source → clic destination)
- [x] Suppression d'étapes et de transitions
- [x] Zoom molette + pan bouton milieu
- [x] Panneau de propriétés (label, type, actions, conditions)
- [x] Sauvegarde / chargement JSON
- [x] Étape initiale (double bordure)

### 🔲 Phase 2 — Qualité éditeur
- [ ] Alignement à la grille (snap-to-grid)
- [ ] Sélection multiple + déplacement groupé
- [ ] Undo / Redo (historique des actions)
- [ ] Copier / Coller des étapes
- [ ] Validation GRAFCET (détection d'erreurs structurelles)
- [ ] Export image PNG / SVG
- [ ] Étapes macro (sous-grafcet)
- [ ] Divergence ET / Convergence ET
- [ ] Divergence OU / Convergence OU

### 🔲 Phase 3 — Simulation
- [ ] Moteur d'exécution GRAFCET (cycle scan)
- [ ] Activation / désactivation des étapes en temps réel
- [ ] Évaluation des réceptivités (expressions booléennes)
- [ ] Affichage des étapes actives en surbrillance
- [ ] Panneau de forçage des entrées (simulation manuelle)
- [ ] Chronomètre par étape (temporisations)

### 🔲 Phase 4 — GEMMA
- [ ] Génération automatique du GS (Guide de Sécurité)
- [ ] Génération du GC (Guide de Conduite)
- [ ] Génération du GPN (Guide de Production Normale)
- [ ] Visualisation GEMMA intégrée (onglets)

### 🔲 Phase 5 — Communication
- [ ] Communication WebSocket avec ESP32
- [ ] Protocole Modbus TCP (automates industriels)
- [ ] Supervision en temps réel (états entrées/sorties)
- [ ] Synchronisation grafcet ↔ automate

---

## Architecture du projet

```
grafcet_editor/
├── Cargo.toml
└── src/
    ├── main.rs                  # Point d'entrée, configuration eframe
    ├── grafcet/
    │   ├── mod.rs               # Grafcet : add/remove étapes et transitions
    │   ├── step.rs              # Step : id, label, actions, position, kind
    │   └── transition.rs        # Transition : from_step, to_step, condition
    ├── gui/
    │   ├── mod.rs
    │   ├── canvas.rs            # Rendu : draw_steps(), draw_links(), grille
    │   └── editor.rs            # App egui : boucle UI, outils, pan/zoom
    └── persistence/
        ├── mod.rs
        └── json_io.rs           # save_json() / load_json()
```

---

## Dépendances

| Crate | Version | Usage |
|---|---|---|
| `eframe` | 0.34 | Fenêtre native multiplateforme |
| `egui` | 0.34 | Interface graphique (immediate mode) |
| `serde` + `serde_json` | 1.x | Sérialisation JSON |
| `rfd` | 0.17 | Boîtes de dialogue fichiers natives |

---

## Démarrage rapide

### Prérequis
- Rust stable ≥ 1.75 ([rustup.rs](https://rustup.rs))

### Compiler et lancer

```bash
git clone https://github.com/jpvw-6560/grafcet_editor.git
cd grafcet_editor
cargo run
```

### Raccourcis clavier

| Touche | Action |
|---|---|
| `Échap` | Retour à l'outil Sélection |
| `Ctrl+S` | Enregistrer |
| Molette | Zoom |
| Bouton du milieu + glisser | Pan |

---

## Utilisation

1. **Ajouter une étape** : sélectionner l'outil ⬛ puis cliquer sur le canvas
2. **Déplacer une étape** : outil ↖ + glisser
3. **Créer une transition** : outil ↕ → cliquer l'étape source, puis la destination
4. **Modifier le label / les actions** : cliquer sur une étape pour la sélectionner, éditer dans le panneau de droite
5. **Supprimer** : outil 🗑 puis cliquer sur l'élément
6. **Sauvegarder** : menu Fichier → Enregistrer (format JSON)

---

## Licence

MIT
