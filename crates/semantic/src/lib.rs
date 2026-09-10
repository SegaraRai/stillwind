//! Cross-module identities, finite candidate values, and module summaries.

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
pub use module::{IntegrationDiagnostic, ModuleId, TypeFacts, TypeFactsUpdate};
pub use module_summary::{ImportBinding, ModuleSpecifier, ModuleSummary, ReExport};
pub use symbolic::{Candidates, ExportedValue, RefAlternative, SymbolicError, SymbolicValue};
pub use type_facts::TypeFact;
