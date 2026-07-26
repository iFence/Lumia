use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(super) struct AssociationFormat {
    pub(super) extensions: &'static [&'static str],
    pub(super) macos_content_types: &'static [&'static str],
    pub(super) linux_mime_types: &'static [&'static str],
}

pub(super) const ASSOCIATION_FORMATS: &[AssociationFormat] = &[
    format(&["avif"], &["public.avif"], &["image/avif"]),
    format(&["jpg", "jpeg"], &["public.jpeg"], &["image/jpeg"]),
    format(&["png"], &["public.png"], &["image/png"]),
    format(&["gif"], &["public.gif"], &["image/gif"]),
    format(&["webp"], &["org.webmproject.webp"], &["image/webp"]),
    format(&["tif", "tiff"], &["public.tiff"], &["image/tiff"]),
    format(&["tga"], &["com.truevision.tga-image"], &["image/x-tga"]),
    format(&["dds"], &["com.ifence.lumia.dds"], &["image/x-dds"]),
    format(&["bmp"], &["com.microsoft.bmp"], &["image/bmp"]),
    format(
        &["ico"],
        &["com.microsoft.ico"],
        &["image/vnd.microsoft.icon"],
    ),
    format(&["hdr"], &["com.ifence.lumia.hdr"], &["image/vnd.radiance"]),
    format(&["exr"], &["com.ifence.lumia.exr"], &["image/x-exr"]),
    format(
        &["pbm", "pam", "ppm", "pgm"],
        &["com.ifence.lumia.netpbm"],
        &[
            "image/x-portable-bitmap",
            "image/x-portable-anymap",
            "image/x-portable-pixmap",
            "image/x-portable-graymap",
        ],
    ),
    format(
        &["ff", "farbfeld"],
        &["com.ifence.lumia.farbfeld"],
        &["image/x-farbfeld"],
    ),
    format(&["qoi"], &["com.ifence.lumia.qoi"], &["image/x-qoi"]),
    format(&["svg"], &["public.svg-image"], &["image/svg+xml"]),
    format(
        &["heic", "heif"],
        &["public.heic", "public.heif"],
        &["image/heic", "image/heif"],
    ),
    format(
        &["psd", "psb"],
        &[
            "com.adobe.photoshop-image",
            "com.ifence.lumia.photoshop-large-document",
        ],
        &["image/vnd.adobe.photoshop"],
    ),
];

const fn format(
    extensions: &'static [&'static str],
    macos_content_types: &'static [&'static str],
    linux_mime_types: &'static [&'static str],
) -> AssociationFormat {
    AssociationFormat {
        extensions,
        macos_content_types,
        linux_mime_types,
    }
}

#[allow(dead_code)]
pub(super) fn extensions_for_linux_mime(mime: &str) -> BTreeSet<String> {
    ASSOCIATION_FORMATS
        .iter()
        .filter(|format| format.linux_mime_types.contains(&mime))
        .flat_map(|format| format.extensions.iter())
        .map(|extension| (*extension).to_string())
        .collect()
}

#[allow(dead_code)]
pub(super) fn extensions_for_macos_content_type(content_type: &str) -> BTreeSet<String> {
    ASSOCIATION_FORMATS
        .iter()
        .filter(|format| format.macos_content_types.contains(&content_type))
        .flat_map(|format| format.extensions.iter())
        .map(|extension| (*extension).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn association_formats_cover_every_supported_extension_once() {
        let mapped = ASSOCIATION_FORMATS
            .iter()
            .flat_map(|format| format.extensions.iter().copied())
            .collect::<Vec<_>>();
        let unique = mapped.iter().copied().collect::<BTreeSet<_>>();
        let supported = lumia_core::supported_image_extensions()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();

        assert_eq!(mapped.len(), unique.len());
        assert_eq!(unique, supported);
    }

    #[test]
    fn shared_handlers_expand_to_complete_format_groups() {
        assert_eq!(
            extensions_for_linux_mime("image/jpeg"),
            ["jpeg".to_string(), "jpg".to_string()].into()
        );
        assert_eq!(
            extensions_for_macos_content_type("public.jpeg"),
            ["jpeg".to_string(), "jpg".to_string()].into()
        );
    }
}
