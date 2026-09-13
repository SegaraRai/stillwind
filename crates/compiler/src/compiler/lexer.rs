//! Utility tokenization that reports span failures as diagnostics.
//!
//! Tokenization cannot fail on ordinary input; only a source-coordinate problem produces an
//! error, and that error has to reach the user as a diagnostic rather than as a panic or as a
//! silently empty token list that hides the real cause.

use stillwind_diagnostics::{CompileError, CompileErrorKind, SourceFailureContext};
use stillwind_source::Spanned;
use stillwind_syntax::UtilityToken;

/// Tokenizes utility text, preserving each token's position relative to `input.span`.
///
/// A coordinate failure appends a fatal error at `input.span` and returns an empty
/// token list. Existing diagnostics are retained; successful tokenization adds none.
pub(super) fn lex_utilities(
    input: &Spanned<String>,
    diagnostics: &mut Vec<CompileError>,
) -> Vec<Spanned<UtilityToken>> {
    match stillwind_syntax::lex_utilities(input) {
        Ok(tokens) => tokens,
        Err(error) => {
            diagnostics.push(CompileError::new(
                CompileErrorKind::source_failure(&error, SourceFailureContext::Utility),
                input.span,
            ));
            Vec::new()
        }
    }
}
