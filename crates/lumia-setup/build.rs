fn main() {
    println!("cargo:rerun-if-env-changed=LUMIA_MSI_EN_US");
    println!("cargo:rerun-if-env-changed=LUMIA_MSI_ZH_CN");
    println!("cargo:rerun-if-changed=../lumia-app/resources/icon.ico");

    #[cfg(windows)]
    build_windows_resources();
}

#[cfg(windows)]
fn build_windows_resources() {
    let mut resources = winres::WindowsResource::new();
    resources.set_icon("../lumia-app/resources/icon.ico");
    resources.set("FileVersion", env!("CARGO_PKG_VERSION"));
    resources.set("ProductVersion", env!("CARGO_PKG_VERSION"));
    resources.set("ProductName", "Lumia Setup");
    resources.set("FileDescription", "Lumia Setup Bootstrapper");
    resources.set_manifest(
        r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*" />
    </dependentAssembly>
  </dependency>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security><requestedPrivileges><requestedExecutionLevel level="asInvoker" uiAccess="false" /></requestedPrivileges></security>
  </trustInfo>
</assembly>"#,
    );

    let en_us = std::env::var_os("LUMIA_MSI_EN_US");
    let zh_cn = std::env::var_os("LUMIA_MSI_ZH_CN");
    if let (Some(en_us), Some(zh_cn)) = (en_us, zh_cn) {
        println!(
            "cargo:rerun-if-changed={}",
            std::path::Path::new(&en_us).display()
        );
        println!(
            "cargo:rerun-if-changed={}",
            std::path::Path::new(&zh_cn).display()
        );
        let en_us = resource_path(&en_us);
        let zh_cn = resource_path(&zh_cn);
        resources.append_rc_content(&format!("101 RCDATA \"{en_us}\"\n102 RCDATA \"{zh_cn}\"\n"));
    } else {
        println!(
            "cargo:warning=building Lumia Setup without embedded MSIs; set LUMIA_MSI_EN_US and LUMIA_MSI_ZH_CN for a distributable Setup"
        );
    }

    resources
        .compile()
        .expect("failed to embed setup resources");
}

#[cfg(windows)]
fn resource_path(path: &std::ffi::OsStr) -> String {
    std::path::Path::new(path)
        .canonicalize()
        .expect("embedded MSI path does not exist")
        .to_string_lossy()
        .replace('\\', "\\\\")
}
