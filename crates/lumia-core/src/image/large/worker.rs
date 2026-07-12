use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

pub fn large_image_worker_count(available_parallelism: usize) -> usize {
    available_parallelism.saturating_sub(1).max(1).min(6)
}

#[derive(Clone)]
pub struct PixelBudget {
    inner: Arc<PixelBudgetInner>,
}

struct PixelBudgetInner {
    max_bytes: u64,
    in_flight: AtomicU64,
}

impl PixelBudget {
    pub fn new(max_bytes: u64) -> Self {
        Self {
            inner: Arc::new(PixelBudgetInner {
                max_bytes,
                in_flight: AtomicU64::new(0),
            }),
        }
    }

    pub fn try_acquire(&self, bytes: u64) -> Option<PixelPermit> {
        if bytes > self.inner.max_bytes {
            return None;
        }
        let mut current = self.inner.in_flight.load(Ordering::Relaxed);
        loop {
            let next = current.checked_add(bytes)?;
            if next > self.inner.max_bytes {
                return None;
            }
            match self.inner.in_flight.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    return Some(PixelPermit {
                        inner: self.inner.clone(),
                        bytes,
                    });
                }
                Err(actual) => current = actual,
            }
        }
    }

    pub fn in_flight(&self) -> u64 {
        self.inner.in_flight.load(Ordering::Acquire)
    }
}

pub struct PixelPermit {
    inner: Arc<PixelBudgetInner>,
    bytes: u64,
}

impl Drop for PixelPermit {
    fn drop(&mut self) {
        self.inner.in_flight.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_count_leaves_capacity_for_the_ui() {
        assert_eq!(large_image_worker_count(1), 1);
        assert_eq!(large_image_worker_count(2), 1);
        assert_eq!(large_image_worker_count(4), 3);
        assert_eq!(large_image_worker_count(16), 6);
    }

    #[test]
    fn pixel_budget_releases_capacity_with_the_permit() {
        let budget = PixelBudget::new(1024);
        let first = budget.try_acquire(700).unwrap();
        assert!(budget.try_acquire(400).is_none());
        assert_eq!(budget.in_flight(), 700);
        drop(first);
        assert_eq!(budget.in_flight(), 0);
        assert!(budget.try_acquire(1024).is_some());
        assert!(budget.try_acquire(1025).is_none());
    }
}
