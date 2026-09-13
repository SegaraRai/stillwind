//! Frontend-independent module compilation, code generation, and stylesheet queries.
//!
//! The crate owns IR lowering, class and symbol allocation, source replacements,
//! generated CSS rules, framework type declarations, and editor queries. Its inputs
//! are a [`stillwind_ir::ModuleAnalysis`] and an immutable
//! [`stillwind_design_system::DesignSystemSnapshot`]; it performs no I/O and depends
//! on neither the JavaScript frontend nor the project layer, so the host supplies
//! opaque source names and contents.
//!
//! Two invariants hold for everything the crate returns. Spans always address the
//! original, unedited source buffer, never generated text, and every generated name
//! is a pure function of canonical content plus callsite identity, so recompiling the
//! same inputs produces the same names. When a compilation fails, diagnostics are
//! still reported against the real source and the transforms, classes, and symbols
//! stay empty instead of describing a partially applied edit plan.

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

mod candidate;
mod compiler;
mod imports;
mod module_compiler;
mod options;
mod query;
mod renderer;
mod runtime;
mod stylesheet_map;
mod transforms;

pub use compiler::{
    CompileResult, Compiler, CompilerThemeError, CssContribution, GeneratedClass, GeneratedSymbol,
    GuardStep, StylesheetError,
};
pub use imports::{ImportPlanError, plan_macro_import_removals};
pub use module_compiler::{
    CompilationDependencies, CompiledModule, GeneratedRule, ModuleArtifactKey, ModuleCompiler,
    ModuleCssArtifact, ModuleResourceReference, ModuleTransformError, RuleKey,
};
pub use options::CompilerOptions;
pub use query::{
    CandidateDiagnostic, CandidateHover, CandidateResolution, CompletionInput, CompletionItem,
    DesignSystemQuery,
};
pub use renderer::{StylesheetPlacement, StylesheetRenderer};
pub use stylesheet_map::{MappedStylesheet, StylesheetMapping};
pub use transforms::{
    RenderedSource, SourceTransform, TransformError, TransformMapping, render_transforms,
};

#[cfg(test)]
#[path = "../../test-support/css.rs"]
mod css_fixture;

#[cfg(test)]
#[path = "../../test-support/defaults.rs"]
mod test_defaults;
