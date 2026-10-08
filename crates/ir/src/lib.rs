//! Language-neutral intermediate representation shared by all source frontends.
//!
//! This crate is the seam between a frontend (`js-oxc` today, other parsers later) and
//! the backend. It defines three things: the host input contract ([`SourceInput`]), the
//! extracted module model ([`ModuleIr`]), and the two-phase analysis contract
//! ([`SourceAnalyzer`]: `ModuleScan` before TypeScript facts, [`ModuleAnalysis`] after
//! them).
//!
//! The IR stores original source spans rather than re-printed JavaScript. A frontend
//! that parses a virtual or shadow source must map every span back to the editable
//! original before handing the IR on; [`ModuleIr::try_map_spans`] performs that rewrite
//! transactionally, and the frontend keeps ownership of correspondence validation and
//! of its own provenance records.
//!
//! [`validate_composition`] and [`ordered_style_calls`] are pure checks over the IR:
//! they report dependency cycles and produce a deterministic producer order, but they
//! never emit CSS or resolve candidates.
//!
//! `input` validates editable documents and prepared parser buffers. `analysis` defines
//! the scan and finalization interfaces. `model` stores the extracted IR, and `spans`
//! maps its coordinates. `framework` selects adapter policies. `composition` checks
//! dependencies and orders style calls before lowering.

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
    StyleDependency, composition_dependencies, ordered_style_calls, validate_composition,
};
pub use framework::{
    AdapterKind, FRAMEWORKS, FrameworkSpec, MODULE_CSS_IMPORT, ProjectionDecision,
    ProjectionExpression, StyleFormat, from_macro_import,
};
pub use input::{
    AttributeQuote, ConverterIdentity, DirectSource, ImportRealm, PreparedSource, ProjectedSource,
    ProjectionRegion, ScriptLanguage, ScriptModuleKind, ScriptSyntax, SourceDocument, SourceFormat,
    SourceInput, SourceInputError, SourceProjection,
};
pub use model::{
    AnchorScope, AssetReference, AttributeEntry, ClassExpression, ConditionalBranches,
    ContainerType, CustomPropertyEntry, GroupEntry, KeyframesBody, KeyframesBranch,
    KeyframesDefinition, KeyframesFrame, MacroImport, MacroImportSpecifier, ModuleImports,
    ModuleIr, RefCondition, RefDefinition, RefProjection, RuleExpression, RuleExpressionError,
    RuleExpressionErrorKind, StyleCall, StyleInput, StyleVariant, ViewTransitionDefinition,
    ViewTransitionKey, ViewTransitionName,
};
