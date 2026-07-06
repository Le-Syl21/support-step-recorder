//! Langue de l'interface et des rapports (français / anglais).
//!
//! Le cœur produit quelques chaînes destinées à l'utilisateur (descriptions
//! d'étapes, messages de progression, rapport HTML). Elles sont construites au
//! moment de l'affichage/export selon la [`Lang`] choisie, plutôt que figées.

/// Langue d'affichage.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Lang {
    /// Français.
    #[default]
    Fr,
    /// Anglais.
    En,
}

impl Lang {
    /// Détecte la langue depuis l'environnement (`LC_ALL`/`LANG`) ; français par
    /// défaut si la locale commence par `fr`, anglais sinon.
    pub fn detect() -> Self {
        let v = std::env::var("LC_ALL")
            .or_else(|_| std::env::var("LC_MESSAGES"))
            .or_else(|_| std::env::var("LANG"))
            .unwrap_or_default()
            .to_lowercase();
        if v.starts_with("fr") {
            Lang::Fr
        } else {
            Lang::En
        }
    }

    /// Code ISO court (`"fr"` / `"en"`), pour l'attribut `lang` du HTML.
    pub fn code(self) -> &'static str {
        match self {
            Lang::Fr => "fr",
            Lang::En => "en",
        }
    }

    /// Bascule vers l'autre langue.
    pub fn toggled(self) -> Self {
        match self {
            Lang::Fr => Lang::En,
            Lang::En => Lang::Fr,
        }
    }

    // --- Messages de progression (barre de statut) ---------------------------

    pub fn msg_capturing(self) -> &'static str {
        self.pick("Capture de l'écran…", "Capturing screen…")
    }
    pub fn msg_saving_image(self) -> &'static str {
        self.pick("Sauvegarde de l'image…", "Saving image…")
    }
    pub fn msg_building_html(self) -> &'static str {
        self.pick("Création du HTML…", "Building HTML…")
    }
    pub fn msg_compressing(self) -> &'static str {
        self.pick("Compression…", "Compressing…")
    }

    /// Choisit la chaîne selon la langue (`fr`, `en`).
    pub fn pick(self, fr: &'static str, en: &'static str) -> &'static str {
        match self {
            Lang::Fr => fr,
            Lang::En => en,
        }
    }
}
