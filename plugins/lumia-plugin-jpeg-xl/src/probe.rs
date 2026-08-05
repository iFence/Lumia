use std::path::Path;

use jxl_oxide::{AllocTracker, JxlImage};
use lumia_plugin_api::ProbeResult;

const PROBE_MEMORY_LIMIT: usize = 32 * 1024 * 1024;

pub(crate) fn probe(path: &Path) -> Result<ProbeResult, ProbeError> {
    let image = JxlImage::builder()
        .alloc_tracker(AllocTracker::with_limit(PROBE_MEMORY_LIMIT))
        .open(path)?;
    Ok(ProbeResult {
        can_decode: true,
        format_name: Some("JPEG XL".to_string()),
        width: Some(image.width()),
        height: Some(image.height()),
        is_hdr: image.hdr_type().is_some(),
        metadata: None,
    })
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ProbeError {
    #[error("JPEG XL header could not be decoded")]
    Jxl(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}
