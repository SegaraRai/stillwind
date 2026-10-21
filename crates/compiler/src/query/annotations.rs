//! Adds comments with resolved values to formatted preview CSS.
//!
//! Editors show these comments next to theme references, `calc()` expressions, rem
//! dimensions, and color functions. The comments describe values for readers; they do
//! not change the CSS. Selectors and at-rules other than `@media` and `@container`
//! receive no comments.
//!
//! Numbers in comments use their shortest decimal form with a leading zero. Values
//! that `calc()` division makes inexact are rounded to six fractional digits, and `≈`
//! precedes each rounded number instead of `=`.
//!
//! A `calc()` that evaluates to one value receives one comment after the expression,
//! and the references inside it receive none. When the expression combines rem with
//! px, the comment contains the pixel total. Other expressions receive comments on
//! the theme references and rem values inside them.

use std::ops::Range;

use cssparser::{ParseError, Parser, Token};
use lean_string::{LeanString, ToLeanString};

use stillwind_css::{color_hex, evaluate_numeric, split_once_top_level};

use super::{DesignSystemQuery, QueryContext};

/// Media and container queries compare rem and em against the initial font size of
/// the browser, which ignores the root element's `font-size`.
const MEDIA_ROOT_FONT_SIZE: f64 = 16.0;

/// Nesting depth after which blocks and functions receive no further comments.
const MAXIMUM_DEPTH: usize = 64;

/// Color functions whose literal arguments receive a hexadecimal equivalent.
const COLOR_FUNCTIONS: [&str; 10] = [
    "rgb", "rgba", "hsl", "hsla", "hwb", "lab", "lch", "oklab", "oklch", "color",
];

/// Selects the comments that [`DesignSystemQuery::annotate_preview`] adds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreviewAnnotations {
    /// Adds the resolved value after each theme `var()` and after each `calc()` that
    /// evaluates to one value.
    pub theme_values: bool,
    /// Adds pixel equivalents for rem values.
    pub pixel_equivalents: bool,
    /// Adds hexadecimal sRGB equivalents for colors. `None` adds no color comments.
    pub color_equivalents: Option<ColorEquivalents>,
}

impl Default for PreviewAnnotations {
    fn default() -> Self {
        Self {
            theme_values: true,
            pixel_equivalents: true,
            color_equivalents: Some(ColorEquivalents::Approximate),
        }
    }
}

/// Handling of colors outside the sRGB gamut.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorEquivalents {
    /// Shows the gamut-mapped color after `≈`.
    Approximate,
    /// Shows no hexadecimal value for colors outside the gamut.
    Exact,
}

impl DesignSystemQuery {
    /// Adds comments with resolved values to `css`, which must be a complete stylesheet
    /// such as the output of `format_css`.
    ///
    /// Theme references resolve against this query's snapshot, and rem values use the
    /// configured root font size, except in `@media` and `@container` preludes, where
    /// they use 16 pixels per rem. Each comment follows the value it describes. Text
    /// outside the inserted comments is unchanged, and `css` that cannot be tokenized
    /// is returned without comments where tokenization stops.
    #[must_use]
    pub fn annotate_preview(&self, css: &str, annotations: PreviewAnnotations) -> LeanString {
        let annotator = Annotator {
            query: &self.context,
            annotations,
            root_font_size: self.context.options.root_font_size.get(),
        };
        let mut comments = Vec::new();
        annotator.block(&mut Parser::new(css), css, 0, &mut comments);
        let mut output = LeanString::with_capacity(css.len());
        let mut cursor = 0;
        for (offset, text) in comments {
            if text.contains("*/") || offset < cursor {
                continue;
            }
            output.push_str(&css[cursor..offset]);
            output.push_str(" /* ");
            output.push_str(&text);
            output.push_str(" */");
            cursor = offset;
        }
        output.push_str(&css[cursor..]);
        output
    }
}

/// One comment, inserted before the byte at its offset.
type Comment = (usize, LeanString);

struct Annotator<'a> {
    query: &'a QueryContext,
    annotations: PreviewAnnotations,
    root_font_size: f64,
}

