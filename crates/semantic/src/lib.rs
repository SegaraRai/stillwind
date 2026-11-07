//! Cross-module identities, symbolic candidate values, and module summaries.
//!
//! This crate is the shared vocabulary of the JS frontend (`js-oxc`), the host type
//! checker, and the compiler. It defines declaration-site [`IdentityId`] values, the
//! [`SymbolicValue`] and [`ExportedValue`] algebra that carries candidate sets across
//! module boundaries, per-module [`ModuleSummary`] values, and the versioned
//! [`TypeFacts`] that a checker returns for one module.
//!
//! Two rules apply:
//!
//! - An identity is created at the declaration site from the module identity and the
//!   declaration span, so an asset keeps one identity however it is imported or
//!   re-exported.
//! - Symbolic values stay sound. A value becomes finite only when the frontend has
//!   proved its candidates, because a wrongly finite set pins CSS that may never apply.
//!
//! No I/O happens here, and nothing in this crate knows which CSS will be generated.
//!
//! Modules:
//!
//! - [`assets`] carries the source records and metadata of declarations.
//! - [`identity`] defines declaration keys.
//! - [`module`] versions checker answers for a source revision.
//! - [`module_summary`] records imports, exports, and declarations.
//! - [`symbolic`] defines candidate sets and their algebra.
//! - [`type_facts`] associates checker answers with expression spans.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]

mod assets;
mod identity;
mod module;
mod module_summary;
mod symbolic;
mod type_facts;

pub use assets::{
    AssetMetadata, AssetRecord, StoredRefCondition, StoredVariantRecord, StoredVariantSegment,
};
pub use identity::{AssetKind, IdentityId};
pub use module::{IntegrationDiagnostic, TypeFacts, TypeFactsUpdate};
pub use module_summary::{ImportBinding, ModuleSpecifier, ModuleSummary, ReExport};
pub use symbolic::{Candidates, ExportedValue, SymbolicError, SymbolicValue};
pub use type_facts::{CheckedValue, TypeFact};
