//! Utility tokenization that reports span failures as diagnostics.
//!
//! Tokenization cannot fail on ordinary input; only a source-coordinate problem produces an
//! error, and that error has to reach the user as a diagnostic rather than as a panic or as a
//! silently empty token list that hides the real cause. The same rule covers a token whose
//! value nests deeper than the calculator accepts: it is reported against the token's own
//! span so the value never reaches a parser that would refuse it.

use lean_string::LeanStr;

use stillwind_css::{MAX_VALUE_NESTING, value_nesting_overflow};
use stillwind_diagnostics::{CompileDiagnostic, CompileError, SourceFailureContext};
use stillwind_source::Spanned;
use stillwind_syntax::UtilityToken;

/// Tokenizes utility text, preserving each token's position relative to `input.span`.
///
/// A coordinate failure appends a fatal error at `input.span` and returns an empty
/// token list. Existing diagnostics are retained; successful tokenization adds none.
pub(super) fn lex_utilities(
    input: &Spanned<LeanStr>,
    diagnostics: &mut Vec<CompileError>,
) -> Vec<Spanned<UtilityToken>> {
    match stillwind_syntax::lex_utilities(input) {
        Ok(tokens) => tokens
            .into_iter()
            .filter(|token| !reject_excessive_nesting(token, diagnostics))
            .collect(),
        Err(error) => {
            diagnostics.push(CompileError::new(
                CompileDiagnostic::source_failure(&error, SourceFailureContext::Utility),
                input.span,
            ));
            Vec::new()
        }
    }
}

/// Appends a fatal error and reports `true` when `token` nests values deeper than the
/// calculator accepts.
///
/// The reported offset is absolute in the token's source file: the token span start plus
/// the offset within the token text. Nesting inside quotes and comments does not count.
fn reject_excessive_nesting(
    token: &Spanned<UtilityToken>,
    diagnostics: &mut Vec<CompileError>,
) -> bool {
    let Some(offset) = value_nesting_overflow(&token.value.raw) else {
        return false;
    };
    diagnostics.push(CompileError::new(
        CompileDiagnostic::CssNestingTooDeep {
            offset: token.span.start.0 as usize + offset,
            limit: MAX_VALUE_NESTING,
        },
        token.span,
    ));
    true
}
