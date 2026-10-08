//! Source revisions and the ID allocator shared by one project scope.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use super::{FileId, SourceError, SourceFile, SourceUri};

/// Failure to register or resolve a project source buffer.
#[derive(Clone, Debug, thiserror::Error)]
pub enum SourceRegistryError {
    /// No further IDs can be issued in this source scope.
    #[error("source identifier space is exhausted")]
    FileIdExhausted,
    /// A span names a buffer absent from this registry.
    #[error("unknown source file {0:?}")]
    UnknownFile(FileId),
    /// An ID already belongs to a different immutable source revision.
    #[error("source file ID {0:?} belongs to another revision")]
    ConflictingFile(FileId),
    /// A generated buffer has no original document to report.
    #[error("source file {0:?} has no reportable document URI")]
    UnreportableSource(FileId),
    /// The registered text or requested range is invalid.
    #[error("{0}")]
    Source(#[from] SourceError),
}

/// Source buffers and the ID allocator shared by one project session and its loaders.
#[derive(Clone, Debug)]
pub struct SourceRegistry {
    files: BTreeMap<FileId, Arc<SourceFile>>,
    next_id: Arc<AtomicU32>,
}

impl Default for SourceRegistry {
    fn default() -> Self {
        Self {
            files: BTreeMap::new(),
            next_id: Arc::new(AtomicU32::new(1 << 31)),
        }
    }
}

impl SourceRegistry {
    fn next_id(&self) -> Result<FileId, SourceRegistryError> {
        let id = self
            .next_id
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                next.checked_add(1).filter(|id| *id < u32::MAX)
            })
            .map_err(|_| SourceRegistryError::FileIdExhausted)?;
        Ok(FileId(id))
    }

    /// Registers one immutable source revision and issues its compilation file ID.
    ///
    /// `None` for `uri` identifies an internal buffer whose positions cannot be
    /// published without projection to an original document.
    ///
    /// # Errors
    ///
    /// Returns an error when the source is too large or the ID space is exhausted.
    pub fn register(
        &mut self,
        uri: Option<SourceUri>,
        source_map_source: Option<Arc<str>>,
        text: impl Into<Arc<str>>,
    ) -> Result<Arc<SourceFile>, SourceRegistryError> {
        let id = self.next_id()?;
        let source = Arc::new(SourceFile::new(id, uri, source_map_source, text)?);
        self.files.insert(source.id(), source.clone());
        Ok(source)
    }

    /// Issues an ID for a generated analysis buffer that will be built after projection.
    ///
    /// # Errors
    ///
    /// Returns an error when the ID space is exhausted.
    pub fn reserve_generated_id(&mut self) -> Result<FileId, SourceRegistryError> {
        self.next_id()
    }

    /// Starts an empty registry that issues IDs from this registry's project scope.
    #[must_use]
    pub fn fork_empty(&self) -> Self {
        Self {
            files: BTreeMap::new(),
            next_id: self.next_id.clone(),
        }
    }

    /// Whether two registries issue IDs from the same project scope.
    pub fn shares_ids_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.next_id, &other.next_id)
    }

    /// Keeps later allocations above an already issued source ID in this scope.
    pub fn reserve_after(&self, id: FileId) {
        self.next_id
            .fetch_max(id.0.saturating_add(1), Ordering::Relaxed);
    }

    /// Finds the registered source that issued a span's file ID.
    ///
    /// # Errors
    ///
    /// Returns an error when the ID was not issued by this registry.
    pub fn source(&self, id: FileId) -> Result<&Arc<SourceFile>, SourceRegistryError> {
        self.files
            .get(&id)
            .ok_or(SourceRegistryError::UnknownFile(id))
    }

    /// Returns the source with the smallest issued ID, if one exists.
    pub fn first(&self) -> Option<&Arc<SourceFile>> {
        self.files.values().next()
    }

    /// Retains an already registered source revision from another project component.
    ///
    /// The source carries its ID, URI, text, and coordinate table together; a
    /// conflicting ID is rejected instead of replacing an existing revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the ID already belongs to a different revision.
    pub fn retain(&mut self, source: Arc<SourceFile>) -> Result<(), SourceRegistryError> {
        if let Some(existing) = self.files.get(&source.id())
            && !Arc::ptr_eq(existing, &source)
        {
            return Err(SourceRegistryError::ConflictingFile(source.id()));
        }
        self.files.insert(source.id(), source);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_ids_are_shared_only_with_explicitly_forked_registries() {
        let mut first = SourceRegistry::default();
        let mut reload = first.fork_empty();
        let original = first
            .register(Some(SourceUri::parse("untitled:one").unwrap()), None, "a")
            .unwrap();
        let revised = reload
            .register(Some(SourceUri::parse("untitled:one").unwrap()), None, "b")
            .unwrap();
        assert_ne!(original.id(), revised.id());
        assert!(first.shares_ids_with(&reload));
        let mut separate = SourceRegistry::default();
        assert!(!first.shares_ids_with(&separate));
        let foreign = separate
            .register(Some(SourceUri::parse("untitled:other").unwrap()), None, "c")
            .unwrap();
        assert_eq!(original.id(), foreign.id());
        assert_eq!(revised.uri().unwrap().as_str(), "untitled:one");
        assert_eq!(revised.text(), "b");
    }
}