impl Annotator<'_> {
    /// Collects comments for the rules and declarations in one block, in source order.
    fn block(&self, parser: &mut Parser<'_>, css: &str, depth: usize, comments: &mut Vec<Comment>) {
        if depth > MAXIMUM_DEPTH {
            return;
        }
        while let Ok(token) = parser.next() {
            match token.clone() {
                Token::AtKeyword(name) => {
                    let start = parser.position().byte_index();
                    let (end, nested) = self.rest_of_item(parser, css, depth, true);
                    if self.annotations.pixel_equivalents
                        && (name.eq_ignore_ascii_case("media")
                            || name.eq_ignore_ascii_case("container"))
                    {
                        let mut prelude = Vec::new();
                        media_dimensions(
                            &mut Parser::new(&css[start..end]),
                            &css[start..end],
                            0,
                            &mut prelude,
                        );
                        comments.extend(
                            prelude
                                .into_iter()
                                .map(|(offset, text)| (start + offset, text)),
                        );
                    }
                    comments.extend(nested.unwrap_or_default());
                }
                Token::Ident(name) => {
                    let custom = name.starts_with("--");
                    let state = parser.state();
                    let declaration = matches!(parser.next(), Ok(Token::Colon));
                    if !declaration {
                        parser.reset(&state);
                    }
                    let start = parser.position().byte_index();
                    // A custom property value can contain a block, while any other
                    // item that reaches a block is a nested rule.
                    let (end, nested) =
                        self.rest_of_item(parser, css, depth, !(declaration && custom));
                    match nested {
                        Some(nested) => {
                            comments.extend(nested);
                        }
                        None if declaration => {
                            let value = &css[start..end];
                            let mut found = Vec::new();
                            self.value(&mut Parser::new(value), value, 0, &mut found);
                            comments.extend(
                                found
                                    .into_iter()
                                    .map(|(offset, text)| (start + offset, text)),
                            );
                        }
                        None => {}
                    }
                }
                Token::CurlyBracketBlock => {
                    let _ = parser.parse_nested_block(|nested| {
                        self.block(nested, css, depth + 1, comments);
                        Ok::<_, ParseError<()>>(())
                    });
                }
                _ => {}
            }
        }
    }

    /// Consumes the rest of a statement or of a rule prelude. Returns the offset where
    /// the consumed text ends, before any `;` or block. When `blocks` is true, a block
    /// ends the item and its comments are returned.
    fn rest_of_item(
        &self,
        parser: &mut Parser<'_>,
        css: &str,
        depth: usize,
        blocks: bool,
    ) -> (usize, Option<Vec<Comment>>) {
        loop {
            let end = parser.position().byte_index();
            match parser.next_including_whitespace_and_comments() {
                Ok(Token::CurlyBracketBlock) if blocks => {
                    let mut nested = Vec::new();
                    let _ = parser.parse_nested_block(|parser| {
                        self.block(parser, css, depth + 1, &mut nested);
                        Ok::<_, ParseError<()>>(())
                    });
                    return (end, Some(nested));
                }
                Ok(Token::Semicolon) | Err(_) => {
                    return (end, None);
                }
                // The parser skips an unread block only when it reads the next token,
                // so the block is consumed here to keep the end offset after it.
                Ok(
                    Token::Function(_)
                    | Token::ParenthesisBlock
                    | Token::SquareBracketBlock
                    | Token::CurlyBracketBlock,
                ) => {
                    let _ = parser.parse_nested_block(|nested| {
                        while nested.next_including_whitespace_and_comments().is_ok() {}
                        Ok::<_, ParseError<()>>(())
                    });
                }
                Ok(_) => {}
            }
        }
    }

    /// Collects comments for one declaration value. Offsets are relative to `value`.
    fn value(
        &self,
        parser: &mut Parser<'_>,
        value: &str,
        depth: usize,
        comments: &mut Vec<Comment>,
    ) {
        if depth > MAXIMUM_DEPTH {
            return;
        }
        loop {
            let start = parser.position().byte_index();
            let Ok(token) = parser.next_including_whitespace_and_comments() else {
                return;
            };
            match token.clone() {
                Token::Dimension { unit, .. }
                    if self.annotations.pixel_equivalents && unit.eq_ignore_ascii_case("rem") =>
                {
                    let end = parser.position().byte_index();
                    if let Some(number) = number_before_suffix(&value[start..end], unit.len()) {
                        let pixels = quantity(number * self.root_font_size, "px");
                        comments.push((end, pixels.standalone()));
                    }
                }
                Token::Function(name) => {
                    let name = name.to_ascii_lowercase();
                    let mut nested = Vec::new();
                    let _ = parser.parse_nested_block(|parser| {
                        self.value(parser, value, depth + 1, &mut nested);
                        Ok::<_, ParseError<()>>(())
                    });
                    let end = parser.position().byte_index();
                    let text = &value[start..end];
                    let comment = match name.as_str() {
                        "var" if self.annotations.theme_values => self.variable(text),
                        "calc" if self.annotations.theme_values => self.calculation(text),
                        name if COLOR_FUNCTIONS.contains(&name) => self.color_literal(text),
                        _ => None,
                    };
                    match comment {
                        Some(comment) => {
                            comments.push((end, comment));
                        }
                        None => {
                            comments.extend(nested);
                        }
                    }
                }
                Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock => {
                    let _ = parser.parse_nested_block(|parser| {
                        self.value(parser, value, depth + 1, comments);
                        Ok::<_, ParseError<()>>(())
                    });
                }
                _ => {}
            }
        }
    }

    /// Describes a theme `var()` reference, or returns `None` when its name is not a
    /// theme token.
    fn variable(&self, text: &str) -> Option<LeanString> {
        let arguments = text.get(text.find('(')? + 1..text.len().checked_sub(1)?)?;
        let (name, _) = split_once_top_level(arguments, |token| matches!(token, Token::Comma));
        let token = self
            .query
            .design_system
            .tokens()
            .get(name.trim())
            .and_then(|tokens| tokens.first())?;
        let resolved = self.query.resolve_theme_values(&token.value.value)?;
        let resolved = resolved.trim();
        if resolved.is_empty() {
            return None;
        }
        let mut description = normalize_numbers(resolved);
        if let Some((number, unit)) = evaluate_numeric(resolved) {
            let value = quantity(number, &unit);
            if value.text != description {
                join(&mut description, value.approximate, &value.text);
            }
            self.join_pixels(&mut description, number, &unit);
        } else {
            self.join_equivalent(&mut description, resolved);
        }
        Some(description)
    }

    /// Describes a `calc()` expression that evaluates to one number after theme
    /// references are substituted. Returns `None` for other expressions.
    ///
    /// An expression that combines rem with px is evaluated after rem values are
    /// converted to pixels, and only the pixel result is described. Pixel equivalents
    /// must be enabled for this conversion.
    fn calculation(&self, text: &str) -> Option<LeanString> {
        let inlined = self.query.resolve_theme_values(text)?;
        if let Some((number, unit)) = evaluate_numeric(&inlined) {
            let mut description = quantity(number, &unit).standalone();
            self.join_pixels(&mut description, number, &unit);
            return Some(description);
        }
        if !self.annotations.pixel_equivalents {
            return None;
        }
        // Full precision keeps the rounding of the final result as the only rounding.
        let pixels = replace_rems(&inlined, |number| {
            format_args!("{}px", number * self.root_font_size).to_lean_string()
        })?;
        let (number, unit) = evaluate_numeric(&pixels)?;
        unit.eq_ignore_ascii_case("px")
            .then(|| quantity(number, "px").standalone())
    }

    /// Returns the hexadecimal equivalent of a literal color function.
    fn color_literal(&self, text: &str) -> Option<LeanString> {
        let policy = self.annotations.color_equivalents?;
        let color = color_hex(text)?;
        match (color.in_gamut, policy) {
            (true, _) => Some(color.hex),
            (false, ColorEquivalents::Approximate) => {
                Some(format_args!("≈ {}", color.hex).to_lean_string())
            }
            (false, ColorEquivalents::Exact) => None,
        }
    }

    /// Appends the pixel equivalent of a rem `number` to `description`.
    fn join_pixels(&self, description: &mut LeanString, number: f64, unit: &str) {
        if self.annotations.pixel_equivalents && unit.eq_ignore_ascii_case("rem") {
            let pixels = quantity(number * self.root_font_size, "px");
            join(description, pixels.approximate, &pixels.text);
        }
    }

    /// Appends the pixel or color equivalent of a value that is not one number.
    fn join_equivalent(&self, description: &mut LeanString, value: &str) {
        if self.annotations.pixel_equivalents
            && let Some(pixels) = rem_to_pixels(value, self.root_font_size)
        {
            join(description, pixels.approximate, &pixels.text);
            return;
        }
        if let Some(policy) = self.annotations.color_equivalents
            && let Some(color) = color_hex(value)
            && !color.hex.eq_ignore_ascii_case(value)
            && (color.in_gamut || policy == ColorEquivalents::Approximate)
        {
            join(description, !color.in_gamut, &color.hex);
        }
    }
}

