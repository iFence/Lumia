fn main() {
    #[cfg(windows)]
    {
        let icon_path = "resources/icon.ico";
        if std::path::Path::new(icon_path).exists() {
            let mut res = winres::WindowsResource::new();
            res.set_icon(icon_path);
            res.compile().expect("failed to embed icon resource");
        }
    }
}
