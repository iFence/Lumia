use std::env;
use std::ffi::{c_char, c_void};
use std::path::{Path, PathBuf};
use std::slice;

use libloading::Library;

const BRIDGE_ABI_VERSION: u32 = 1;
const STATUS_UNSUPPORTED: i32 = 1;
const STATUS_CORRUPT: i32 = 2;
const STATUS_RESOURCE_LIMIT: i32 = 3;
const STATUS_DECODE_FAILED: i32 = 4;
const MAX_NATIVE_IMAGE_BYTES: usize = 512 * 1024 * 1024;

type AbiVersionFn = unsafe extern "C" fn() -> u32;
type ProbeFn = unsafe extern "C" fn(*const u8, usize, *mut NativeProbe) -> i32;
type DecodeFn = unsafe extern "C" fn(*const u8, usize, *mut NativeImage) -> i32;
type FreeImageFn = unsafe extern "C" fn(*mut NativeImage);
type LastErrorFn = unsafe extern "C" fn(*mut u8, usize) -> usize;

#[repr(C)]
#[derive(Clone, Copy)]
struct NativeProbe {
    width: u32,
    height: u32,
    iso: f64,
    exposure_seconds: f64,
    aperture: f64,
    focal_length_mm: f64,
    latitude: f64,
    longitude: f64,
    altitude_meters: f64,
    gps_valid: u8,
    altitude_valid: u8,
    reserved: [u8; 6],
    camera_make: [c_char; 64],
    camera_model: [c_char; 64],
    lens: [c_char; 128],
    date_taken: [c_char; 32],
}

impl Default for NativeProbe {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            iso: 0.0,
            exposure_seconds: 0.0,
            aperture: 0.0,
            focal_length_mm: 0.0,
            latitude: 0.0,
            longitude: 0.0,
            altitude_meters: 0.0,
            gps_valid: 0,
            altitude_valid: 0,
            reserved: [0; 6],
            camera_make: [0; 64],
            camera_model: [0; 64],
            lens: [0; 128],
            date_taken: [0; 32],
        }
    }
}

#[repr(C)]
struct NativeImage {
    data: *const u8,
    data_len: usize,
    owner: *mut c_void,
    width: u32,
    height: u32,
    stride: u32,
    channels: u8,
    bits_per_channel: u8,
    reserved: [u8; 2],
}

impl Default for NativeImage {
    fn default() -> Self {
        Self {
            data: std::ptr::null(),
            data_len: 0,
            owner: std::ptr::null_mut(),
            width: 0,
            height: 0,
            stride: 0,
            channels: 0,
            bits_per_channel: 0,
            reserved: [0; 2],
        }
    }
}

pub(crate) struct Bridge {
    _library: Library,
    probe_fn: ProbeFn,
    #[cfg(windows)]
    _libraw_library: Library,
    decode_fn: DecodeFn,
    free_image_fn: FreeImageFn,
    last_error_fn: LastErrorFn,
}

#[derive(Debug)]
pub(crate) struct BridgeProbe {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) camera_make: Option<String>,
    pub(crate) camera_model: Option<String>,
    pub(crate) lens: Option<String>,
    pub(crate) iso: Option<u32>,
    pub(crate) exposure_seconds: Option<f64>,
    pub(crate) aperture: Option<f64>,
    pub(crate) focal_length_mm: Option<f64>,
    pub(crate) date_taken: Option<String>,
    pub(crate) location: Option<BridgeLocation>,
}

#[derive(Debug)]
pub(crate) struct BridgeLocation {
    pub(crate) latitude: f64,
    pub(crate) longitude: f64,
    pub(crate) altitude_meters: Option<f64>,
}

