//! Theme queries expose authored values and resolve concrete colors in candidate previews.

use stillwind_css::{css_color_values, find_css_functions, split_css_function_arguments};
use stillwind_design_system::{ThemeNamespace, UtilityResolutionError};

use super::{DesignSystemQuery, QueryContext};

impl DesignSystemQuery {
    /// Returns active theme values by their full custom-property names.
    /// Values keep their authored CSS spelling, including references to other tokens.
    pub fn theme_values(&self) -> Vec<(String, String)> {
        self.context
            .design_system
            .tokens()
            .iter()
            .flat_map(|(_, tokens)| tokens)
            .map(|token| (token.name.value.clone(), token.value.value.clone()))
            .collect()
    }

    /// Returns the scale used by spacing utilities, or `None` when it is absent.
    pub fn spacing_scale(&self) -> Option<String> {
        self.context.spacing_scale()
    }

    /// Returns a concrete candidate color when every emitted color agrees, or `None` otherwise.
    /// Theme variables are resolved against this snapshot. Runtime and contextual colors are omitted.
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
        let Some(css) = self.context.resolve_theme_values(&preview.css) else {
            return Ok(None);
        };
        let values = css_color_values(&css);
        let color = values.first().copied();
        Ok(color.filter(|color| {
            values.iter().all(|other| {
                color
                    .iter()
                    .zip(other)
                    .all(|(a, b)| (a - b).abs() <= 1.0 / 255.0)
            })
        }))
    }
}

impl QueryContext {
    /// Returns the scale used by spacing utilities, or `None` when it is absent.
    pub(super) fn spacing_scale(&self) -> Option<String> {
        self.design_system
            .theme()
            .scale(ThemeNamespace::Spacing)
            .map(|token| token.value.value.clone())
    }

    /// Substitutes known theme variables in `input`. Unknown variables retain their spelling.
    /// Returns `None` after 64 substitution passes or if an edit expands the CSS beyond 1 MiB.
    pub(super) fn resolve_theme_values(&self, input: &str) -> Option<String> {
        let mut css = input.to_owned();
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
                return Some(css);
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
