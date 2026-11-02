//! Composition analysis of the properties and markers recorded during lowering.
//!
//! Register the current artifacts with [`StyleResolver`], resolve each call by
//! identity, and pass the result to [`check_style`]. Hosts apply lint settings and
//! source suppression. A missing base keeps its conditions and prevents conclusions
//! that rely on the absence of a property.
//!
//! Modules:
//!
//! - [`checks`] combines the findings and identifies the rules that hosts must replace
//!   after an update.
//! - [`composition`] resolves bases and keeps decisions and origins.
//! - [`effects`] owns the direct contribution records.
//! - [`properties`] checks attachments and property interference.
//! - [`utilities`] compares direct candidates with the detection settings of the
//!   producing call.
//! - [`view_transition`] checks view-transition names and class replacement.

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