/// Collects pixel comments for rem and em dimensions in a media or container prelude.
/// Offsets are relative to `prelude`.
fn media_dimensions(
    parser: &mut Parser<'_>,
    prelude: &str,
    depth: usize,
    comments: &mut Vec<Comment>,
) {
    if depth > MAXIMUM_DEPTH {
        return;
    }
    loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            return;
        };
        match token.clone() {
            Token::Dimension { unit, .. }
                if unit.eq_ignore_ascii_case("rem") || unit.eq_ignore_ascii_case("em") =>
            {
                let end = parser.position().byte_index();
                if let Some(number) = number_before_suffix(&prelude[start..end], unit.len()) {
                    let pixels = quantity(number * MEDIA_ROOT_FONT_SIZE, "px");
                    comments.push((end, pixels.standalone()));
                }
            }
            Token::Function(_) | Token::ParenthesisBlock | Token::SquareBracketBlock => {
                let _ = parser.parse_nested_block(|nested| {
                    media_dimensions(nested, prelude, depth + 1, comments);
                    Ok::<_, ParseError<()>>(())
                });
            }
            _ => {}
        }
    }
}

/// A number, percentage, or dimension token found by [`numeric_tokens`].
struct NumericToken {
    /// Byte range of the token in the scanned value.
    range: Range<usize>,
    /// Byte length of the `%` or unit suffix; zero for a number.
    suffix: usize,
    /// True for a dimension whose unit is `rem`, compared case-insensitively.
    rem: bool,
}

