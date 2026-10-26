//! Compares the complete CSS of two sets of candidates, such as a group before and
//! after a replacement.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use lean_string::LeanStr;

use stillwind_css::{
    CssMinifyOptions, CssRule, Declaration, minify_css, property_covers, property_value_domains,
    repeated_longhands,
};
use stillwind_design_system::UtilityResolutionError;

use super::super::candidate::{authored_variables, variable_calls};
use super::super::preview::{CanonicalPreview, normalized_signature, spacing_properties};
use crate::query::DesignSystemQuery;
use crate::query::theme::ScaleReference;

/// The normalized CSS of a set of candidates and the facts that a replacement must
/// keep.
pub(super) struct CanonicalSet {
    signature: String,
    required: BTreeSet<LeanStr>,
    written_variables: Vec<lean_string::LeanString>,
    references: BTreeSet<lean_string::LeanString>,
}

impl CanonicalSet {
    /// Checks whether `other` can replace this set.
    ///
    /// The two sets must produce the same normalized CSS and need the same registered
    /// properties. Each `var()` call that the author wrote in this set must also appear
    /// in the CSS of `other`, so that a replacement does not remove an explicit
    /// variable. Both sets must come from the same query.
    pub(super) fn matches(&self, other: &Self) -> bool {
        self.signature == other.signature
            && self.required == other.required
            && self
                .written_variables
                .iter()
                .all(|variable| other.references.contains(variable))
    }
}

impl DesignSystemQuery {
    /// Returns the `CanonicalSet` of the candidates `values`.
    ///
    /// The method sorts the candidates in the order of the compiled CSS before it
    /// merges their CSS. Returns `None` when a candidate does not resolve, or when its
    /// CSS cannot be normalized.
    ///
    /// # Errors
    ///
    /// Returns malformed requirements of a candidate.
    pub(super) fn canonical_set(
        &self,
        values: &[LeanStr],
    ) -> Result<Option<CanonicalSet>, UtilityResolutionError> {
        let scales = self.context.theme_scales();
        let mut css = String::new();
        let mut raw_css = String::new();
        let mut required = BTreeSet::new();
        let mut written_variables = Vec::new();
        let ranks =
            self.candidate_ranks(&values.iter().map(LeanStr::as_str).collect::<Vec<_>>())?;
        let mut order: Vec<_> = (0..values.len()).collect();
        order.sort_by_key(|index| ranks[*index]);
        for index in order {
            let value = &values[index];
            let Some(preview) = self.context.canonical_preview(value)? else {
                return Ok(None);
            };
            let Some(normalized) = self.set_candidate_css(&preview, &scales) else {
                return Ok(None);
            };
            css.push_str(&normalized);
            raw_css.push_str(&preview.css);
            required.extend(preview.required_properties.iter().cloned());
            written_variables.extend(authored_variables(value));
        }
        let Some(signature) = self.normalize_set_css(&css, &scales) else {
            return Ok(None);
        };
        let signature = ordered_css(&signature, 0);
        let composition = self.context.composition();
        required.retain(|name| {
            let rendered = self.context.options.rewrite_generated(name);
            !composition.shadows.contains_key(&rendered) || signature.contains(rendered.as_str())
        });
        Ok(Some(CanonicalSet {
            signature,
            required,
            written_variables,
            references: variable_calls(&raw_css).into_iter().collect(),
        }))
    }

    /// Returns the CSS of one candidate for `normalize_set_css`. Returns `None` when
    /// the CSS of the candidate cannot be resolved.
    ///
    /// When the `logicalToPhysical` option is enabled, the method returns the resolved
    /// CSS, because `normalize_set_css` converts property names later. Otherwise it
    /// returns the cached normalized CSS of the candidate. `scales` must come from this
    /// query.
    fn set_candidate_css(
        &self,
        preview: &CanonicalPreview,
        scales: &[ScaleReference],
    ) -> Option<LeanStr> {
        let css = preview.resolved_css.as_deref()?;
        if self.canonical_options().logical_to_physical {
            return preview.resolved_css.clone();
        }
        let mut properties = BTreeSet::new();
        if self.canonical_options().px_to_rem {
            spacing_properties(css, scales, &mut properties, 0);
        }
        preview.signature(scales, &properties, self.context.options.root_font_size)
    }

    /// Returns the merged CSS `css` of several candidates in normalized form. `css`
    /// must contain the results of `set_candidate_css` in the order of the compiled
    /// CSS.
    ///
    /// When the `logicalToPhysical` option is enabled, the method first replaces
    /// logical property names with physical names, and then normalizes the result.
    /// Otherwise the input is already normalized per candidate, and the method only
    /// minifies it. `scales` must come from this query. Returns `None` when the CSS is
    /// invalid.
    fn normalize_set_css(&self, css: &str, scales: &[ScaleReference]) -> Option<LeanStr> {
        if !self.canonical_options().logical_to_physical {
            return minify_css(css, &CssMinifyOptions::default())
                .ok()
                .map(|output| output.css.into());
        }
        let css = physical_css(css, 0);
        let mut properties = BTreeSet::new();
        if self.canonical_options().px_to_rem {
            spacing_properties(&css, scales, &mut properties, 0);
        }
        normalized_signature(
            &css,
            scales,
            &properties,
            self.context.options.root_font_size,
            self.canonical_options().preserve_specificity,
            &self.context.composition(),
        )
    }
}

