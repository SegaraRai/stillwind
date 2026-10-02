//! Cross-module identities, symbolic candidate values, and module summaries.
//!
//! This crate is the shared vocabulary between the JS frontend (`js-oxc`), the host
//! type checker, and the compiler: declaration-site [`IdentityId`] values, the
//! [`SymbolicValue`] / [`ExportedValue`] algebra that carries candidate sets across
//! module boundaries, per-module [`ModuleSummary`] values, and the versioned
//! [`TypeFacts`] a checker hands back for one module.
//!
//! Two rules dominate. Identity is created at the declaration site and derives from the
//! module identity plus the declaration span, so an asset keeps one identity however it
//! is imported or re-exported. Symbolic values stay sound: a value becomes finite only
//! when the frontend proved the candidates, because a wrongly finite set pins CSS that
//! may never apply. No I/O happens here, and nothing in this crate knows what CSS will
//! be generated.

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

pub use assets::{AssetMetadata, AssetRecord};
pub use identity::{AssetKind, IdentityId};
pub use module::{IntegrationDiagnostic, TypeFacts, TypeFactsUpdate};
pub use module_summary::{ImportBinding, ModuleSpecifier, ModuleSummary, ReExport};
pub use symbolic::{Candidates, ExportedValue, SymbolicError, SymbolicValue};
pub use type_facts::TypeFact;