/// Collects the numeric tokens of a value, including those inside functions and blocks.
fn numeric_tokens(parser: &mut Parser<'_>, depth: usize, tokens: &mut Vec<NumericToken>) {
    if depth > MAXIMUM_DEPTH {
        return;
    }

    loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            return;
        };
        let suffix = match token.clone() {
            Token::Number { .. } => Some((0, false)),
            Token::Percentage { .. } => Some((1, false)),
            Token::Dimension { unit, .. } => Some((unit.len(), unit.eq_ignore_ascii_case("rem"))),
            Token::Function(_) | Token::ParenthesisBlock | Token::SquareBracketBlock => {
                let _ = parser.parse_nested_block(|nested| {
                    numeric_tokens(nested, depth + 1, tokens);
                    Ok::<_, ParseError<()>>(())
                });
                None
            }
            _ => None,
        };
        if let Some((suffix, rem)) = suffix {
            tokens.push(NumericToken {
                range: start..parser.position().byte_index(),
                suffix,
                rem,
            });
        }
    }
}

/// Rewrites each number in `value` in its shortest decimal form with a leading zero,
/// such as `0.25rem` for `.25rem` and `1.5rem` for `1.50rem`. Other text is unchanged.
fn normalize_numbers(value: &str) -> LeanString {
    let mut tokens = Vec::new();
    numeric_tokens(&mut Parser::new(value), 0, &mut tokens);
    let mut result = value.to_lean_string();
    for token in tokens.into_iter().rev() {
        let text = &value[token.range.clone()];
        if let Some(number) = number_before_suffix(text, token.suffix) {
            // Adding positive zero turns a negative zero into positive zero.
            let normalized = format_args!("{}{}", number + 0.0, &text[text.len() - token.suffix..])
                .to_lean_string();
            result.replace_range(token.range, &normalized);
        }
    }
    result
}

/// Replaces each rem dimension in `value` with pixels. Returns `None` when `value`
/// contains no rem dimension.
fn rem_to_pixels(value: &str, root_font_size: f64) -> Option<Quantity> {
    let mut approximate = false;
    let text = replace_rems(value, |number| {
        let pixels = quantity(number * root_font_size, "px");
        approximate |= pixels.approximate;
        pixels.text
    })?;
    Some(Quantity { text, approximate })
}

/// Replaces each rem dimension in `value` with the text that `replacement` returns
/// for its number. Returns `None` when `value` contains no rem dimension or a rem
/// number cannot be read.
fn replace_rems(value: &str, mut replacement: impl FnMut(f64) -> LeanString) -> Option<LeanString> {
    let mut tokens = Vec::new();
    numeric_tokens(&mut Parser::new(value), 0, &mut tokens);
    let mut text = value.to_lean_string();
    let mut replaced = false;
    for token in tokens.into_iter().rev().filter(|token| token.rem) {
        let number = number_before_suffix(&value[token.range.clone()], token.suffix)?;
        text.replace_range(token.range, &replacement(number));
        replaced = true;
    }
    replaced.then_some(text)
}

