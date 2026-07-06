//! Rapporteur de progression partagé entre threads : le cœur y écrit l'action
//! longue en cours (capture, sauvegarde, compression…), l'IHM la lit à chaque
//! frame pour l'afficher dans sa barre de statut.

use std::sync::{Arc, Mutex};

/// Poignée de progression. Cloner partage le même état sous-jacent, de sorte
/// qu'un thread de travail et l'IHM voient la même valeur.
#[derive(Clone, Default)]
pub struct Progress(Arc<Mutex<Option<String>>>);

impl Progress {
    /// Crée une poignée au repos (aucune action en cours).
    pub fn new() -> Self {
        Self::default()
    }

    /// Signale l'action longue en cours.
    pub fn set(&self, msg: impl Into<String>) {
        if let Ok(mut g) = self.0.lock() {
            *g = Some(msg.into());
        }
    }

    /// Repasse au repos (plus aucune action en cours).
    pub fn clear(&self) {
        if let Ok(mut g) = self.0.lock() {
            *g = None;
        }
    }

    /// Action en cours, s'il y en a une.
    pub fn get(&self) -> Option<String> {
        self.0.lock().ok().and_then(|g| g.clone())
    }
}
