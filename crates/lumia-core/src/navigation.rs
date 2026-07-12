use std::io;
use std::path::{Path, PathBuf};

use crate::is_supported_image_extension;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FolderNavigation {
    paths: Vec<PathBuf>,
}

impl FolderNavigation {
    pub fn scan(current_path: &Path) -> io::Result<Self> {
        let Some(parent) = current_path.parent() else {
            return Ok(Self::default());
        };
        let mut paths = std::fs::read_dir(parent)?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| {
                path.extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(is_supported_image_extension)
            })
            .collect::<Vec<_>>();
        paths.sort_by(|left, right| {
            left.file_name()
                .and_then(|name| name.to_str())
                .cmp(&right.file_name().and_then(|name| name.to_str()))
        });
        Ok(Self { paths })
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.paths.iter().any(|candidate| candidate == path)
    }

    pub fn current_index(&self, current_path: &Path) -> Option<usize> {
        self.paths.iter().position(|path| path == current_path)
    }

    pub fn step_path(&self, current_path: &Path, step: i32) -> Option<&Path> {
        let current_index = self.current_index(current_path)?;
        let next_index = (current_index as i32 + step).rem_euclid(self.paths.len() as i32) as usize;
        self.paths.get(next_index).map(PathBuf::as_path)
    }

    pub fn adjacent_paths(&self, current_path: &Path) -> Vec<PathBuf> {
        let Some(current_index) = self.current_index(current_path) else {
            return Vec::new();
        };
        [-1, 1]
            .into_iter()
            .filter_map(|offset| {
                let index = current_index as i32 + offset;
                (index >= 0)
                    .then_some(index as usize)
                    .and_then(|index| self.paths.get(index))
                    .cloned()
            })
            .collect()
    }

    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temp_dir() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("lumia-navigation-{nonce}"))
    }

    #[test]
    fn scan_filters_sorts_and_navigates_with_wraparound() {
        let dir = temp_dir();
        std::fs::create_dir(&dir).unwrap();
        for name in ["c.PNG", "a.jpg", "b.txt", "b.heic", "d.psd", "e.PSB"] {
            std::fs::write(dir.join(name), []).unwrap();
        }

        let current = dir.join("a.jpg");
        let navigation = FolderNavigation::scan(&current).unwrap();
        let names = navigation
            .paths()
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(names, ["a.jpg", "b.heic", "c.PNG", "d.psd", "e.PSB"]);
        assert_eq!(
            navigation.step_path(&current, -1),
            Some(dir.join("e.PSB").as_path())
        );
        assert_eq!(
            navigation.step_path(&current, 1),
            Some(dir.join("b.heic").as_path())
        );
        assert_eq!(
            navigation.adjacent_paths(&current),
            vec![dir.join("b.heic")]
        );

        std::fs::remove_dir_all(dir).unwrap();
    }
}
