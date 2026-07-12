use std::collections::HashMap;

use lumia_core::TileCoordinate;

pub(crate) struct TileCache<T> {
    entries: HashMap<TileCoordinate, CacheEntry<T>>,
    max_bytes: usize,
    used_bytes: usize,
    clock: u64,
}

struct CacheEntry<T> {
    value: T,
    bytes: usize,
    last_used: u64,
}

impl<T> TileCache<T> {
    pub(crate) fn new(max_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_bytes,
            used_bytes: 0,
            clock: 0,
        }
    }

    pub(crate) fn insert(&mut self, key: TileCoordinate, value: T, bytes: usize) {
        if bytes > self.max_bytes {
            return;
        }
        if let Some(previous) = self.entries.remove(&key) {
            self.used_bytes -= previous.bytes;
        }
        self.clock = self.clock.wrapping_add(1);
        self.used_bytes += bytes;
        self.entries.insert(
            key,
            CacheEntry {
                value,
                bytes,
                last_used: self.clock,
            },
        );
        while self.used_bytes > self.max_bytes {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| *key)
            else {
                break;
            };
            if let Some(removed) = self.entries.remove(&oldest) {
                self.used_bytes -= removed.bytes;
            }
        }
    }

    pub(crate) fn get(&mut self, key: &TileCoordinate) -> Option<&T> {
        self.clock = self.clock.wrapping_add(1);
        let entry = self.entries.get_mut(key)?;
        entry.last_used = self.clock;
        Some(&entry.value)
    }

    pub(crate) fn peek(&self, key: &TileCoordinate) -> Option<&T> {
        self.entries.get(key).map(|entry| &entry.value)
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.used_bytes = 0;
    }

    pub(crate) fn contains(&self, key: &TileCoordinate) -> bool {
        self.entries.contains_key(key)
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) const fn used_bytes(&self) -> usize {
        self.used_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumia_core::TileCoordinate;

    #[test]
    fn lru_evicts_oldest_tiles_to_stay_within_budget() {
        let mut cache = TileCache::new(10);
        let first = TileCoordinate::new(0, 0, 0);
        let second = TileCoordinate::new(0, 1, 0);
        let third = TileCoordinate::new(0, 2, 0);
        cache.insert(first, 1_u8, 4);
        cache.insert(second, 2_u8, 4);
        assert_eq!(cache.get(&first), Some(&1));
        cache.insert(third, 3_u8, 4);
        assert!(cache.contains(&first));
        assert!(!cache.contains(&second));
        assert!(cache.contains(&third));
        assert_eq!(cache.used_bytes(), 8);
    }

    #[test]
    fn oversized_item_is_not_cached() {
        let mut cache = TileCache::new(4);
        cache.insert(TileCoordinate::new(0, 0, 0), 1_u8, 5);
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.used_bytes(), 0);
    }
}
