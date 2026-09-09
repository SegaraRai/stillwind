//! Public orchestration API for the first compiler slice.

mod analysis;
mod candidate;
mod compiler;
mod design_system;
mod fingerprint;
mod loader;
mod module_compiler;
mod session;
mod transforms;

pub use analysis::{
    IntegrationDiagnostic, ModuleAnalysis, ModuleId, ModuleScan, OxcSourceAnalyzer, SourceAnalyzer,
    TypeFacts, TypeFactsUpdate, TypeProbe, TypeScriptFactsProvider,
};
pub use compiler::{
    CompileResult, Compiler, CompilerOptions, CompilerOptionsError, CompilerThemeError,
    GeneratedClass, GeneratedSymbol,
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
pub use transforms::{SourceTransform, TransformError};

pub(crate) use compiler::{
    class_name, emit_utility_rules, render_react_types, render_stylesheet, resolve_utility_targets,
    unresolved_utility_rule,
};
