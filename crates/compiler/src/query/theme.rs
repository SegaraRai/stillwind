//! Theme queries expose authored values and resolve concrete colors in candidate previews.

use lean_string::{LeanStr, ToLeanStr, ToLeanString};

use stillwind_css::{
    CssNode, CssNodeKind, color_rgba, find_css_functions, parse_structure,
    split_css_function_arguments,
};
use stillwind_design_system::{ThemeToken, UtilityResolutionError};

use super::{DesignSystemQuery, QueryContext};
use crate::compiler::{dynamic_candidates, stylesheet_dynamic_candidates};

impl DesignSystemQuery {
    /// Reports host plugin utility keys and `@variant ` requests found in source text.
    pub fn dynamic_candidates(&self, text: &str) -> Vec<LeanStr> {
        dynamic_candidates(text, self.context.design_system.theme())
    }

    /// Reports host plugin utility keys and `@variant ` requests that a stylesheet
    /// applies with `@apply` and `@variant`. Comments, strings, definition headers, and
    /// other directives are not read.
    pub fn stylesheet_dynamic_candidates(&self, css: &str) -> Vec<LeanStr> {
        stylesheet_dynamic_candidates(css, self.context.design_system.theme())
    }

    /// Returns active token definitions, including their output and fallback policies.
    pub fn theme_tokens(&self) -> Vec<ThemeToken> {
        self.context
            .design_system
            .theme()
            .active_tokens()
            .into_iter()
            .cloned()
            .collect()
    }

    /// Returns active theme values by their full custom-property names.
    /// Values keep their authored CSS spelling, including references to other tokens.
    pub fn theme_values(&self) -> Vec<(LeanStr, LeanStr)> {
        self.context
            .design_system
            .tokens()
            .iter()
            .flat_map(|(_, tokens)| tokens)
            .map(|token| (token.name.value.clone(), token.value.value.clone()))
            .collect()
    }

    /// Returns a concrete color for a candidate whose ordinary declarations only set
    /// colors.
    ///
    /// Mask images, mask composition, and empty pseudo-element content are ignored.
    /// Custom properties supply colors only when no ordinary declarations remain.
    /// Theme variables are resolved against this snapshot. Runtime and contextual
    /// colors are omitted. Colors must agree within 1/255 per component, except that
    /// matching RGB colors may include alpha 0.
    ///
    /// # Errors
    ///
    /// Returns malformed utility requirements.
    pub fn candidate_color(
        &self,
        candidate: &str,
    ) -> Result<Option<[f64; 4]>, UtilityResolutionError> {
        let Some(preview) = self.hover_candidate(candidate)? else {
            return Ok(None);
        };
        let mut declarations = Vec::new();
        color_declarations(
            &preview.css,
            &parse_structure(&preview.css),
            &mut declarations,
        );
        let content = self.context.options.rewrite_generated("var(--tw-content)");
        declarations.retain(|(property, value)| {
            let value = declaration_value(value);
            !property.eq_ignore_ascii_case("mask-image")
                && !property.eq_ignore_ascii_case("mask-composite")
                && !(property.eq_ignore_ascii_case("content")
                    && (matches!(value, "\"\"" | "''") || value == content))
        });
        let all_custom = declarations
            .iter()
            .all(|(property, _)| property.starts_with("--"));
        if declarations.iter().any(|(property, _)| {
            let property = property.to_ascii_lowercase();
            !property.starts_with("--")
                && !matches!(property.as_str(), "color" | "fill" | "stroke")
                && !property.ends_with("-color")
        }) {
            return Ok(None);
        }
        let values: Vec<_> = declarations
            .iter()
            .filter(|(property, _)| all_custom || !property.starts_with("--"))
            .filter_map(|(_, value)| {
                self.context
                    .resolve_theme_values(declaration_value(value))
                    .and_then(|value| color_rgba(&value))
            })
            .collect();
        let color = values
            .iter()
            .find(|color| color[3] != 0.0)
            .or(values.first())
            .copied();
        Ok(color.filter(|color| {
            values.iter().all(|other| {
                color[..3]
                    .iter()
                    .zip(other)
                    .all(|(a, b)| (a - b).abs() <= 1.0 / 255.0)
                    && (other[3] == 0.0 || (color[3] - other[3]).abs() <= 1.0 / 255.0)
            })
        }))
    }
}

