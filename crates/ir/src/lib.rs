//! Language-neutral intermediate representation shared by all source frontends.
//!
//! This crate connects a frontend (`js-oxc` today) to the backend. It defines the host
//! input contract ([`SourceInput`]), the extracted module model ([`ModuleIr`]), and the
//! two-phase analysis contract ([`SourceAnalyzer`]): a [`ModuleScan`] before TypeScript
//! facts and a [`ModuleAnalysis`] after them.
//!
//! The IR stores original source spans instead of reprinted JavaScript. A frontend that
//! parses a virtual or shadow source must map every span back to the editable original
//! before it returns the IR. [`ModuleIr::try_map_spans`] performs that rewrite
//! transactionally. The frontend keeps ownership of correspondence validation and of
//! its own provenance records.
//!
//! [`validate_composition`] checks dependency cycles and producer availability, and
//! [`ordered_style_calls`] orders the remaining dependencies.
//! [`IncompatibleComposition`] reports composition bases that supply only part of their
//! value. These operations do not emit CSS.
//!
//! Modules:
//!
//! - [`analysis`] defines the scan and finalization interfaces.
//! - [`composition`] checks composition dependencies and orders style calls before
//!   lowering.
//! - [`framework`] selects framework adapter policies.
//! - [`input`] validates editable documents and prepared parser buffers.
//! - [`model`] stores the extracted module model.
//! - [`spans`] maps the spans of the model to other coordinates.

#![warn(missing_docs)]
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
mod composition;
mod framework;
mod input;
mod model;
mod spans;

pub use analysis::{
    CheckerBinding, CheckerProjection, CheckerRole, CheckerSpan, ModuleAnalysis, ModuleScan,
    SourceAnalyzer, TypeProbe, TypeScriptFactsProvider,
};
pub use composition::{
    IncompatibleComposition, StyleDependency, composition_dependencies, ordered_style_calls,
    validate_composition,
};
pub use framework::{
    AdapterKind, FRAMEWORKS, FrameworkSpec, MODULE_CSS_IMPORT, OutputFormat, OutputSpec,
    ProjectionDecision, ProjectionExpression, RUNTIME_IMPORT, RUNTIME_IMPORT_DEV, StyleFormat,
    from_macro_import,
};
pub use input::{
    AttributeQuote, ConverterIdentity, DirectSource, ImportRealm, PreparedSource, ProjectedSource,
    ProjectionRegion, ScriptLanguage, ScriptModuleKind, ScriptSyntax, SourceDocument, SourceFormat,
    SourceInput, SourceInputError, SourceProjection,
};
pub use model::{
    AnchorScope, AssetReference, BranchArm, ClassExpression, ConditionalBranches, ContainerType,
    CssCall, CssInput, CustomPropertyEntry, CvaCall, CvaReplacement, GroupEntry, KeyframesBody,
    KeyframesBranch, KeyframesDefinition, KeyframesFrame, MacroImport, MacroImportSpecifier,
    MediaCall, ModuleImports, ModuleIr, RefCondition, RefDefinition, RefProjection, RuleExpression,
    RuleExpressionError, RuleExpressionErrorKind, SelectEntry, StyleCall, StyleInput, StyleVariant,
    ViewTransitionDefinition, ViewTransitionKey, ViewTransitionName,
};
