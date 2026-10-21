//! Concrete colors and editable ranges in authored CSS.

use std::ops::Range;

use cssparser::Token;
use lean_string::LeanStr;

use stillwind_css::{
    CssNode, color_rgba, find_css_functions, parse_structure, split_once_top_level,
};
use stillwind_design_system::UtilityResolutionError;

use crate::{compiler::apply_candidates, query::DesignSystemQuery};

/// A concrete color at one complete utility or theme-reference range.
#[derive(Clone, Debug)]
pub struct StylesheetColor {
    /// Half-open UTF-8 byte range in the original source. References include the entire
    /// function so replacing a color changes only this use.
    pub range: Range<usize>,
    /// sRGB red, green, blue, and effective alpha in the inclusive range 0..1.
    pub rgba: [f64; 4],
    /// Original utility spelling for candidate replacement. References have no
    /// candidate and are replaced by a CSS color literal.
    pub candidate: Option<LeanStr>,
}

impl DesignSystemQuery {
    /// Resolves colors in `@apply` candidates and complete theme-reference functions
    /// in `css`. Ranges address the original UTF-8 source, including unclosed blocks.
    /// Functions inside an `@apply` candidate are handled only through that candidate.
    /// Unknown, cyclic, contextual, and ambiguous colors are omitted. Runtime `var()`
    /// references show the active theme definition rather than browser cascade state.
    ///
    /// # Errors
    ///
    /// Returns a malformed requirement from a matching utility definition.
    pub fn stylesheet_colors(
        &self,
        css: &str,
    ) -> Result<Vec<StylesheetColor>, UtilityResolutionError> {
        let mut ranges = Vec::new();
        applications(css, &parse_structure(css), &mut ranges);
        let mut colors = Vec::new();
        for range in &ranges {
            let candidate = &css[range.clone()];
            if let Some(rgba) = self.candidate_color(candidate)? {
                colors.push(StylesheetColor {
                    range: range.clone(),
                    rgba,
                    candidate: Some(candidate.into()),
                });
            }
        }
        for call in find_css_functions(css, &["theme", "--theme", "var"], 0) {
            let Some(end) = call.end else { continue };
            let range = call.start..end;
            if ranges
                .iter()
                .any(|candidate| candidate.start < range.end && range.start < candidate.end)
                || colors
                    .iter()
                    .any(|color| color.range.start < range.end && range.start < color.range.end)
            {
                continue;
            }
            let arguments = &css[call.arguments];
            let name = split_once_top_level(arguments, |token| matches!(token, Token::Comma))
                .0
                .trim();
            let Some(token) = self.context.design_system.theme().custom_property(name) else {
                continue;
            };
            let Some(value) = self.context.resolve_theme_values(&token.value.value) else {
                continue;
            };
            if let Some(rgba) = color_rgba(&value) {
                colors.push(StylesheetColor {
                    range,
                    rgba,
                    candidate: None,
                });
            }
        }
        colors.sort_by_key(|color| color.range.start);
        Ok(colors)
    }
}

/// Appends utility ranges from nodes parsed from the same original source.
fn applications(css: &str, nodes: &[CssNode], ranges: &mut Vec<Range<usize>>) {
    for node in nodes {
        if let Some(candidates) = apply_candidates(css, node) {
            ranges.extend(candidates.iter().cloned());
        }
        applications(css, node.children(), ranges);
    }
}

#[cfg(test)]
mod tests {
    use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
    use stillwind_source::FileId;
    use stillwind_test_support::css_source;

    use crate::{CompilerOptions, DesignSystemQuery};

    #[test]
    fn colors_address_original_candidates_and_whole_reference_uses() {
        let source = css_source(FileId(1), "@theme { --color-brand:red; --color-alias:var(--color-brand); --color-context:currentColor; --color-cycle:var(--color-cycle); } @utility bg-* {background-color:--value(--color-*);} @custom-variant hover (&:hover);").unwrap();
        let query = DesignSystemQuery::new(
            DesignSystemSnapshot::from_source(
                &source,
                "theme.css".into(),
                DesignSystemOptions::default(),
            )
            .unwrap(),
            CompilerOptions::default(),
        );
        let css = "/* 日本語 🦀 */ .a { @apply hover:bg-brand bg-[var(--color-brand)] unknown-[var(--color-brand)]; color:theme(--color-alias); outline-color:var(--color-brand, blue); border-color:--theme(--color-context); background:url('theme(--color-brand)'); content:'theme(--color-brand)'; accent-color:var(--color-cycle);";
        let colors = query.stylesheet_colors(css).unwrap();
        assert_eq!(
            colors
                .iter()
                .map(|color| &css[color.range.clone()])
                .collect::<Vec<_>>(),
            [
                "hover:bg-brand",
                "bg-[var(--color-brand)]",
                "theme(--color-alias)",
                "var(--color-brand, blue)"
            ]
        );
        assert!(colors.iter().all(|color| {
            color
                .rgba
                .iter()
                .zip([1.0, 0.0, 0.0, 1.0])
                .all(|(actual, expected)| (actual - expected).abs() < f64::EPSILON)
        }));
        assert!(colors[0].candidate.is_some());
        assert!(colors[2].candidate.is_none());
    }
}
