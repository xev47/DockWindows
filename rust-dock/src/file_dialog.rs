//! Boite de dialogue native pour choisir un fichier de configuration.

use std::path::PathBuf;

/// Ouvre la boite de dialogue native de choix de fichier.
///
/// `save` choisit l'enregistrement, sinon l'ouverture. Retourne `None` quand
/// l'utilisateur annule.
pub fn pick_json_file(save: bool, title: &str, suggested_name: &str) -> Option<PathBuf> {
    let dialog = rfd::FileDialog::new()
        .set_title(title)
        .add_filter("Dock Windows", &["json"]);
    if save {
        dialog.set_file_name(suggested_name).save_file()
    } else {
        dialog.pick_file()
    }
}
