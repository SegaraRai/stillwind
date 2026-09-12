//! Splitting one decoded utility string into source-spanned tokens.
//!
//! Input is a [`Spanned<String>`] the host already decoded from JavaScript: the span
//! normally covers the literal contents and the value is decoded text. Tokens are
//! split on Unicode whitespace in value order. Spans add decoded byte offsets to the
//! supplied start; escapes are not mapped back to their original source spelling.
//!
//! The lexer performs no trimming, unescaping or normalization: [`UtilityToken::raw`]
//! is the exact substring, and runs of whitespace disappear instead of producing empty
//! tokens. [`Span`] arithmetic is checked, so a reversed input span or an offset that
//! overflows 32 bits fails rather than wrapping.

use stillwind_source::{SourceError, Span, Spanned};

/// One whitespace-delimited token of a utility string.
///
/// The token is not re-validated: it may be any spelling a candidate could take,
/// including one that `parse_candidate` will later reject.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtilityToken {
    /// Exact substring of the supplied value; this lexer performs no unescaping.
    pub raw: String,
}

/// Splits one decoded utility string on Unicode whitespace while retaining a source
/// span for every token.
///
/// Spans use value-relative offsets from `input.span.start`. The supplied end is not
/// used to bound tokens, so callers must provide a sufficiently long span. Exact
/// original-source correspondence additionally requires unchanged literal spelling.
///
/// # Errors
///
/// Returns an error for reversed spans or token offsets outside the 32-bit range.
pub fn lex_utilities(input: &Spanned<String>) -> Result<Vec<Spanned<UtilityToken>>, SourceError> {
    let len =
        u32::try_from(input.value.len()).map_err(|_| SourceError::TooLarge(input.value.len()))?;
    if input.span.len().is_none() || input.span.start.0.checked_add(len).is_none() {
        return Err(SourceError::InvalidSpan(input.span));
    }
    let mut tokens = Vec::new();
    let mut token_start = None;

    for (offset, character) in input.value.char_indices() {
        if character.is_whitespace() {
            if let Some(start) = token_start.take() {
                push_token(input, start, offset, &mut tokens)?;
            }
        } else if token_start.is_none() {
            token_start = Some(offset);
        }
    }

    if let Some(start) = token_start {
        push_token(input, start, input.value.len(), &mut tokens)?;
    }

    Ok(tokens)
}

/// Pushes one token for `start..end` (value-relative byte offsets) after shifting it
/// into the input literal's coordinate space.
///
/// Offsets outside the value cannot occur, because callers derive them from
/// `char_indices`; the checked conversions only guard against a value that does not fit
/// the 32-bit coordinate space.
fn push_token(
    input: &Spanned<String>,
    start: usize,
    end: usize,
    tokens: &mut Vec<Spanned<UtilityToken>>,
) -> Result<(), SourceError> {
    let start = u32::try_from(start).map_err(|_| SourceError::TooLarge(start))?;
    let end = u32::try_from(end).map_err(|_| SourceError::TooLarge(end))?;
    tokens.push(Spanned::new(
        UtilityToken {
            raw: input.value[start as usize..end as usize].to_owned(),
        },
        Span::new(
            input.span.file,
            input
                .span
                .start
                .0
                .checked_add(start)
                .ok_or(SourceError::InvalidSpan(input.span))?,
            input
                .span
                .start
                .0
                .checked_add(end)
                .ok_or(SourceError::InvalidSpan(input.span))?,
        ),
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use stillwind_source::FileId;

    use super::*;

    #[test]
    fn rejects_reversed_and_overflowing_coordinates() {
        for span in [
            Span::new(FileId(1), 2, 1),
            Span::new(FileId(1), u32::MAX, u32::MAX),
        ] {
            let input = Spanned::new("x".into(), span);
            assert!(
                matches!(lex_utilities(&input), Err(SourceError::InvalidSpan(value)) if value == span)
            );
        }
        let input = Spanned::new("x".into(), Span::new(FileId(1), u32::MAX - 1, u32::MAX));
        assert_eq!(lex_utilities(&input).unwrap()[0].span.end.0, u32::MAX);
    }

    #[test]
    fn retains_each_utilitys_exact_relative_span() {
        let input = Spanned::new(
            "px-4  text-center\nβeta".to_owned(),
            Span::new(FileId(4), 12, 36),
        );

        let tokens = lex_utilities(&input).unwrap();

        assert_eq!(
            tokens
                .iter()
                .map(|token| token.value.raw.as_str())
                .collect::<Vec<_>>(),
            ["px-4", "text-center", "βeta"],
        );
        assert_eq!(tokens[1].span, Span::new(FileId(4), 18, 29));
        assert_eq!(tokens[2].span, Span::new(FileId(4), 30, 35));
    }
}

#[cfg(test)]
mod boundary_tests {
    use stillwind_source::FileId;

    use super::*;

    proptest::proptest! {
        #[test]
        fn token_spans_slice_back_to_the_original_input(a in "[a-z]{1,12}", b in "[a-z]{1,12}") {
            let text = format!("  {a}\t{b} ");
            let input = Spanned::new(
                text.clone(),
                Span::new(FileId(1), 0, u32::try_from(text.len()).unwrap())
            );
            let tokens = lex_utilities(&input).unwrap();
            proptest::prop_assert_eq!(tokens.len(), 2);
            for token in tokens {
                proptest::prop_assert_eq!(&text[token.span.start.0 as usize..token.span.end.0 as usize], token.value.raw);
            }
        }
    }
}
