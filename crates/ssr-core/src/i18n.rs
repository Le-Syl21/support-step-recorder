//! Langue de l'interface et des rapports (français / anglais / espagnol).
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
    /// Espagnol.
    Es,
}

impl Lang {
    /// Toutes les langues disponibles, dans l'ordre d'affichage du sélecteur.
    pub const ALL: [Lang; 3] = [Lang::Fr, Lang::En, Lang::Es];

    /// Détecte la langue depuis l'environnement (`LC_ALL`/`LANG`) : français si
    /// la locale commence par `fr`, espagnol par `es`, anglais sinon.
    pub fn detect() -> Self {
        let v = std::env::var("LC_ALL")
            .or_else(|_| std::env::var("LC_MESSAGES"))
            .or_else(|_| std::env::var("LANG"))
            .unwrap_or_default()
            .to_lowercase();
        if v.starts_with("fr") {
            Lang::Fr
        } else if v.starts_with("es") {
            Lang::Es
        } else {
            Lang::En
        }
    }

    /// Code ISO court (`"fr"` / `"en"`), pour l'attribut `lang` du HTML.
    pub fn code(self) -> &'static str {
        match self {
            Lang::Fr => "fr",
            Lang::En => "en",
            Lang::Es => "es",
        }
    }

    /// Retrouve la langue depuis son code (`"fr"`, `"en"`, `"es"`).
    pub fn from_code(code: &str) -> Option<Self> {
        Lang::ALL.into_iter().find(|l| l.code() == code)
    }

    /// Nom de la langue, dans cette langue (pour le sélecteur).
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::Fr => "Français",
            Lang::En => "English",
            Lang::Es => "Español",
        }
    }

    // --- Messages de progression (barre de statut) ---------------------------

    pub fn msg_capturing(self) -> &'static str {
        self.pick(
            "Capture de l'écran…",
            "Capturing screen…",
            "Capturando la pantalla…",
        )
    }
    pub fn msg_saving_image(self) -> &'static str {
        self.pick(
            "Sauvegarde de l'image…",
            "Saving image…",
            "Guardando la imagen…",
        )
    }
    pub fn msg_building_html(self) -> &'static str {
        self.pick("Création du HTML…", "Building HTML…", "Creando el HTML…")
    }
    pub fn msg_compressing(self) -> &'static str {
        self.pick("Compression…", "Compressing…", "Comprimiendo…")
    }

    /// Choisit la chaîne selon la langue (`fr`, `en`, `es`).
    pub fn pick(self, fr: &'static str, en: &'static str, es: &'static str) -> &'static str {
        match self {
            Lang::Fr => fr,
            Lang::En => en,
            Lang::Es => es,
        }
    }
}
