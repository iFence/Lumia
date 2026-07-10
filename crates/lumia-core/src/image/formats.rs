pub const SUPPORTED_IMAGE_EXTENSIONS: &[&str] = &[
    "avif", "jpg", "jpeg", "png", "gif", "webp", "tif", "tiff", "tga", "dds", "bmp", "ico", "hdr",
    "exr", "pbm", "pam", "ppm", "pgm", "ff", "farbfeld", "qoi", "svg", "heic", "heif",
];

pub fn supported_image_extensions() -> &'static [&'static str] {
    SUPPORTED_IMAGE_EXTENSIONS
}

pub fn is_supported_image_extension(extension: &str) -> bool {
    SUPPORTED_IMAGE_EXTENSIONS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(extension))
}
