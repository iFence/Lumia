use crate::{dialog, embedded::ExtractedMsi, installer, legacy};

pub(crate) fn run() {
    let Some(language) = dialog::choose_language() else {
        return;
    };
    if let Err(error) = run_selected(language) {
        dialog::show_error(language, &error);
    }
}

fn run_selected(language: dialog::Language) -> anyhow::Result<()> {
    let legacy_products = legacy::installed_per_machine_products()?;
    if !legacy_products.is_empty() {
        if !dialog::confirm_legacy_migration(language) {
            return Ok(());
        }
        for product in legacy_products {
            legacy::uninstall(&product)?;
        }
    }

    let msi = ExtractedMsi::extract(language.msi_resource(), language.msi_file_name())?;
    installer::install(msi.path())?;
    if let Err(error) = installer::repair_file_associations() {
        dialog::show_repair_warning(language, &error);
    }
    Ok(())
}
