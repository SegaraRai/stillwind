//! Frontend-independent module compilation, code generation, and stylesheet queries.

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
mod module_compiler;
mod options;
mod query;
mod renderer;
mod stylesheet_map;
mod transforms;

pub use compiler::{
    CompileResult, Compiler, CompilerThemeError, GeneratedClass, GeneratedSymbol, StylesheetError,
};
pub use module_compiler::{
    CompilationDependencies, CompiledModule, GeneratedRule, ModuleArtifactKey, ModuleCompiler,
    ModuleCssArtifact, ModuleResourceReference, ModuleTransformError, RuleKey,
};
pub use options::{ClassIdentity, CompilerOptions};
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
