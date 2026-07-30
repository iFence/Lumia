pub(crate) const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_SIDE: u32 = 100_000;
const MAX_PIXELS: u64 = 500_000_000;
const HEADER_LEN: usize = 26;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PhotoshopKind {
    Psd,
    Psb,
}

impl PhotoshopKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Psd => "psd",
            Self::Psb => "psb",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PhotoshopHeader {
    pub(crate) kind: PhotoshopKind,
    pub(crate) channels: u16,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) depth: u16,
    pub(crate) color_mode: u16,
}

impl PhotoshopHeader {
    pub(crate) fn parse(bytes: &[u8], file_len: u64) -> Result<Self, HeaderError> {
        if file_len > MAX_FILE_BYTES {
            return Err(HeaderError::ResourceLimit);
        }
        let bytes = bytes.get(..HEADER_LEN).ok_or(HeaderError::Truncated)?;
        if &bytes[..4] != b"8BPS" {
            return Err(HeaderError::InvalidSignature);
        }

        let version = u16::from_be_bytes([bytes[4], bytes[5]]);
        let kind = match version {
            1 => PhotoshopKind::Psd,
            2 => PhotoshopKind::Psb,
            value => return Err(HeaderError::UnsupportedVersion(value)),
        };
        let channels = u16::from_be_bytes([bytes[12], bytes[13]]);
        let height = u32::from_be_bytes(bytes[14..18].try_into().unwrap());
        let width = u32::from_be_bytes(bytes[18..22].try_into().unwrap());
        let depth = u16::from_be_bytes([bytes[22], bytes[23]]);
        let color_mode = u16::from_be_bytes([bytes[24], bytes[25]]);

        if channels == 0 || width == 0 || height == 0 || !matches!(depth, 1 | 8 | 16 | 32) {
            return Err(HeaderError::InvalidHeader);
        }
        if width > MAX_SIDE
            || height > MAX_SIDE
            || u64::from(width) * u64::from(height) > MAX_PIXELS
        {
            return Err(HeaderError::ResourceLimit);
        }

        Ok(Self {
            kind,
            channels,
            width,
            height,
            depth,
            color_mode,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum HeaderError {
    #[error("Photoshop header is truncated")]
    Truncated,
    #[error("file does not have a Photoshop signature")]
    InvalidSignature,
    #[error("Photoshop version {0} is unsupported")]
    UnsupportedVersion(u16),
    #[error("Photoshop header contains invalid dimensions, channels, or depth")]
    InvalidHeader,
    #[error("Photoshop document exceeds preview safety limits")]
    ResourceLimit,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(version: u16, width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"8BPS");
        bytes.extend_from_slice(&version.to_be_bytes());
        bytes.extend_from_slice(&[0; 6]);
        bytes.extend_from_slice(&4_u16.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&8_u16.to_be_bytes());
        bytes.extend_from_slice(&3_u16.to_be_bytes());
        bytes
    }

    #[test]
    fn parses_psd_and_psb_headers() {
        let psd = PhotoshopHeader::parse(&header(1, 640, 480), 1024).unwrap();
        assert_eq!(psd.kind, PhotoshopKind::Psd);
        assert_eq!((psd.width, psd.height), (640, 480));

        let psb = PhotoshopHeader::parse(&header(2, 4000, 3000), 1024).unwrap();
        assert_eq!(psb.kind, PhotoshopKind::Psb);
    }

    #[test]
    fn rejects_invalid_signature() {
        let mut bytes = header(1, 640, 480);
        bytes[..4].copy_from_slice(b"NOPE");
        assert_eq!(
            PhotoshopHeader::parse(&bytes, 1024).unwrap_err(),
            HeaderError::InvalidSignature
        );
    }

    #[test]
    fn rejects_unsupported_version() {
        assert_eq!(
            PhotoshopHeader::parse(&header(3, 640, 480), 1024).unwrap_err(),
            HeaderError::UnsupportedVersion(3)
        );
    }

    #[test]
    fn rejects_resource_limits() {
        assert_eq!(
            PhotoshopHeader::parse(&header(1, 100_001, 1), 1024).unwrap_err(),
            HeaderError::ResourceLimit
        );
        assert_eq!(
            PhotoshopHeader::parse(&header(1, 1, 1), MAX_FILE_BYTES + 1).unwrap_err(),
            HeaderError::ResourceLimit
        );
    }
}
