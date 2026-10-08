//! Composition analysis of properties and markers recorded during lowering.
//!
//! Register the current artifacts with `StyleResolver`, resolve each call by identity,
//! and pass the result to `check_style`. Hosts apply lint settings and source
//! suppression. Missing bases retain their conditions and prevent compatible absence
//! conclusions.
//!
//! `effects` owns the direct contribution records. `composition` resolves bases and
//! preserves decisions and origins. `properties` checks attachments and property
//! interference. `utilities` compares direct candidates using the producing call's
//! detection settings. `view_transition` checks names and class replacement. `checks`
//! combines their findings and identifies the rules that hosts must replace after an
//! update.

mod checks;
mod composition;
mod effects;
mod properties;
mod utilities;
mod view_transition;

pub use checks::{check_style, is_style_analysis_lint};
pub use composition::{ResolvedStyle, StyleResolver};
pub use effects::StyleEffects;

pub(crate) use effects::{
    DeclarationValue, RuleContext, RuleSubject, StyleEffect, StyleSupply, SupplyKind, UseId,
};

#[cfg(test)]
mod test_support;
