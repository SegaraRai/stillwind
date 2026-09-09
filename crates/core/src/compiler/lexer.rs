use stillwind_grammar::UtilityToken;
use stillwind_source::{CompileError, Spanned};

/// Attach source-coordinate failures to the compilation's diagnostic stream.
pub(super) fn lex_utilities(
    input: &Spanned<String>,
    diagnostics: &mut Vec<CompileError>,
) -> Vec<Spanned<UtilityToken>> {
    match stillwind_grammar::lex_utilities(input) {
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