#[derive(Debug)]
pub(crate) struct BridgeImage {
    pub(crate) pixels_rgb8: Vec<u8>,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl Bridge {
    pub(crate) fn load() -> Result<Self, BridgeError> {
        let path = match env::var_os("LUMIA_RAW_BRIDGE_PATH") {
            Some(path) => PathBuf::from(path),
            None => env::current_exe()
                .map_err(|error| BridgeError::Unavailable(error.to_string()))?
                .parent()
                .ok_or_else(|| BridgeError::Unavailable("plugin directory is unavailable".into()))?
                .join(library_name()),
        };
        Self::load_from(&path)
    }

    fn load_from(path: &Path) -> Result<Self, BridgeError> {
        // SAFETY: Libraries are loaded from the signed plugin directory. Symbols are
        // copied as function pointers while both owning libraries remain in Bridge.
        unsafe {
            #[cfg(windows)]
            let libraw_library = {
                let dependency = path
                    .parent()
                    .ok_or_else(|| {
                        BridgeError::Unavailable("plugin directory is unavailable".into())
                    })?
                    .join("raw.dll");
                Library::new(dependency)
                    .map_err(|error| BridgeError::Unavailable(error.to_string()))?
            };
            let library =
                Library::new(path).map_err(|error| BridgeError::Unavailable(error.to_string()))?;
            let abi_version = *library
                .get::<AbiVersionFn>(b"lumia_raw_bridge_abi_version\0")
                .map_err(|error| BridgeError::Unavailable(error.to_string()))?;
            let actual = abi_version();
            if actual != BRIDGE_ABI_VERSION {
                return Err(BridgeError::AbiMismatch {
                    expected: BRIDGE_ABI_VERSION,
                    actual,
                });
            }
            let probe_fn = *library
                .get::<ProbeFn>(b"lumia_raw_bridge_probe\0")
                .map_err(|error| BridgeError::Unavailable(error.to_string()))?;
            let decode_fn = *library
                .get::<DecodeFn>(b"lumia_raw_bridge_decode\0")
                .map_err(|error| BridgeError::Unavailable(error.to_string()))?;
            let free_image_fn = *library
                .get::<FreeImageFn>(b"lumia_raw_bridge_free_image\0")
                .map_err(|error| BridgeError::Unavailable(error.to_string()))?;
            let last_error_fn = *library
                .get::<LastErrorFn>(b"lumia_raw_bridge_last_error\0")
                .map_err(|error| BridgeError::Unavailable(error.to_string()))?;
            Ok(Self {
                _library: library,
                probe_fn,
                decode_fn,
                #[cfg(windows)]
                _libraw_library: libraw_library,
                free_image_fn,
                last_error_fn,
            })
        }
    }

    pub(crate) fn probe(&self, path: &Path) -> Result<BridgeProbe, BridgeError> {
        let path = utf8_path(path)?;
        let mut probe = NativeProbe::default();
        // SAFETY: The UTF-8 path and output structure remain valid for the call.
        let status = unsafe { (self.probe_fn)(path.as_ptr(), path.len(), &mut probe) };
        self.check(status)?;
        Ok(BridgeProbe {
            width: probe.width,
            height: probe.height,
            camera_make: native_string(&probe.camera_make),
            camera_model: native_string(&probe.camera_model),
            lens: native_string(&probe.lens),
            iso: finite_positive(probe.iso).map(|value| value.round() as u32),
            exposure_seconds: finite_positive(probe.exposure_seconds),
            aperture: finite_positive(probe.aperture),
            focal_length_mm: finite_positive(probe.focal_length_mm),
            date_taken: native_string(&probe.date_taken),
            location: (probe.gps_valid != 0).then(|| BridgeLocation {
                latitude: probe.latitude,
                longitude: probe.longitude,
                altitude_meters: (probe.altitude_valid != 0).then_some(probe.altitude_meters),
            }),
        })
    }

    pub(crate) fn decode(&self, path: &Path) -> Result<BridgeImage, BridgeError> {
        let path = utf8_path(path)?;
        let mut image = NativeImage::default();
        // SAFETY: The UTF-8 path and output structure remain valid for the call.
        let status = unsafe { (self.decode_fn)(path.as_ptr(), path.len(), &mut image) };
        if let Err(error) = self.check(status) {
            // SAFETY: The bridge accepts an empty or partially initialized image.
            unsafe { (self.free_image_fn)(&mut image) };
            return Err(error);
        }
        let expected = usize::try_from(image.width)
            .ok()
            .and_then(|width| {
                usize::try_from(image.height)
                    .ok()
                    .map(|height| (width, height))
            })
            .and_then(|(width, height)| width.checked_mul(height))
            .and_then(|pixels| pixels.checked_mul(3));
        let valid = !image.data.is_null()
            && image.width > 0
            && image.height > 0
            && image.channels == 3
            && image.bits_per_channel == 8
            && image.stride == image.width.saturating_mul(3)
            && expected == Some(image.data_len)
            && image.data_len <= MAX_NATIVE_IMAGE_BYTES;
        let result = if valid {
            // SAFETY: The bridge promises data_len initialized bytes owned by owner;
            // they are copied before the bridge allocation is released.
            Ok(BridgeImage {
                pixels_rgb8: unsafe { slice::from_raw_parts(image.data, image.data_len) }.to_vec(),
                width: image.width,
                height: image.height,
            })
        } else {
            Err(BridgeError::InvalidImage)
        };
        // SAFETY: image was initialized by the matching bridge library.
        unsafe { (self.free_image_fn)(&mut image) };
        result
    }

    fn check(&self, status: i32) -> Result<(), BridgeError> {
        if status == 0 {
            return Ok(());
        }
        let message = self.last_error();
        Err(match status {
            STATUS_UNSUPPORTED => BridgeError::Unsupported(message),
            STATUS_CORRUPT => BridgeError::Corrupt(message),
            STATUS_RESOURCE_LIMIT => BridgeError::ResourceLimit(message),
            STATUS_DECODE_FAILED => BridgeError::DecodeFailed(message),
            _ => BridgeError::DecodeFailed(message),
        })
    }

    fn last_error(&self) -> String {
        let mut bytes = [0_u8; 512];
        // SAFETY: bytes is a writable buffer with the passed capacity.
        let len = unsafe { (self.last_error_fn)(bytes.as_mut_ptr(), bytes.len()) };
        String::from_utf8_lossy(&bytes[..len.min(bytes.len())])
            .trim_end_matches('\0')
            .to_string()
    }
}

fn utf8_path(path: &Path) -> Result<String, BridgeError> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| BridgeError::Unavailable("input path is not valid UTF-8".into()))
}

