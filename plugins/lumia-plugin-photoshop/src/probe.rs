use std::fs::File;
use std::io::Read;
use std::path::Path;

use lumia_plugin_api::ProbeResult;

use crate::header::{HeaderError, PhotoshopHeader};

pub(crate) fn probe(path: &Path) -> Result<ProbeResult, ProbeError> {
    let mut file = File::open(path)?;
    let file_len = file.metadata()?.len();
    let mut bytes = [0_u8; 26];
    file.read_exact(&mut bytes)?;
    let header = PhotoshopHeader::parse(&bytes, file_len)?;

    Ok(ProbeResult {
        can_decode: true,
        format_name: Some(header.kind.name().to_string()),
        width: Some(header.width),
        height: Some(header.height),
        is_hdr: header.depth == 32,
    })
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ProbeError {
    #[error("could not read Photoshop document")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Header(#[from] HeaderError),
}
