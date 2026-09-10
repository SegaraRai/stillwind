//! Public orchestration API for the first compiler slice.

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

mod analysis;
mod candidate;
mod compiler;
mod design_system;
mod fingerprint;
mod loader;
mod module_compiler;
mod session;
mod stylesheet_plan;
mod transforms;

pub use analysis::{
    IntegrationDiagnostic, ModuleAnalysis, ModuleId, ModuleScan, OxcSourceAnalyzer, SourceAnalyzer,
    TypeFacts, TypeFactsUpdate, TypeProbe, TypeScriptFactsProvider,
};
pub use compiler::{
    ClassIdentity, CompileResult, Compiler, CompilerOptions, CompilerOptionsError,
    CompilerThemeError, GeneratedClass, GeneratedSymbol, StylesheetError,
};
pub use design_system::{
    CandidateDiagnostic, CandidateHover, CandidateResolution, CompletionInput, CompletionItem,
    DefinitionFingerprints, DefinitionRegistry, DesignSystemError, DesignSystemQuery,
    DesignSystemRevision, DesignSystemSnapshot, PropertyRegistry, ThemeRegistry, UtilityRegistry,
    VariantRegistry,
};
pub use fingerprint::Fingerprint;
pub use loader::{
    CssDependencyKind, CssFileInput, CssFileRequest, CssImportError, CssSource, CssSourceEdge,
    CssSourceGraph, CssSourceMapping, CssSourceName, DesignSystemLoadError, DesignSystemLoadStep,
    DesignSystemLoader,
};
pub use module_compiler::{
    CompilationDependencies, CompiledModule, GeneratedRule, ModuleArtifactKey, ModuleCompiler,
    ModuleCssArtifact, ModuleTransformError, RuleKey,
};
pub use session::{
    DependencyIndex, ProjectSession, ProjectStylesheet, SessionMode, SessionReloadError,
    SessionUpdate,
};
pub use stylesheet_plan::{
    StylesheetCoverage, StylesheetPlacement, StylesheetPlan, StylesheetPlanError,
    StylesheetSelection,
};
pub use transforms::{SourceTransform, TransformError};

pub(crate) use compiler::{
    class_name, emit_utility_rules, render_react_types, render_stylesheet, resolve_utility_targets,
    unresolved_utility_rule,
};

#[cfg(test)]
#[path = "../../test-support/defaults.rs"]
mod test_defaults;

#[cfg(test)]
#[path = "../../test-support/css.rs"]
mod css_fixture;
