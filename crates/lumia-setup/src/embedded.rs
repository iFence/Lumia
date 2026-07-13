use std::{fs, path::PathBuf, ptr, slice};

use anyhow::{bail, Context as _};
use windows_sys::Win32::System::LibraryLoader::{
    FindResourceW, GetModuleHandleW, LoadResource, LockResource, SizeofResource,
};

const RT_RCDATA: *const u16 = 10usize as *const u16;

pub(crate) struct ExtractedMsi {
    directory: PathBuf,
    path: PathBuf,
}

impl ExtractedMsi {
    pub(crate) fn extract(resource_id: u16, file_name: &str) -> anyhow::Result<Self> {
        let bytes = embedded_resource(resource_id)?;
        let directory = std::env::temp_dir().join(format!(
            "LumiaSetup-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        ));
        fs::create_dir_all(&directory).context("create the Setup temporary directory")?;
        let path = directory.join(file_name);
        fs::write(&path, bytes).context("extract the embedded Lumia MSI")?;
        Ok(Self { directory, path })
    }

    pub(crate) fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for ExtractedMsi {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn embedded_resource(resource_id: u16) -> anyhow::Result<&'static [u8]> {
    let module = unsafe { GetModuleHandleW(ptr::null()) };
    if module.is_null() {
        bail!("locate the Setup executable module");
    }
    let resource = unsafe { FindResourceW(module, resource_id as usize as *const u16, RT_RCDATA) };
    if resource.is_null() {
        bail!("the selected language MSI is not embedded in this Setup executable");
    }
    let size = unsafe { SizeofResource(module, resource) };
    let loaded = unsafe { LoadResource(module, resource) };
    if size == 0 || loaded.is_null() {
        bail!("load the embedded Lumia MSI resource");
    }
    let data = unsafe { LockResource(loaded) };
    if data.is_null() {
        bail!("read the embedded Lumia MSI resource");
    }
    Ok(unsafe { slice::from_raw_parts(data.cast::<u8>(), size as usize) })
}