/// Parses the number of a numeric token whose `%` or unit suffix has `suffix` bytes.
fn number_before_suffix(token: &str, suffix: usize) -> Option<f64> {
    token
        .get(..token.len().checked_sub(suffix)?)?
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
}

/// Text that describes a number after rounding for display.
struct Quantity {
    /// The number with at most six fractional digits, followed by its unit.
    text: LeanString,
    /// True when rounding changed the number.
    approximate: bool,
}

impl Quantity {
    /// Returns the text alone, preceded by `≈` when it is approximate.
    fn standalone(self) -> LeanString {
        if self.approximate {
            format_args!("≈ {}", self.text).to_lean_string()
        } else {
            self.text
        }
    }
}

/// Formats `number` and `unit` with at most six fractional digits, a leading zero, and
/// no negative zero.
fn quantity(number: f64, unit: &str) -> Quantity {
    // Adding positive zero turns a negative zero into positive zero.
    let rounded = (number * 1_000_000.0).round() / 1_000_000.0 + 0.0;
    Quantity {
        text: format_args!("{rounded}{unit}").to_lean_string(),
        // The tolerance ignores floating-point errors, such as the error of 0.1 + 0.2.
        approximate: (rounded - number).abs() > 1e-12 * number.abs().max(1.0),
    }
}

/// Appends `part` to `description`, separated by `≈` when `approximate` is true and by
/// `=` otherwise.
fn join(description: &mut LeanString, approximate: bool, part: &str) {
    description.push_str(if approximate { " ≈ " } else { " = " });
    description.push_str(part);
}

#[cfg(test)]
mod tests {
    use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
    use stillwind_source::{CssSourceName, FileId};
    use stillwind_test_support::css_source;

    use super::*;
    use crate::{CompilerOptions, RootFontSize};

    const THEME: &str = "@theme { --*: initial; --spacing: .25rem; --color-brand: oklch(63.7% .237 25.331); --color-wide: oklch(72.3% 0.219 149.579); --color-alias: var(--color-brand); --color-black: #000; --text-sm--line-height: calc(1.25 / 0.875); --font-sans: ui-sans-serif, system-ui; --radius-lg: 1.50rem; }";

    const PREVIEW: &str = "& {\n  padding: calc(var(--spacing) * 4);\n  margin: calc(var(--spacing) + 1px);\n  color: var(--color-brand);\n  background-color: var(--color-wide);\n  border-color: var(--color-alias);\n  caret-color: var(--color-black);\n  line-height: var(--tw-leading, var(--text-sm--line-height));\n  font-family: var(--font-sans);\n  outline-color: oklch(72.3% 0.219 149.579);\n  accent-color: rgb(255 0 0);\n  inset: 1rem;\n  border-radius: var(--radius-lg);\n  gap: calc(var(--spacing) / 3);\n  margin-top: -.5rem;\n  translate: var(--tw-translate-x) 0;\n\n  &:hover {\n    width: 2rem;\n  }\n}\n\n@media (width >= 40rem) {\n  & {\n    display: flex;\n  }\n}\n";

    fn query(options: CompilerOptions) -> DesignSystemQuery {
        let source = css_source(FileId(1), THEME).unwrap();
        let snapshot = DesignSystemSnapshot::from_source(
            &source,
            CssSourceName::from("theme.css"),
            DesignSystemOptions::default(),
        )
        .unwrap();
        DesignSystemQuery::new(snapshot, options)
    }

    #[test]
    fn default_annotations_describe_theme_values_pixels_and_colors() {
        assert_eq!(
            query(CompilerOptions::default())
                .annotate_preview(PREVIEW, PreviewAnnotations::default()),
            "& {\n  padding: calc(var(--spacing) * 4) /* 1rem = 16px */;\n  margin: calc(var(--spacing) + 1px) /* 5px */;\n  color: var(--color-brand) /* oklch(63.7% 0.237 25.331) = #fb2c36 */;\n  background-color: var(--color-wide) /* oklch(72.3% 0.219 149.579) ≈ #00c758 */;\n  border-color: var(--color-alias) /* oklch(63.7% 0.237 25.331) = #fb2c36 */;\n  caret-color: var(--color-black) /* #000 = #000000 */;\n  line-height: var(--tw-leading, var(--text-sm--line-height) /* calc(1.25 / 0.875) ≈ 1.428571 */);\n  font-family: var(--font-sans) /* ui-sans-serif, system-ui */;\n  outline-color: oklch(72.3% 0.219 149.579) /* ≈ #00c758 */;\n  accent-color: rgb(255 0 0) /* #ff0000 */;\n  inset: 1rem /* 16px */;\n  border-radius: var(--radius-lg) /* 1.5rem = 24px */;\n  gap: calc(var(--spacing) / 3) /* ≈ 0.083333rem ≈ 1.333333px */;\n  margin-top: -.5rem /* -8px */;\n  translate: var(--tw-translate-x) 0;\n\n  &:hover {\n    width: 2rem /* 32px */;\n  }\n}\n\n@media (width >= 40rem /* 640px */) {\n  & {\n    display: flex;\n  }\n}\n"
        );
    }