/// Returns `property` with logical sides and logical sizes replaced by physical names,
/// such as `margin-left` for `margin-inline-start` and `width` for `inline-size`. The
/// method assumes horizontal, left-to-right writing. `property` must not be a custom
/// property. A logical shorthand such as `margin-inline` keeps its name.
pub(super) fn physical(property: &str) -> String {
    property
        .replace("-inline-start", "-left")
        .replace("-inline-end", "-right")
        .replace("-block-start", "-top")
        .replace("-block-end", "-bottom")
        .replace("inline-size", "width")
        .replace("block-size", "height")
}

/// Returns the physical properties that `property` sets in horizontal, left-to-right
/// writing. A logical shorthand returns both sides, such as `margin-left` and
/// `margin-right` for `margin-inline`. A custom property is returned unchanged.
pub(super) fn physical_properties(property: &str) -> Vec<String> {
    if property.starts_with("--") {
        return vec![property.into()];
    }
    let property = physical(property);
    if property.contains("-inline") {
        return vec![
            property.replace("-inline", "-left"),
            property.replace("-inline", "-right"),
        ];
    }
    if property.contains("-block") {
        return vec![
            property.replace("-block", "-top"),
            property.replace("-block", "-bottom"),
        ];
    }
    vec![property]
}

/// Returns `css` with each declaration block reduced to the final value of each
/// property, sorted by property name. After this step, two blocks that set the same
/// values compare equal even when they write the declarations in a different order.
///
/// A shorthand is expanded to the longhands that it sets. A shorthand with one
/// variable, such as `border-style: var(--tw-border-style)`, is expanded with
/// `repeated_longhands`. An `!important` value wins over later values without
/// `!important`. `depth` is the nesting level of `css`. CSS that cannot be parsed, and
/// rules nested deeper than 64 levels, stay as written.
fn ordered_css(css: &str, depth: usize) -> String {
    if depth > 64 {
        return css.into();
    }
    if let Some(mut declarations) = Declaration::parse(css) {
        let mut values = BTreeMap::new();
        for declaration in &mut declarations {
            let Some((_, value)) = declaration.css.split_once(':') else {
                continue;
            };
            let important = value.trim().ends_with("!important");
            let value = value
                .trim()
                .strip_suffix("!important")
                .unwrap_or(value)
                .trim();
            let mut domains = if declaration.property.starts_with("--") {
                vec![(declaration.property.as_str().into(), value.into())]
            } else {
                property_value_domains(&declaration.property, value)
            };
            if domains.len() == 1 {
                domains.extend(
                    repeated_longhands(&declaration.property, value)
                        .iter()
                        .map(|property| ((*property).into(), value.into())),
                );
            }
            for (property, value) in &domains {
                if domains
                    .iter()
                    .any(|(other, _)| property_covers(property, other))
                {
                    continue;
                }
                if values
                    .get(property)
                    .is_none_or(|(_, previous)| !previous || important)
                {
                    values.insert(property.clone(), (value.clone(), important));
                }
            }
        }
        return values.into_iter().fold(
            String::new(),
            |mut output, (property, (value, important))| {
                let _ = write!(
                    output,
                    "{property}:{value}{};",
                    if important { "!important" } else { "" }
                );
                output
            },
        );
    }
    CssRule::parse(css).map_or_else(
        |_| css.into(),
        |rules| {
            rules
                .into_iter()
                .map(|rule| {
                    rule.body.as_deref().map_or_else(
                        || rule.prelude.to_string(),
                        |body| format!("{}{{{}}}", rule.prelude, ordered_css(body, depth + 1)),
                    )
                })
                .collect()
        },
    )
}

/// Returns `css` with each property name replaced by the names that
/// `physical_properties` returns. A declaration of a logical shorthand becomes one
/// declaration for each physical side. `depth` is the nesting level of `css`. CSS that
/// cannot be parsed, and rules nested deeper than 64 levels, stay as written.
fn physical_css(css: &str, depth: usize) -> String {
    if depth > 64 {
        return css.into();
    }
    if let Some(declarations) = Declaration::parse(css) {
        return declarations
            .into_iter()
            .map(|declaration| {
                let Some((_, value)) = declaration.css.split_once(':') else {
                    return declaration.css.to_string();
                };
                physical_properties(&declaration.property).iter().fold(
                    String::new(),
                    |mut output, property| {
                        let _ = write!(output, "{property}:{value};");
                        output
                    },
                )
            })
            .collect();
    }
    CssRule::parse(css).map_or_else(
        |_| css.into(),
        |rules| {
            rules
                .into_iter()
                .map(|rule| {
                    rule.body.as_deref().map_or_else(
                        || rule.prelude.to_string(),
                        |body| format!("{}{{{}}}", rule.prelude, physical_css(body, depth + 1)),
                    )
                })
                .collect()
        },
    )
}
