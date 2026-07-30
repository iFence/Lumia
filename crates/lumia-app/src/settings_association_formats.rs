use crate::i18n::TextKey;

#[derive(Clone, Copy)]
pub(crate) struct FileAssociationCategory {
    pub(crate) id: &'static str,
    pub(crate) title: TextKey,
    pub(crate) extensions: &'static [&'static str],
}

const COMMON_IMAGE_FORMATS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "tif", "tiff", "tga", "dds", "bmp", "ico",
];

const MODERN_IMAGE_FORMATS: &[&str] = &["avif", "heic", "heif", "hdr", "exr", "qoi"];

const DESIGN_IMAGE_FORMATS: &[&str] = &["psd", "psb", "svg"];

const TECHNICAL_IMAGE_FORMATS: &[&str] = &["pbm", "pam", "ppm", "pgm", "ff", "farbfeld"];

const RAW_IMAGE_FORMATS: &[&str] = &[
    "dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "sr2", "srf", "raf", "orf", "rw2", "rwl",
    "pef", "srw", "3fr", "fff", "mef", "mos", "mrw", "kdc", "dcr", "erf", "x3f", "iiq",
];

pub(crate) const FILE_ASSOCIATION_CATEGORIES: &[FileAssociationCategory] = &[
    FileAssociationCategory {
        id: "common",
        title: TextKey::FormatCategoryCommon,
        extensions: COMMON_IMAGE_FORMATS,
    },
    FileAssociationCategory {
        id: "modern",
        title: TextKey::FormatCategoryModern,
        extensions: MODERN_IMAGE_FORMATS,
    },
    FileAssociationCategory {
        id: "design",
        title: TextKey::FormatCategoryDesign,
        extensions: DESIGN_IMAGE_FORMATS,
    },
    FileAssociationCategory {
        id: "technical",
        title: TextKey::FormatCategoryTechnical,
        extensions: TECHNICAL_IMAGE_FORMATS,
    },
    FileAssociationCategory {
        id: "raw",
        title: TextKey::FormatCategoryRaw,
        extensions: RAW_IMAGE_FORMATS,
    },
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn categories_cover_every_supported_extension_once() {
        let categorized = FILE_ASSOCIATION_CATEGORIES
            .iter()
            .flat_map(|category| category.extensions.iter().copied())
            .collect::<Vec<_>>();
        let unique = categorized.iter().copied().collect::<BTreeSet<_>>();
        let supported = lumia_core::supported_image_extensions()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();

        assert_eq!(categorized.len(), unique.len());
        assert_eq!(unique, supported);
    }
}
