//! Single entry point from a candidate string to a design-system resolution.
//!
//! Every consumer in this crate - generated utilities, animation options, and
//! authored `@apply` - resolves through [`CandidateResolver`], so a change of resolver
//! semantics cannot leave one call path behind. `None` means the candidate is not
//! known to this snapshot. Malformed matched definitions carry their causes to the
//! caller, which selects a reporting position.

use stillwind_design_system::{
    ResolvedUtility, ThemeIndex, UtilityResolutionError, resolve_utility,
};

/// Stateless candidate resolution against the theme of one snapshot.
pub(crate) struct CandidateResolver;

impl CandidateResolver {
    /// Resolves a raw candidate, variant prefix included, against `theme`.
    ///
    /// The result borrows nothing from `raw`, so an unresolved candidate can be kept
    /// as an external class name without holding the source text alive.
    ///
    /// # Errors
    ///
    /// Returns an invalid requirement from a matching utility definition.
    pub(crate) fn resolve(
        raw: &str,
        theme: &ThemeIndex,
    ) -> Result<Option<ResolvedUtility>, UtilityResolutionError> {
        resolve_utility(raw, theme)
    }
}
