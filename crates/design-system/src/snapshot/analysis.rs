//! Shares derived analyses for one immutable revision without process-wide caches.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use stillwind_source::Fingerprint;

#[derive(Default)]
struct AnalysisEntry {
    value: OnceLock<Arc<dyn Any + Send + Sync>>,
}

/// Owns analyses whose keys include every setting used by their constructor.
#[derive(Default)]
pub(super) struct SnapshotAnalyses {
    entries: Mutex<HashMap<(TypeId, Fingerprint), Arc<AnalysisEntry>>>,
}

impl SnapshotAnalyses {
    pub(super) fn get_or_init<T: Any + Send + Sync>(
        &self,
        key: Fingerprint,
        init: impl FnOnce() -> T,
    ) -> Arc<T> {
        let slot = self
            .entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry((TypeId::of::<T>(), key))
            .or_default()
            .clone();
        let value = slot.value.get_or_init(|| Arc::new(init())).clone();
        match value.downcast::<T>() {
            Ok(value) => value,
            #[expect(
                clippy::unreachable,
                reason = "the cache owns each TypeId and never accepts an erased value from callers"
            )]
            Err(_) => unreachable!("snapshot analysis keys include the concrete TypeId"),
        }
    }
}

impl fmt::Debug for SnapshotAnalyses {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SnapshotAnalyses")
            .field(
                "entries",
                &self
                    .entries
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .len(),
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use lean_string::LeanString;

    use super::*;

    #[test]
    fn concurrent_callers_share_preparation_and_keys_keep_values_independent() {
        let cache = SnapshotAnalyses::default();
        let prepared = AtomicUsize::new(0);
        let key = Fingerprint::of("options");
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    scope.spawn(|| {
                        cache.get_or_init(key.clone(), || {
                            prepared.fetch_add(1, Ordering::SeqCst);
                            7usize
                        })
                    })
                })
                .collect();
            let results: Vec<_> = handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect();
            assert!(
                results
                    .iter()
                    .all(|result| Arc::ptr_eq(result, &results[0]))
            );
        });
        assert_eq!(prepared.load(Ordering::SeqCst), 1);
        assert_eq!(*cache.get_or_init(Fingerprint::of("other"), || 9usize), 9);
        assert_eq!(
            cache
                .get_or_init(key, || LeanString::from("different type"))
                .as_str(),
            "different type"
        );
    }
}
