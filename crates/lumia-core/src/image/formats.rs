#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageFormatGroup {
    pub id: &'static str,
    pub name: &'static str,
    pub extensions: &'static [&'static str],
}

pub const SUPPORTED_IMAGE_FORMAT_GROUPS: &[ImageFormatGroup] = &[
    ImageFormatGroup {
        id: "avif",
        name: "AVIF",
        extensions: &["avif"],
    },
    ImageFormatGroup {
        id: "jpeg",
        name: "JPEG",
        extensions: &["jpg", "jpeg"],
    },
    ImageFormatGroup {
        id: "png",
        name: "PNG",
        extensions: &["png"],
    },
    ImageFormatGroup {
        id: "gif",
        name: "GIF",
        extensions: &["gif"],
    },
    ImageFormatGroup {
        id: "webp",
        name: "WebP",
        extensions: &["webp"],
    },
    ImageFormatGroup {
        id: "tiff",
        name: "TIFF",
        extensions: &["tif", "tiff"],
    },
    ImageFormatGroup {
        id: "tga",
        name: "TGA",
        extensions: &["tga"],
    },
    ImageFormatGroup {
        id: "dds",
        name: "DDS",
        extensions: &["dds"],
    },
    ImageFormatGroup {
        id: "bmp",
        name: "BMP",
        extensions: &["bmp"],
    },
    ImageFormatGroup {
        id: "ico",
        name: "ICO",
        extensions: &["ico"],
    },
    ImageFormatGroup {
        id: "hdr",
        name: "HDR",
        extensions: &["hdr"],
    },
    ImageFormatGroup {
        id: "exr",
        name: "OpenEXR",
        extensions: &["exr"],
    },
    ImageFormatGroup {
        id: "pnm",
        name: "Netpbm",
        extensions: &["pbm", "pam", "ppm", "pgm"],
    },
    ImageFormatGroup {
        id: "farbfeld",
        name: "Farbfeld",
        extensions: &["ff", "farbfeld"],
    },
    ImageFormatGroup {
        id: "qoi",
        name: "QOI",
        extensions: &["qoi"],
    },
    ImageFormatGroup {
        id: "svg",
        name: "SVG",
        extensions: &["svg"],
    },
    ImageFormatGroup {
        id: "heif",
        name: "HEIF",
        extensions: &["heic", "heif"],
    },
    ImageFormatGroup {
        id: "photoshop",
        name: "Adobe Photoshop",
        extensions: &["psd", "psb"],
    },
];

pub const SUPPORTED_IMAGE_EXTENSIONS: &[&str] = &[
    "avif", "jpg", "jpeg", "png", "gif", "webp", "tif", "tiff", "tga", "dds", "bmp", "ico", "hdr",
    "exr", "pbm", "pam", "ppm", "pgm", "ff", "farbfeld", "qoi", "svg", "heic", "heif", "psd",
    "psb",
];

pub fn supported_image_format_groups() -> &'static [ImageFormatGroup] {
    SUPPORTED_IMAGE_FORMAT_GROUPS
}

pub fn supported_image_extensions() -> &'static [&'static str] {
    SUPPORTED_IMAGE_EXTENSIONS
}

pub fn is_supported_image_extension(extension: &str) -> bool {
    SUPPORTED_IMAGE_EXTENSIONS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(extension))
}

pub fn requires_plugin_preview_extension(extension: &str) -> bool {
    extension.eq_ignore_ascii_case("psd") || extension.eq_ignore_ascii_case("psb")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn format_groups_cover_supported_extensions_exactly_once() {
        let grouped = SUPPORTED_IMAGE_FORMAT_GROUPS
            .iter()
            .flat_map(|group| group.extensions.iter().copied())
            .collect::<Vec<_>>();
        let unique = grouped.iter().copied().collect::<BTreeSet<_>>();
        let supported = SUPPORTED_IMAGE_EXTENSIONS
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();

        assert_eq!(
            grouped.len(),
            unique.len(),
            "format groups contain duplicates"
        );
        assert_eq!(unique, supported);
    }

    #[test]
    fn format_group_ids_are_unique() {
        let ids = SUPPORTED_IMAGE_FORMAT_GROUPS
            .iter()
            .map(|group| group.id)
            .collect::<Vec<_>>();
        let unique = ids.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), unique.len());
    }

    #[test]
    fn photoshop_group_exposes_psd_and_psb() {
        let group = SUPPORTED_IMAGE_FORMAT_GROUPS
            .iter()
            .find(|group| group.id == "photoshop")
            .expect("Photoshop format group should be registered");
        assert_eq!(group.name, "Adobe Photoshop");
        assert_eq!(group.extensions, ["psd", "psb"]);
        assert!(is_supported_image_extension("PSD"));
        assert!(is_supported_image_extension("psb"));
        assert!(requires_plugin_preview_extension("PSD"));
    }
}
