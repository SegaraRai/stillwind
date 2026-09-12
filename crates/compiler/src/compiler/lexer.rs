//! Utility tokenization that reports span failures as diagnostics.
//!
//! Tokenization cannot fail on ordinary input; only a source-coordinate problem produces an
//! error, and that error has to reach the user as a diagnostic rather than as a panic or as a
//! silently empty token list that hides the real cause.

use stillwind_source::{CompileError, Spanned};
use stillwind_syntax::UtilityToken;

/// Attach source-coordinate failures to the compilation's diagnostic stream.
pub(super) fn lex_utilities(
    input: &Spanned<String>,
    diagnostics: &mut Vec<CompileError>,
) -> Vec<Spanned<UtilityToken>> {
    match stillwind_syntax::lex_utilities(input) {
        Ok(tokens) => tokens,
        Err(error) => {
            diagnostics.push(CompileError::new(
                "SHSPAN001",
                error.to_string(),
                input.span,
            ));
            Vec::new()
        }
    }
}
