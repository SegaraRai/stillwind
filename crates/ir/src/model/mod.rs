//! Extracted module model: what a frontend found in one module.
//!
//! Every [`stillwind_source::Span`] here refers to the editable original source, never
//! to a virtual or converted view. The model keeps author intent, not generated CSS: a
//! [`StyleInput`] says which expression supplies utilities, and it does not name
//! classes or declarations.
//!
//! Order has meaning and every consumer must keep it. [`ModuleIr::style_calls`],
//! [`ModuleIr::ref_definitions`], and the input entries inside one call stay in source
//! order to keep CSS and contribution precedence. Value expressions must satisfy the
//! macro purity contract. Folding may reorder their evaluation and omit unused values,
//! but it keeps the guards of selected contributions.
//!
//! Modules:
//!
//! - [`assets`] describes named declarations and their uses.
//! - [`expression`] stores recursive class and rule contributions and validates
//!   conversion to the rule-only subset.
//! - [`module`] groups the declarations and calls extracted from one source, including
//!   `style.media()` and `style.css()` calls.
//! - [`style`] describes call inputs and variants.

mod assets;
mod expression;
mod module;
mod style;

pub use assets::{
    AssetReference, KeyframesBody, KeyframesBranch, KeyframesDefinition, KeyframesFrame,
    RefDefinition, ViewTransitionDefinition, ViewTransitionKey, ViewTransitionName,
};
pub use expression::{
    BranchArm, ClassExpression, ConditionalBranches, CustomPropertyEntry, GroupEntry,
    RuleExpression, RuleExpressionError, RuleExpressionErrorKind, SelectEntry,
};
pub use module::{
    CssCall, CssInput, CvaCall, CvaReplacement, MacroImport, MacroImportSpecifier, MediaCall,
    ModuleImports, ModuleIr,
};
pub use style::{
    AnchorScope, ContainerType, RefCondition, RefProjection, StyleCall, StyleInput, StyleVariant,
};
