use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use lumia_plugin_api::ProbeResult;

const JP2_SIGNATURE: [u8; 12] = [
    0x00, 0x00, 0x00, 0x0c, b'j', b'P', b' ', b' ', 0x0d, 0x0a, 0x87, 0x0a,
];
const MAX_HEADER_SCAN: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Jpeg2000Header {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) components: u16,
    pub(crate) max_precision: u8,
}

pub(crate) fn probe(path: &Path) -> Result<ProbeResult, ProbeError> {
    let header = read_header(path)?;
    Ok(ProbeResult {
        can_decode: true,
        format_name: Some("JPEG 2000".to_string()),
        width: Some(header.width),
        height: Some(header.height),
        is_hdr: header.max_precision > 8,
        metadata: None,
    })
}

pub(crate) fn read_header(path: &Path) -> Result<Jpeg2000Header, ProbeError> {
    let mut file = File::open(path)?;
    let file_len = file.metadata()?.len();
    let mut prefix = [0_u8; 12];
    file.read_exact(&mut prefix)?;
    let codestream_offset = if prefix.starts_with(&[0xff, 0x4f]) {
        0
    } else if prefix == JP2_SIGNATURE {
        find_jp2_codestream(&mut file, file_len)?
    } else {
        return Err(ProbeError::UnsupportedSignature);
    };
    file.seek(SeekFrom::Start(codestream_offset))?;
    parse_siz(&mut file)
}

fn find_jp2_codestream(file: &mut File, file_len: u64) -> Result<u64, ProbeError> {
    let mut offset = 12_u64;
    while offset < file_len && offset <= MAX_HEADER_SCAN {
        file.seek(SeekFrom::Start(offset))?;
        let mut header = [0_u8; 8];
        file.read_exact(&mut header)?;
        let short_len = u32::from_be_bytes(header[..4].try_into().unwrap());
        let box_type = &header[4..8];
        let (box_len, header_len) = match short_len {
            0 => (file_len.saturating_sub(offset), 8_u64),
            1 => {
                let mut extended = [0_u8; 8];
                file.read_exact(&mut extended)?;
                (u64::from_be_bytes(extended), 16_u64)
            }
            length => (u64::from(length), 8_u64),
        };
        if box_len < header_len || offset.saturating_add(box_len) > file_len {
            return Err(ProbeError::CorruptHeader);
        }
        if box_type == b"jp2c" {
            return Ok(offset + header_len);
        }
        offset = offset
            .checked_add(box_len)
            .ok_or(ProbeError::ResourceLimit)?;
    }
    if offset > MAX_HEADER_SCAN {
        Err(ProbeError::ResourceLimit)
    } else {
        Err(ProbeError::MissingCodestream)
    }
}

fn parse_siz(reader: &mut impl Read) -> Result<Jpeg2000Header, ProbeError> {
    let mut marker = [0_u8; 6];
    reader.read_exact(&mut marker)?;
    if marker[..4] != [0xff, 0x4f, 0xff, 0x51] {
        return Err(ProbeError::CorruptHeader);
    }
    let length = usize::from(u16::from_be_bytes([marker[4], marker[5]]));
    if !(41..=49_190).contains(&length) {
        return Err(ProbeError::CorruptHeader);
    }
    let mut siz = vec![0_u8; length - 2];
    reader.read_exact(&mut siz)?;
    let x_size = u32::from_be_bytes(siz[2..6].try_into().unwrap());
    let y_size = u32::from_be_bytes(siz[6..10].try_into().unwrap());
    let x_offset = u32::from_be_bytes(siz[10..14].try_into().unwrap());
    let y_offset = u32::from_be_bytes(siz[14..18].try_into().unwrap());
    let components = u16::from_be_bytes(siz[34..36].try_into().unwrap());
    let expected = 38_usize
        .checked_add(usize::from(components).saturating_mul(3))
        .ok_or(ProbeError::ResourceLimit)?;
    if components == 0 || length != expected || x_size <= x_offset || y_size <= y_offset {
        return Err(ProbeError::CorruptHeader);
    }
    let mut max_precision = 0_u8;
    for component in siz[36..].chunks_exact(3) {
        let precision = (component[0] & 0x7f).saturating_add(1);
        if precision > 38 || component[1] == 0 || component[2] == 0 {
            return Err(ProbeError::CorruptHeader);
        }
        max_precision = max_precision.max(precision);
    }
    Ok(Jpeg2000Header {
        width: x_size - x_offset,
        height: y_size - y_offset,
        components,
        max_precision,
    })
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ProbeError {
    #[error("could not read JPEG 2000 input")]
    Io(#[from] std::io::Error),
    #[error("unsupported JPEG 2000 signature")]
    UnsupportedSignature,
    #[error("JPEG 2000 codestream is missing")]
    MissingCodestream,
    #[error("JPEG 2000 header is corrupt")]
    CorruptHeader,
    #[error("JPEG 2000 header exceeds resource limits")]
    ResourceLimit,
}
