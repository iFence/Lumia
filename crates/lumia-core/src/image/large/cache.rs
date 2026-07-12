use std::{
    collections::hash_map::DefaultHasher,
    fs::{self, File, OpenOptions},
    hash::{Hash, Hasher},
    ops::Range,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use memmap2::{Mmap, MmapMut};

use super::LargeImageError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RasterLayout {
    width: u32,
    height: u32,
    row_bytes: usize,
    len: usize,
}

impl RasterLayout {
    pub(crate) fn new(width: u32, height: u32) -> Result<Self, LargeImageError> {
        if width == 0 || height == 0 {
            return Err(LargeImageError::InvalidDimensions);
        }
        let row_bytes = usize::try_from(
            u64::from(width)
                .checked_mul(4)
                .ok_or(LargeImageError::SizeOverflow)?,
        )
        .map_err(|_| LargeImageError::SizeOverflow)?;
        let len = row_bytes
            .checked_mul(usize::try_from(height).map_err(|_| LargeImageError::SizeOverflow)?)
            .ok_or(LargeImageError::SizeOverflow)?;
        Ok(Self {
            width,
            height,
            row_bytes,
            len,
        })
    }

    pub(crate) const fn width(self) -> u32 {
        self.width
    }

    pub(crate) const fn height(self) -> u32 {
        self.height
    }

    pub(crate) const fn row_bytes(self) -> usize {
        self.row_bytes
    }

    pub(crate) const fn len(self) -> usize {
        self.len
    }

    pub(crate) fn row_range(self, row: u32) -> Option<Range<usize>> {
        if row >= self.height {
            return None;
        }
        let start = usize::try_from(row).ok()?.checked_mul(self.row_bytes())?;
        Some(start..start.checked_add(self.row_bytes())?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RasterCacheKey(String);

impl RasterCacheKey {
    pub(crate) fn from_source(path: &Path) -> Result<Self, LargeImageError> {
        let metadata = fs::metadata(path)?;
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let modified = metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_nanos())
            .unwrap_or_default();
        let mut hasher = DefaultHasher::new();
        canonical.hash(&mut hasher);
        metadata.len().hash(&mut hasher);
        modified.hash(&mut hasher);
        Ok(Self(format!("{:016x}", hasher.finish())))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
pub(crate) fn ensure_disk_space(required: u64, available: u64) -> Result<(), LargeImageError> {
    if required > available {
        Err(LargeImageError::InsufficientDiskSpace {
            required,
            available,
        })
    } else {
        Ok(())
    }
}

pub(crate) struct RasterCacheWriter {
    map: Option<MmapMut>,
    file: Option<File>,
    layout: RasterLayout,
    partial_path: PathBuf,
    finished_path: PathBuf,
    finished: bool,
}

impl RasterCacheWriter {
    pub(crate) fn create(
        cache_dir: &Path,
        key: &str,
        layout: RasterLayout,
    ) -> Result<Self, LargeImageError> {
        validate_cache_key(key)?;
        fs::create_dir_all(cache_dir)?;
        let partial_path = cache_dir.join(format!("{key}.bgra.part"));
        let finished_path = cache_dir.join(format!("{key}.bgra"));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&partial_path)?;
        file.set_len(u64::try_from(layout.len()).map_err(|_| LargeImageError::SizeOverflow)?)?;
        // SAFETY: the file is held open for the mapping lifetime and its length is
        // fixed to exactly `layout.len` before the mapping is created.
        let map = unsafe { MmapMut::map_mut(&file)? };
        Ok(Self {
            map: Some(map),
            file: Some(file),
            layout,
            partial_path,
            finished_path,
            finished: false,
        })
    }

    pub(crate) fn row_mut(&mut self, row: u32) -> Option<&mut [u8]> {
        let range = self.layout.row_range(row)?;
        self.map.as_mut()?.get_mut(range)
    }

    #[cfg(test)]
    pub(crate) fn partial_path(&self) -> &Path {
        &self.partial_path
    }

    #[cfg(test)]
    pub(crate) fn finished_path(&self) -> &Path {
        &self.finished_path
    }

    pub(crate) fn finish(mut self) -> Result<PathBuf, LargeImageError> {
        if let Some(map) = self.map.as_mut() {
            map.flush()?;
        }
        drop(self.map.take());
        drop(self.file.take());
        if self.finished_path.exists() {
            fs::remove_file(&self.finished_path)?;
        }
        fs::rename(&self.partial_path, &self.finished_path)?;
        self.finished = true;
        Ok(self.finished_path.clone())
    }
}

impl Drop for RasterCacheWriter {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        drop(self.map.take());
        drop(self.file.take());
        let _ = fs::remove_file(&self.partial_path);
    }
}

pub(crate) struct RasterCacheReader {
    map: Mmap,
    layout: RasterLayout,
}

impl RasterCacheReader {
    pub(crate) fn open(path: &Path, layout: RasterLayout) -> Result<Self, LargeImageError> {
        let file = File::open(path)?;
        let actual = file.metadata()?.len();
        let expected = u64::try_from(layout.len()).map_err(|_| LargeImageError::SizeOverflow)?;
        if actual != expected {
            return Err(LargeImageError::InvalidCacheLength { expected, actual });
        }
        // SAFETY: the immutable mapping cannot resize the file, and callers only
        // receive shared slices bounded by the validated layout.
        let map = unsafe { Mmap::map(&file)? };
        Ok(Self { map, layout })
    }

    pub(crate) fn row(&self, row: u32) -> Option<&[u8]> {
        self.map.get(self.layout.row_range(row)?)
    }
}

fn validate_cache_key(key: &str) -> Result<(), LargeImageError> {
    let valid = !key.is_empty()
        && key != "."
        && key != ".."
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    if valid {
        Ok(())
    } else {
        Err(LargeImageError::InvalidCacheKey)
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, time::SystemTime};

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("lumia-large-cache-{nonce}-{name}"))
    }

    #[test]
    fn raster_layout_checks_lengths_and_rows() {
        let layout = RasterLayout::new(34752, 11584).unwrap();
        assert_eq!(layout.width(), 34752);
        assert_eq!(layout.height(), 11584);
        assert_eq!(layout.row_bytes(), 139_008);
        assert_eq!(layout.len(), 1_610_268_672);
        assert_eq!(layout.row_range(1), Some(139_008..278_016));
        assert_eq!(layout.row_range(11584), None);
        assert!(RasterLayout::new(0, 10).is_err());
        assert!(RasterLayout::new(u32::MAX, u32::MAX).is_err());
    }

    #[test]
    fn cache_key_changes_when_source_changes() {
        let dir = temp_dir("key");
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("image.png");
        fs::write(&source, b"one").unwrap();
        let first = RasterCacheKey::from_source(&source).unwrap();
        assert_eq!(first.as_str().len(), 16);
        fs::write(&source, b"different length").unwrap();
        let second = RasterCacheKey::from_source(&source).unwrap();
        assert_ne!(first, second);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn disk_space_check_reports_required_and_available_bytes() {
        assert!(ensure_disk_space(100, 100).is_ok());
        assert!(matches!(
            ensure_disk_space(101, 100),
            Err(LargeImageError::InsufficientDiskSpace {
                required: 101,
                available: 100
            })
        ));
    }

    #[test]
    fn mapped_cache_writes_rows_and_atomically_finishes() {
        let dir = temp_dir("mapped");
        fs::create_dir_all(&dir).unwrap();
        let layout = RasterLayout::new(2, 2).unwrap();
        let mut writer = RasterCacheWriter::create(&dir, "sample", layout).unwrap();
        let partial = writer.partial_path().to_path_buf();
        let finished = writer.finished_path().to_path_buf();
        assert!(partial.starts_with(&dir));
        assert!(partial.exists());
        assert!(!finished.exists());

        writer.row_mut(0).unwrap().copy_from_slice(&[1; 8]);
        writer.row_mut(1).unwrap().copy_from_slice(&[2; 8]);
        let path = writer.finish().unwrap();
        assert_eq!(path, finished);
        assert!(!partial.exists());

        let reader = RasterCacheReader::open(&path, layout).unwrap();
        assert_eq!(reader.row(0).unwrap(), &[1; 8]);
        assert_eq!(reader.row(1).unwrap(), &[2; 8]);
        drop(reader);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unfinished_cache_removes_partial_file() {
        let dir = temp_dir("partial");
        fs::create_dir_all(&dir).unwrap();
        let layout = RasterLayout::new(1, 1).unwrap();
        let partial = {
            let writer = RasterCacheWriter::create(&dir, "drop", layout).unwrap();
            writer.partial_path().to_path_buf()
        };
        assert!(!partial.exists());
        fs::remove_dir_all(dir).unwrap();
    }
}