fn native_string<const N: usize>(value: &[c_char; N]) -> Option<String> {
    let end = value.iter().position(|byte| *byte == 0).unwrap_or(N);
    let bytes = value[..end]
        .iter()
        .map(|byte| *byte as u8)
        .collect::<Vec<_>>();
    let value = String::from_utf8_lossy(&bytes).trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn finite_positive(value: f64) -> Option<f64> {
    (value.is_finite() && value > 0.0).then_some(value)
}

fn library_name() -> &'static str {
    if cfg!(windows) {
        "lumia_raw_bridge.dll"
    } else if cfg!(target_os = "macos") {
        "liblumia_raw_bridge.dylib"
    } else {
        "liblumia_raw_bridge.so"
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum BridgeError {
    #[error("RAW native bridge is unavailable: {0}")]
    Unavailable(String),
    #[error("RAW native bridge ABI mismatch (expected {expected}, found {actual})")]
    AbiMismatch { expected: u32, actual: u32 },
    #[error("RAW format is unsupported: {0}")]
    Unsupported(String),
    #[error("RAW image is corrupt: {0}")]
    Corrupt(String),
    #[error("RAW image exceeds resource limits: {0}")]
    ResourceLimit(String),
    #[error("RAW image could not be decoded: {0}")]
    DecodeFailed(String),
    #[error("RAW bridge returned invalid image data")]
    InvalidImage,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_bridge_is_reported_without_linking_libraw() {
        let error = match Bridge::load_from(Path::new("definitely-missing-lumia-raw-bridge")) {
            Ok(_) => panic!("unexpectedly loaded a missing bridge"),
            Err(error) => error,
        };
        assert!(matches!(error, BridgeError::Unavailable(_)));
    }

    #[test]
    fn native_strings_are_trimmed_and_optional() {
        let mut value = [0_i8; 8];
        value[..4].copy_from_slice(&[b'R' as i8, b'A' as i8, b'W' as i8, b' ' as i8]);
        assert_eq!(native_string(&value).as_deref(), Some("RAW"));
        assert_eq!(native_string(&[0_i8; 4]), None);
    }
}
