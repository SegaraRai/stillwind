//! Host module identity and the versioned type-fact contract.
//!
//! [`ModuleId`] is the opaque key the host uses for a module, kept verbatim.
//! [`TypeFacts`] is what the TypeScript checker returns for one module: a schema tag,
//! the module it describes, the source revision it was computed from, a revision for
//! the semantic payload, expression facts and asset metadata. [`TypeFactsUpdate`]
//! batches those across modules together with integration diagnostics.
//!
//! Staleness is part of the contract. The frontend accepts facts only when schema,
//! module id and source revision all match the scan, and it carries the semantic
//! revision forward as the cache key of the finalized analysis.

use std::collections::BTreeMap;

use stillwind_diagnostics::CompileErrorKind;
use stillwind_source::{Fingerprint, ModuleId};

use crate::{AssetMetadata, IdentityId, TypeFact};

/// Type facts for one module, as produced by the host type checker.
///
/// Facts are a cache of expensive checker work, never a source of truth: the frontend
/// re-parses the module around them and rejects the whole payload when any consistency
/// check fails.
#[derive(Clone, Debug)]
pub struct TypeFacts {
    /// Schema tag of this payload; must equal [`Self::SCHEMA`].
    pub schema: u32,
    /// Module the facts describe; must equal the scanned module id.
    pub module_id: ModuleId,
    /// Fingerprint of the exact source text the facts were computed from.
    pub source_revision: Fingerprint,
    /// Host-provided revision of the semantic payload. Current Rust adapters hash the
    /// facts and asset metadata; checker version and configuration are not separately
    /// included. Hosts must refresh the payload when their checker inputs change.
    pub semantic_revision: Fingerprint,
    /// Expression facts keyed by byte span. Each span must satisfy
    /// `start <= end` and lie inside the scanned source.
    pub facts: Vec<TypeFact>,
    /// Metadata for the declaration-site identities the facts mention.
    pub assets: BTreeMap<IdentityId, AssetMetadata>,
}

impl TypeFacts {
    /// Schema version the frontend understands. Bump it whenever the shape or meaning
    /// of [`TypeFacts`] changes: hosts send payloads produced by an older schema, and
    /// those must be rejected rather than reinterpreted.
    pub const SCHEMA: u32 = 2;
}

/// A problem the host hit while producing type facts for a module.
///
/// The frontend can fall back to local analysis and record why facts were ignored.
/// Module compilation attaches each cause to the module's source and reports it as
/// a fatal error, so these records prevent publication rather than acting as warnings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegrationDiagnostic {
    /// Module the failure concerns, or `None` for a run-wide problem.
    pub module_id: Option<ModuleId>,
    /// Typed compilation failure reported by the fact provider.
    pub kind: CompileErrorKind,
}

/// One round of type facts from the host: facts per module plus diagnostics.
///
/// Modules missing from [`Self::modules`] are analysed without facts; an empty update
/// therefore means "nothing new" rather than "no assets exist".
#[derive(Clone, Debug, Default)]
pub struct TypeFactsUpdate {
    /// Facts keyed by the module id the host used.
    pub modules: BTreeMap<ModuleId, TypeFacts>,
    /// Problems reported while producing the facts.
    pub diagnostics: Vec<IntegrationDiagnostic>,
}
