//! Renames theme variables in finished CSS for the `themePrefix` option.
//!
//! Resolution, lint, and editor queries keep authored theme variable names. Only the
//! CSS that leaves assembly carries the configured prefix, so every reference in it
//! agrees with its definition, including references in authored rules and arbitrary
//! values. A reference may also spell the prefixed name, which selects the same
//! variable.

use std::collections::BTreeSet;
use std::ops::Range;
use std::sync::Arc;

use cssparser::{ParseError, Parser, Token};
use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_design_system::{ThemeIndex, ThemeValue, stillwind_property_definitions};
use stillwind_diagnostics::CompileDiagnostic;

use crate::{CompilerOptions, MappedStylesheet};

/// Prefix of the optional variables that the package's preflight reads. The theme does
/// not need to define them.
const DEFAULT_VARIABLE_PREFIX: &str = "--default-";

/// Checks that no selected theme variable takes the name of a registered property.
///
/// `tokens` are the theme values that the output declares.
///
/// # Errors
///
/// Returns [`CompileDiagnostic::ThemePrefixCollision`] for the first such variable.
pub(super) fn check_theme_prefix<'a>(
    tokens: impl IntoIterator<Item = &'a ThemeValue>,
    theme: &ThemeIndex,
    options: &CompilerOptions,
) -> Result<(), CompileDiagnostic> {
    if options.theme_prefix.is_empty() {
        return Ok(());
    }
    let stillwind = stillwind_property_definitions(theme);
    let properties = stillwind
        .iter()
        .chain(theme.properties())
        .map(|property| options.rewrite_generated(&property.name))
        .collect::<BTreeSet<_>>();
    for token in tokens {
        let name = prefixed(&token.name.value, options);
        if properties.contains(name.as_str()) {
            return Err(CompileDiagnostic::ThemePrefixCollision {
                token: token.name.value.clone(),
                property: name.as_str().into(),
            });
        }
    }
    Ok(())
}

/// Returns `var()` references with authored names for the variables that `css` names
/// by their prefixed names.
///
/// Token selection reads authored names. Appending the result to the CSS that it
/// inspects keeps those variables in the output.
pub(super) fn prefixed_references(
    css: &str,
    theme: &ThemeIndex,
    options: &CompilerOptions,
) -> LeanString {
    let mut references = LeanString::new();
    if options.theme_prefix.is_empty() {
        return references;
    }
    let names = variable_names(theme);
    visit_identifiers(css, &mut |name, _| {
        if let Some(rest) = name.strip_prefix(options.theme_prefix.as_str()) {
            let authored = format!("--{rest}");
            if names.contains(authored.as_str()) {
                references.push_str(&format!("var({authored})"));
            }
        }
    });
    references
}

/// Renames the theme variables of finished CSS with `options.theme_prefix`.
///
/// Only whole CSS identifiers change. Strings, comments, URLs, and class names keep
/// their text. Source mappings and banners follow the retained text. The function
/// changes nothing when the prefix is empty, and it must run once for each output.
pub(crate) fn prefix_theme_variables(
    css: &mut MappedStylesheet,
    theme: &ThemeIndex,
    options: &CompilerOptions,
) {
    if options.theme_prefix.is_empty() {
        return;
    }
    let names = variable_names(theme);
    let mut edits: Vec<(Range<usize>, LeanString)> = Vec::new();
    visit_identifiers(&css.css, &mut |name, range| {
        let default = name.starts_with(DEFAULT_VARIABLE_PREFIX);
        if default || names.contains(name) {
            edits.push((range, prefixed(name, options)));
        }
    });
    css.replace_ranges(&edits);
}

/// Returns the output name of the theme variable `name`.
fn prefixed(name: &str, options: &CompilerOptions) -> LeanString {
    format_args!(
        "{}{}",
        options.theme_prefix,
        name.strip_prefix("--").unwrap_or(name)
    )
    .to_lean_string()
}

/// Returns the authored names of every theme variable, including inactive ones.
fn variable_names(theme: &ThemeIndex) -> BTreeSet<&str> {
    theme
        .values()
        .iter()
        .map(|token: &Arc<ThemeValue>| token.name.value.as_str())
        .collect()
}

/// Calls `visit` with the decoded name and byte range of each `--` identifier in `css`.
///
/// A name after a `.` delimiter is a class name and does not count.
fn visit_identifiers(css: &str, visit: &mut impl FnMut(&str, Range<usize>)) {
    visit_tokens(&mut Parser::new(css), visit);
}

/// Walks the tokens of `parser` and of its nested blocks for [`visit_identifiers`].
fn visit_tokens(parser: &mut Parser<'_>, visit: &mut impl FnMut(&str, Range<usize>)) {
    let mut class = false;
    loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            return;
        };
        let nested = matches!(
            token,
            Token::CurlyBracketBlock
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::Function(_)
        );
        let name = match token {
            Token::Ident(name) if !class && name.starts_with("--") => {
                Some(LeanStr::from(name.as_ref()))
            }
            _ => None,
        };
        class = matches!(token, Token::Delim('.'));
        if let Some(name) = name {
            visit(&name, start..parser.position().byte_index());
        }
        if nested {
            let _ = parser.parse_nested_block(|nested| {
                visit_tokens(nested, visit);
                Ok::<_, ParseError<()>>(())
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::visit_identifiers;

    /// Returns the names that [`visit_identifiers`] reports for `css`.
    fn identifiers(css: &str) -> Vec<String> {
        let mut names = Vec::new();
        visit_identifiers(css, &mut |name, range| {
            assert_eq!(&css[range], name);
            names.push(name.to_owned());
        });
        names
    }

    #[test]
    fn identifiers_skip_text_that_is_not_a_name() {
        assert_eq!(
            identifiers(
                r#".--a, .b\(--c\) { --d: var(--e, calc(var(--f) * 2)); content: "--g"; background: url(--h) /* --i */; }"#
            ),
            ["--d", "--e", "--f"]
        );
    }
}
