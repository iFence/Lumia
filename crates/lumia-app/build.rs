fn main() {
    #[cfg(windows)]
    {
        let icon_path = "resources/icon.ico";
        if std::path::Path::new(icon_path).exists() {
            let mut res = winres::WindowsResource::new();
            res.set_icon(icon_path);
            res.set("FileVersion", env!("CARGO_PKG_VERSION"));
            res.set("ProductVersion", env!("CARGO_PKG_VERSION"));
            res.set("ProductName", "Lumia");
            res.set("FileDescription", "Lumia Image Viewer");
            res.compile().expect("failed to embed windows resource");
        }
    }
}