/// Collects declarations from all nested rules while omitting property registrations.
/// Returned slices address `css`, and `nodes` must have been parsed from that text.
fn color_declarations<'a>(
    css: &'a str,
    nodes: &[CssNode],
    declarations: &mut Vec<(&'a str, &'a str)>,
) {
    for node in nodes {
        match &node.kind {
            CssNodeKind::Declaration { property, value } => {
                declarations.push((&css[property.clone()], &css[value.clone()]));
            }
            CssNodeKind::AtRule { name, .. } if name == "property" => {}
            _ => color_declarations(css, node.children(), declarations),
        }
    }
}

/// Returns a declaration value without surrounding whitespace and `!important`.
fn declaration_value(value: &str) -> &str {
    value.trim().trim_end_matches("!important").trim()
}

/// An active theme scale, such as `--spacing`, that canonical comparisons substitute.
#[derive(Clone, Debug)]
pub(super) struct ScaleReference {
    /// Complete reference to the scale token, such as `var(--spacing)`.
    pub(super) reference: LeanStr,
    /// Prefix of references to named tokens of the scale's namespace, such as
    /// `var(--spacing-`.
    pub(super) token_prefix: LeanStr,
    /// Authored value of the scale token, such as `0.25rem`.
    pub(super) value: LeanStr,
}

impl ScaleReference {
    /// Whether `css` refers to the scale token or to a named token of its namespace.
    pub(super) fn referenced_by(&self, css: &str) -> bool {
        css.contains(self.reference.as_str()) || css.contains(self.token_prefix.as_str())
    }
}

impl QueryContext {
    /// Returns the active scale token of each declared namespace with a scale, ordered
    /// by namespace name.
    pub(super) fn theme_scales(&self) -> Vec<ScaleReference> {
        let theme = self.design_system.theme();
        theme
            .namespaces()
            .scales()
            .filter_map(|namespace| {
                let token = theme.scale(&namespace.name)?;
                Some(ScaleReference {
                    reference: format_args!("var({})", token.name.value).to_lean_str(),
                    token_prefix: format_args!("var({}-", token.name.value).to_lean_str(),
                    value: token.value.value.clone(),
                })
            })
            .collect()
    }

