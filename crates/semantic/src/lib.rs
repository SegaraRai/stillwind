//! Cross-module identities, finite candidate values, and module summaries.

mod assets;
mod identity;
mod module_summary;
mod symbolic;
mod type_facts;

pub use assets::{AssetMetadata, AssetRecord};
pub use identity::{AssetKind, IdentityId};
pub use module_summary::{ImportBinding, ModuleSummary, ReExport};
pub use symbolic::{Candidates, ExportedValue, RefAlternative, SymbolicError, SymbolicValue};
pub use type_facts::TypeFact;
