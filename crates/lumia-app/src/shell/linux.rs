use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub(super) fn register(exe_path: &Path) -> anyhow::Result<()> {
    let home = std::env::var("HOME")?;
    let applications_dir = PathBuf::from(&home).join(".local/share/applications");
    let icons_dir = PathBuf::from(&home).join(".local/share/icons/hicolor/128x128/apps");
    std::fs::create_dir_all(&applications_dir)?;
    std::fs::create_dir_all(&icons_dir)?;

    let desktop_content = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Lumia\n\
         Comment=Fast and lightweight image viewer\n\
         Exec={} %f\n\
         Icon=lumia\n\
         Terminal=false\n\
         Categories=Graphics;Viewer;\n\
         MimeType={}\n\
         NoDisplay=false\n\
         StartupNotify=false\n",
        exe_path.to_string_lossy(),
        mime_types()
    );
    std::fs::write(applications_dir.join("lumia.desktop"), desktop_content)?;
    install_icon(&icons_dir)?;

    let _ = std::process::Command::new("update-desktop-database")
        .arg(&applications_dir)
        .output();
    Ok(())
}

pub(super) fn unregister() -> anyhow::Result<()> {
    let home = std::env::var("HOME")?;
    let applications_dir = PathBuf::from(&home).join(".local/share/applications");
    let desktop_file = applications_dir.join("lumia.desktop");
    let icon_file = PathBuf::from(&home).join(".local/share/icons/hicolor/128x128/apps/lumia.png");

    if desktop_file.exists() {
        std::fs::remove_file(desktop_file)?;
    }
    if icon_file.exists() {
        std::fs::remove_file(icon_file)?;
    }
    let _ = std::process::Command::new("update-desktop-database")
        .arg(applications_dir)
        .output();
    Ok(())
}

fn mime_types() -> String {
    const MIME_MAP: &[(&str, &str)] = &[
        ("avif", "image/avif"),
        ("bmp", "image/bmp"),
        ("dds", "image/x-dds"),
        ("exr", "image/x-exr"),
        ("ff", "image/x-farbfeld"),
        ("farbfeld", "image/x-farbfeld"),
        ("gif", "image/gif"),
        ("hdr", "image/vnd.radiance"),
        ("heic", "image/heic"),
        ("heif", "image/heif"),
        ("ico", "image/vnd.microsoft.icon"),
        ("jpg", "image/jpeg"),
        ("jpeg", "image/jpeg"),
        ("pbm", "image/x-portable-bitmap"),
        ("pam", "image/x-portable-anymap"),
        ("ppm", "image/x-portable-pixmap"),
        ("pgm", "image/x-portable-graymap"),
        ("png", "image/png"),
        ("qoi", "image/x-qoi"),
        ("svg", "image/svg+xml"),
        ("tga", "image/x-tga"),
        ("tif", "image/tiff"),
        ("tiff", "image/tiff"),
        ("webp", "image/webp"),
    ];

    MIME_MAP
        .iter()
        .map(|(_, mime)| *mime)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(";")
}

fn install_icon(icons_dir: &Path) -> anyhow::Result<()> {
    let executable_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf));
    if let Some(base) = executable_dir {
        for candidate in [base.join("icon.png"), base.join("resources/icon.png")] {
            if candidate.exists() {
                std::fs::copy(candidate, icons_dir.join("lumia.png"))?;
                return Ok(());
            }
        }
    }

    const PLACEHOLDER: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];
    std::fs::write(icons_dir.join("lumia.png"), PLACEHOLDER)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::mime_types;

    #[test]
    fn mime_types_are_unique_and_sorted() {
        let values = mime_types();
        let parts = values.split(';').collect::<Vec<_>>();
        assert!(parts.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(parts.contains(&"image/png"));
        assert!(parts.contains(&"image/heic"));
    }
}
