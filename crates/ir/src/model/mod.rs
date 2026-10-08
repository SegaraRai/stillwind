//! Extracted module model: what a frontend found in one module.
//!
//! Every [`Span`] here refers to the editable original source, never to a virtual or
//! converted view. The model keeps author intent, not generated CSS: a [`StyleInput`]
//! still says "these utilities come from this expression", it does not name classes or
//! declarations.
//!
//! Ordering is semantic and must survive any consumer. [`ModuleIr::style_calls`],
//! [`ModuleIr::ref_definitions`], and the input entries inside one call stay in source
//! order to preserve CSS and contribution precedence. Value expressions must satisfy
//! the macro purity contract. Folding may reorder their evaluation and omit unused
//! values while retaining guards for selected contributions.

//!
//! `module` groups declarations and calls extracted from one source. `assets`
//! describes named declarations and their uses. `style` describes call inputs and
//! variants. `expression` stores recursive class and rule contributions and validates
//! conversion to the rule-only subset.

mod assets;
mod expression;
mod module;
mod style;

pub use assets::{
    AssetReference, KeyframesBody, KeyframesBranch, KeyframesDefinition, KeyframesFrame,
    RefDefinition, ViewTransitionDefinition, ViewTransitionKey, ViewTransitionName,
};
pub use expression::{
    AttributeEntry, ClassExpression, ConditionalBranches, CustomPropertyEntry, GroupEntry,
    RuleExpression, RuleExpressionError, RuleExpressionErrorKind,
};
pub use module::{MacroImport, MacroImportSpecifier, ModuleImports, ModuleIr};
pub use style::{
    AnchorScope, ContainerType, RefCondition, RefProjection, StyleCall, StyleInput, StyleVariant,
};
