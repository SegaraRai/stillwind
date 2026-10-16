//! Single entry point from a candidate string to a design-system resolution.
//!
//! Every consumer in this crate - generated utilities, animation options, and
//! authored `@apply` - resolves through [`CandidateResolver`], so a change of resolver
//! semantics cannot leave one call path behind. A candidate that this snapshot does not
//! resolve returns the design system's explanation. Malformed matched definitions carry
//! their causes to the caller, which selects a reporting position.

use stillwind_design_system::{
    ResolvedUtility, ThemeIndex, UtilityFailure, UtilityResolutionError, resolve_utility,
};

/// Stateless candidate resolution against the theme of one snapshot.
pub(crate) struct CandidateResolver;

impl CandidateResolver {
    /// Resolves a raw candidate against `theme`. Variants are parsed but not resolved.
    ///
    /// # Errors
    ///
    /// Returns an invalid requirement from a matching utility definition.
    pub(crate) fn resolve(
        raw: &str,
        theme: &ThemeIndex,
    ) -> Result<Result<ResolvedUtility, UtilityFailure>, UtilityResolutionError> {
        resolve_utility(raw, theme)
    }
}
