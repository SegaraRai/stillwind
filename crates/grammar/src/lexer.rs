//! Stillwind's small, source-spanned utility-string lexer.

use stillwind_source::{Span, Spanned};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtilityToken {
    pub raw: String,
}

/// Splits one decoded utility string on Unicode whitespace while retaining a
/// source span for every token.
pub fn lex_utilities(input: &Spanned<String>) -> Vec<Spanned<UtilityToken>> {
    let mut tokens = Vec::new();
    let mut token_start = None;

    for (offset, character) in input.value.char_indices() {
        if character.is_whitespace() {
            if let Some(start) = token_start.take() {
                push_token(input, start, offset, &mut tokens);
            }
        } else if token_start.is_none() {
            token_start = Some(offset);
        }
    }

    if let Some(start) = token_start {
        push_token(input, start, input.value.len(), &mut tokens);
    }

    tokens
}

fn push_token(
    input: &Spanned<String>,
    start: usize,
    end: usize,
    tokens: &mut Vec<Spanned<UtilityToken>>,
) {
    let start = u32::try_from(start).expect("utility strings must be smaller than 4 GiB");
    let end = u32::try_from(end).expect("utility strings must be smaller than 4 GiB");
    tokens.push(Spanned::new(
        UtilityToken {
            raw: input.value[start as usize..end as usize].to_owned(),
        },
        Span::new(
            input.span.file,
            input.span.start.0 + start,
            input.span.start.0 + end,
        ),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use stillwind_source::FileId;

    #[test]
    fn retains_each_utilitys_exact_relative_span() {
        let input = Spanned::new(
            "px-4  text-center\nβeta".to_owned(),
            Span::new(FileId(4), 12, 36),
        );

        let tokens = lex_utilities(&input);

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
    use super::*;

    proptest::proptest! {
        #[test]
        fn token_spans_slice_back_to_the_original_input(a in "[a-z]{1,12}", b in "[a-z]{1,12}") {
            let text = format!("  {a}\t{b} ");
            let input = Spanned::new(
                text.clone(),
                Span::new(stillwind_source::FileId(1), 0, u32::try_from(text.len()).unwrap())
            );
            let tokens = lex_utilities(&input);
            proptest::prop_assert_eq!(tokens.len(), 2);
            for token in tokens {
                proptest::prop_assert_eq!(&text[token.span.start.0 as usize..token.span.end.0 as usize], token.value.raw);
            }
        }
    }
}