    /// Substitutes known theme variables in `input`. Unknown variables retain their
    /// spelling. Returns `None` after 64 substitution passes or if an edit expands the
    /// CSS beyond 1 MiB.
    pub(super) fn resolve_theme_values(&self, input: &str) -> Option<LeanStr> {
        let mut css = input.to_lean_string();
        for _ in 0..64 {
            let mut edits = Vec::new();
            let mut covered = 0;
            for function in find_css_functions(&css, &["var"], 0) {
                if function.start < covered {
                    continue;
                }
                let (name, _) = split_css_function_arguments(&css[function.arguments]);
                if let Some(token) = self
                    .design_system
                    .tokens()
                    .get(name)
                    .and_then(|tokens| tokens.first())
                    && let Some(end) = function.end
                {
                    edits.push((function.start..end, token.value.value.clone()));
                    covered = end;
                }
            }
            if edits.is_empty() {
                return Some(css.into());
            }
            for (range, value) in edits.into_iter().rev() {
                css.replace_range(range, &value);
                if css.len() > 1_048_576 {
                    return None;
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
    use stillwind_source::FileId;
    use stillwind_test_support::css_source;

    use super::*;
    use crate::CompilerOptions;

    fn query(css: &str, options: DesignSystemOptions) -> DesignSystemQuery {
        let source = css_source(FileId(1), css).unwrap();
        DesignSystemQuery::new(
            DesignSystemSnapshot::from_source(&source, "theme.css".into(), options).unwrap(),
            CompilerOptions::default(),
        )
    }

    #[test]
    fn candidate_colors_require_color_declarations_throughout_nested_rules() {
        let query = query(
            r"@import 'stillwind';
@theme { --color-brand: red; }
@utility card { background-color: red; padding: 1rem; }
@utility nested-card { color: red; &:hover { display: block; } }
@utility nested-paint { color: red; &:hover { outline-color: red; } }
@utility masked-paint { background-color: red; mask-image: linear-gradient(red, blue); mask-composite: intersect; }
@utility empty-content { color: red; content: ''; }
@utility named-content { color: red; content: 'label'; }
@utility custom-paint { --paint: red; --size: 1rem; }
@utility mixed-paint { --paint: blue; color: red; }
@utility contextual-paint { --paint: red; color: currentColor; }
",
            DesignSystemOptions::default(),
        );
        for candidate in [
            "bg-brand",
            "hover:bg-brand",
            "md:hover:bg-brand",
            "before:bg-brand",
            "after:bg-brand!",
            "border-x-brand",
            "divide-brand",
            "outline-brand",
            "decoration-brand",
            "caret-brand",
            "accent-brand",
            "fill-brand",
            "stroke-brand",
            "ring-brand",
            "shadow-brand",
            "drop-shadow-brand",
            "from-brand",
            "nested-paint",
            "masked-paint",
            "empty-content",
            "custom-paint",
            "mixed-paint",
        ] {
            assert_eq!(
                query.candidate_color(candidate).unwrap(),
                Some([1.0, 0.0, 0.0, 1.0]),
                "{candidate}: {:?}",
                query.resolve_candidate(candidate).unwrap()
            );
        }
        for candidate in [
            "card",
            "nested-card",
            "named-content",
            "contextual-paint",
            "bg-[currentColor]",
            "p-4",
        ] {
            assert!(
                query.hover_candidate(candidate).unwrap().is_some(),
                "{candidate}"
            );
            assert_eq!(
                query.candidate_color(candidate).unwrap(),
                None,
                "{candidate}"
            );
        }
        assert_eq!(
            query.candidate_color("bg-transparent").unwrap(),
            Some([0.0; 4])
        );
    }

    #[test]
    fn candidate_colors_allow_zero_alpha_only_when_the_rgb_components_match() {
        let query = query(
            r"@utility same { color: red; outline-color: rgb(255 0 0 / 0); }
@utility faded { --from: rgb(255 0 0 / 0); --to: rgb(255 0 0 / .5); }
@utility different { color: red; outline-color: rgb(0 0 255 / 0); }
@utility different-alpha { color: red; outline-color: rgb(255 0 0 / .5); }
@utility different-colors { --from: red; --to: blue; }
",
            DesignSystemOptions::default(),
        );
        assert_eq!(
            query.candidate_color("same").unwrap(),
            Some([1.0, 0.0, 0.0, 1.0])
        );
        assert_eq!(
            query.candidate_color("faded").unwrap(),
            color_rgba("rgb(255 0 0 / .5)")
        );
        for candidate in ["different", "different-alpha", "different-colors"] {
            assert_eq!(
                query.candidate_color(candidate).unwrap(),
                None,
                "{candidate}"
            );
        }
    }

    #[test]
    fn pseudo_element_content_uses_the_configured_compatibility_prefix() {
        let query = query(
            "@import 'stillwind'; @theme { --color-brand: red; }",
            DesignSystemOptions {
                tailwind_prefix: "--compat-".into(),
                ..DesignSystemOptions::default()
            },
        );
        assert_eq!(
            query.candidate_color("before:bg-brand").unwrap(),
            Some([1.0, 0.0, 0.0, 1.0])
        );
    }
}
