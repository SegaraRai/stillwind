//! Public orchestration API for the first compiler slice.

mod analysis;
mod module_compiler;
mod query;
mod registry;
mod session;
pub use module_compiler::*;
pub use query::{
    CandidateDiagnostic, CandidateHover, CandidateResolution, CompletionInput, CompletionItem,
    DesignSystemQuery,
};
pub use registry::*;
pub use session::*;
mod design_system;
pub use analysis::*;
mod loader;
pub use design_system::{
    DefinitionFingerprints, DesignSystemRevision, DesignSystemSnapshot, Fingerprint,
};
pub use loader::{
    CssDependencyKind, CssFileInput, CssFileRequest, CssSource, CssSourceEdge, CssSourceGraph,
    CssSourceMapping, CssSourceName, DesignSystemLoadError, DesignSystemLoadStep,
    DesignSystemLoader, DesignSystemOptions,
};

mod compiler;
pub use compiler::{
    CompileResult, Compiler, CompilerOptions, GeneratedClass, GeneratedSymbol, PropsTransform,
};
pub(crate) use compiler::{render_react_types, render_stylesheet};

pub(crate) use compiler::{
    class_name, emit_utility_rules, resolve_utility_targets, unresolved_utility_rule,
};
