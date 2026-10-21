//! Frontend-independent module compilation, code generation, and stylesheet queries.
//!
//! The crate owns IR lowering, class and symbol allocation, source replacements,
//! generated CSS rules, framework type declarations, and editor queries. Its inputs
//! are a [`stillwind_ir::ModuleAnalysis`] and an immutable
//! [`stillwind_design_system::DesignSystemSnapshot`]; it performs no I/O and depends
//! on neither the JavaScript frontend nor the project layer, so the host supplies
//! opaque source names and contents.
//!
//! Result spans address the original, unedited source buffer. Default names derive from
//! canonical content and declaration identity. Host name generators must return the
//! same name for the same request to preserve names across compilations. When a
//! compilation fails, diagnostics still address the real source, and transforms,
//! classes, and symbols stay empty.
//!
//! `module_compiler` compiles one analyzed module through the lowering passes in
//! `compiler`. `compiled_module` applies the resulting edits, and `module_css` owns
//! CSS occurrences and rule identities. `identifiers` allocates names, and `options`
//! configures compilation. `imports` plans macro import removal, while `runtime`
//! allocates framework helper bindings.
//!
//! `style_analysis` resolves composed supplies and checks their properties and markers.
//! `generated` composes JavaScript fragments with expression provenance. `transforms`
//! converts their source ranges to UTF-16 mappings for host source-map generation.
//! `renderer` assembles stylesheet slots, and `stylesheet_map` retains CSS origins.
//! `query` answers editor requests. `editor_lint` detects canonical spellings and
//! collects utility literals for `candidate_fixes`, which detects redundant variants
//! and untyped variables with their fixes.

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

mod candidate_fixes;
mod compiled_module;
mod compiler;
mod editor_lint;
mod generated;
mod identifiers;
mod imports;
mod module_compiler;
mod module_css;
mod options;
mod query;
mod renderer;
mod runtime;
mod style_analysis;
mod stylesheet_map;
mod transforms;

pub use compiled_module::{
    CompilationDependencies, CompiledModule, ModuleArtifactKey, ModuleResourceReference,
    ModuleTransformError,
};
pub use compiler::{
    CompileResult, Compiler, CssContribution, EditorSite, EditorSiteKind, GeneratedClass,
    GeneratedSymbol, GeneratedSymbolKind, GuardStep, ModuleEditor, StylesheetLint, SymbolOrigin,
    lint_stylesheets,
};
pub use identifiers::{
    AnchorName, AnchorNameRequest, ClassContext, ClassName, ConditionContext, ConditionHelperName,
    ConditionHelperRequest, ConditionRole, ContainerName, ContainerNameRequest, DeclarationContext,
    GeneratedName, GeneratorSet, IdentifierContext, IdentifierGenerator, IdentifierHash,
    IdentifierKind, IdentifierMode, IdentifierNameError, IdentifierPool, Identifiers,
    KeyframesName, KeyframesNameRequest, Label, LayerContext, LayerName, LayerNameRequest,
    LocatorContext, LocatorProjection, NameRequest, RefMarker, RefMarkerName, RejectedIdentifier,
    RejectedNames, Token, UtilityClass, ViewTransitionClass, ViewTransitionClassRequest,
    ViewTransitionSlotName, ViewTransitionSlotRequest,
};
pub use imports::plan_macro_import_removals;
pub use module_compiler::ModuleCompiler;
pub use module_css::{GeneratedRule, ModuleCssArtifact, RuleKey};
pub use options::{CompilerOptions, RootFontSize};
pub use query::{
    AddedMarker, CandidateContext, CandidateFailure, CandidateHover, CandidateResolution,
    ColorEquivalents, CompletionInput, CompletionItem, DesignSystemQuery, PreviewAnnotations,
    StylesheetColor, StylesheetCompletion, StylesheetCompletionOptions, StylesheetEdit,
    StylesheetHover, StylesheetPluginPath, StylesheetSortOptions, completion_detail,
    editor_css_source, prepare_canonical_index, stylesheet_plugin_paths,
};
pub use renderer::{
    RenderedStylesheet, StylesheetPlacement, StylesheetRenderError, StylesheetRenderer,
    merge_rejected_names,
};
pub use style_analysis::{
    ResolvedStyle, StyleEffects, StyleResolver, check_style, is_style_analysis_lint,
};
pub use stylesheet_map::{MappedStylesheet, StylesheetMapping};
pub use transforms::{
    ExpressionMapping, ExpressionTransformMapping, RenderedSource, SourceTransform, TransformError,
    TransformMapping, render_transforms,
};

#[cfg(test)]
mod test_support;
