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