    #[test]
    fn disabled_annotations_keep_the_css() {
        let annotations = PreviewAnnotations {
            theme_values: false,
            pixel_equivalents: false,
            color_equivalents: None,
        };
        assert_eq!(
            query(CompilerOptions::default()).annotate_preview(PREVIEW, annotations),
            PREVIEW
        );
    }

    #[test]
    fn exact_colors_skip_values_outside_the_srgb_gamut() {
        let annotations = PreviewAnnotations {
            color_equivalents: Some(ColorEquivalents::Exact),
            ..PreviewAnnotations::default()
        };
        let css = query(CompilerOptions::default()).annotate_preview(PREVIEW, annotations);
        assert!(
            css.contains("background-color: var(--color-wide) /* oklch(72.3% 0.219 149.579) */;")
        );
        assert!(css.contains("outline-color: oklch(72.3% 0.219 149.579);"));
        assert!(
            css.contains("color: var(--color-brand) /* oklch(63.7% 0.237 25.331) = #fb2c36 */;")
        );
    }

    #[test]
    fn root_font_size_applies_to_values_but_not_media_queries() {
        let css = query(CompilerOptions {
            root_font_size: RootFontSize::new(20.0).unwrap(),
            ..CompilerOptions::default()
        })
        .annotate_preview(PREVIEW, PreviewAnnotations::default());
        assert!(css.contains("padding: calc(var(--spacing) * 4) /* 1rem = 20px */;"));
        assert!(css.contains("inset: 1rem /* 20px */;"));
        assert!(css.contains("@media (width >= 40rem /* 640px */)"));
    }

    #[test]
    fn pixel_equivalents_remain_when_theme_values_are_disabled() {
        let annotations = PreviewAnnotations {
            theme_values: false,
            ..PreviewAnnotations::default()
        };
        let css = query(CompilerOptions::default()).annotate_preview(PREVIEW, annotations);
        assert!(css.contains("padding: calc(var(--spacing) * 4);"));
        assert!(css.contains("color: var(--color-brand);"));
        assert!(css.contains("inset: 1rem /* 16px */;"));
        assert!(css.contains("accent-color: rgb(255 0 0) /* #ff0000 */;"));
    }

    #[test]
    fn calc_that_cannot_become_one_pixel_value_describes_its_parts() {
        let css = "& {\n  margin: calc(var(--spacing) + 1px);\n  width: calc(100% - var(--spacing));\n  height: calc(1em + var(--spacing));\n}\n";
        assert_eq!(
            query(CompilerOptions::default()).annotate_preview(css, PreviewAnnotations::default()),
            "& {\n  margin: calc(var(--spacing) + 1px) /* 5px */;\n  width: calc(100% - var(--spacing) /* 0.25rem = 4px */);\n  height: calc(1em + var(--spacing) /* 0.25rem = 4px */);\n}\n"
        );
        let annotations = PreviewAnnotations {
            pixel_equivalents: false,
            ..PreviewAnnotations::default()
        };
        assert!(
            query(CompilerOptions::default())
                .annotate_preview(css, annotations)
                .contains("margin: calc(var(--spacing) /* 0.25rem */ + 1px);")
        );
    }

    #[test]
    fn custom_property_blocks_and_container_names_are_scanned_as_values() {
        let css = query(CompilerOptions::default()).annotate_preview(
            "@container card (width >= 20rem) {\n  & {\n    --data: {a: 1rem};\n    --gap: 1rem;\n  }\n}\n",
            PreviewAnnotations::default(),
        );
        assert_eq!(
            css,
            "@container card (width >= 20rem /* 320px */) {\n  & {\n    --data: {a: 1rem /* 16px */};\n    --gap: 1rem /* 16px */;\n  }\n}\n"
        );
    }
}
