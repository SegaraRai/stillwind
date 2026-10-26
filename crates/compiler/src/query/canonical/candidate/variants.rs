//! Suggests shorter spellings for variants, such as `data-selected` for
//! `data-[selected]` and `nth-2` for `[&:nth-child(2)]`.
//!
//! The suggestions come from the variant definitions of the snapshot. The value type of
//! a functional variant decides how a bracket value becomes a named value, and the
//! templates of the canonical index turn arbitrary selectors back into variants.

use lean_string::{LeanStr, ToLeanStr};

use stillwind_design_system::{StillwindVariantValueType, UtilityResolutionError};
use stillwind_syntax::{decode_arbitrary_value, parse_candidate, print_arbitrary_value};

use super::super::variants::{
    aria_true_name, probe_declaration, simple_name, test_css, written_test_css,
};
use crate::query::DesignSystemQuery;

impl DesignSystemQuery {
    /// Returns `raw` with one of its variants replaced by a spelling that produces the
    /// same CSS, or `None` when there is no such spelling.
    ///
    /// The method tries the spellings from `variant_spellings` and the variants that
    /// the canonical index finds. A spelling from `variant_spellings` may also repair
    /// `raw` when `raw` does not resolve and the replacement does. A candidate that
    /// cannot be parsed has no replacement.
    ///
    /// # Errors
    ///
    /// Returns malformed requirements of `raw` or of a replacement.
    pub(super) fn canonical_variant(
        &self,
        raw: &str,
    ) -> Result<Option<LeanStr>, UtilityResolutionError> {
        let Some(candidate) = parse_candidate(raw) else {
            return Ok(None);
        };
        for variant in &candidate.variants {
            let mut names: Vec<LeanStr> = self
                .variant_spellings(&variant.raw, 0)
                .into_iter()
                .map(Into::into)
                .collect();
            let recoverable = names.clone();
            let index = &self.canonical_index().variants;
            names.extend(index.matching(&self.context, &variant.raw));
            for (prefix, suffix, child) in self.compound_captures(&variant.raw) {
                names.extend(
                    index
                        .matching(&self.context, child)
                        .into_iter()
                        .map(|name| format!("{prefix}{name}{suffix}").into()),
                );
            }
            for name in names {
                let replacement = raw.replacen(
                    format_args!("{}:", variant.raw).to_lean_str().as_str(),
                    &format_args!("{name}:").to_lean_str(),
                    1,
                );
                if replacement != raw
                    && (self.equivalent(raw, &replacement)?
                        || (recoverable.contains(&name)
                            && self.context.canonical_preview(raw)?.is_none()
                            && self.context.canonical_preview(&replacement)?.is_some()))
                {
                    return Ok(Some(replacement.into()));
                }
            }
        }
        Ok(None)
    }

    /// Returns the variant prefix, the variant suffix, and the captured child variant
    /// for each functional variant of the variant type that captures `raw`. For
    /// example, `group-hover` gives `group-`, an empty suffix, and `hover`.
    fn compound_captures<'a>(&'a self, raw: &'a str) -> Vec<(&'a str, &'a str, &'a str)> {
        self.context
            .design_system
            .variants()
            .iter()
            .flat_map(|(_, definitions)| definitions)
            .filter(|definition| definition.value_type == Some(StillwindVariantValueType::Variant))
            .filter_map(|definition| {
                let (prefix, suffix) = definition.wildcard_parts()?;
                Some((prefix, suffix, definition.capture(raw)?))
            })
            .collect()
    }

    /// Returns other spellings of the variant `raw`.
    ///
    /// For a functional variant of the variant type, such as `group-`, the method keeps
    /// the prefix and spells the nested variant again. For other value types, a bracket
    /// value that is a plain name becomes a named value. For the `[aria]` value type, a
    /// value that tests `="true"` becomes the bare name. An arbitrary selector variant
    /// is spelled by `selector_spellings`. `depth` is the nesting level of `raw`, and
    /// the method returns nothing after 64 levels.
    fn variant_spellings(&self, raw: &str, depth: usize) -> Vec<String> {
        if depth > 64 {
            return Vec::new();
        }
        let mut output = Vec::new();
        for definition in self
            .context
            .design_system
            .variants()
            .iter()
            .flat_map(|(_, definitions)| definitions)
        {
            let Some((prefix, suffix)) = definition.wildcard_parts() else {
                continue;
            };
            let Some(capture) = definition.capture(raw) else {
                continue;
            };
            if definition.value_type == Some(StillwindVariantValueType::Variant) {
                for child in self.variant_spellings(capture, depth + 1) {
                    output.push(format!("{prefix}{child}{suffix}"));
                }
                continue;
            }
            let Some(value) = capture
                .strip_prefix('[')
                .and_then(|value| value.strip_suffix(']'))
            else {
                continue;
            };
            let value = decode_arbitrary_value(value);
            if definition.value_type == Some(StillwindVariantValueType::Aria) {
                if let Some(name) = aria_true_name(&value) {
                    output.push(format!("{prefix}{name}{suffix}"));
                }
            } else if simple_name(&value) {
                output.push(format!("{prefix}{value}{suffix}"));
            }
        }
        if raw.starts_with('[') && raw.ends_with(']') {
            output.extend(self.selector_spellings(raw, depth + 1));
        }
        output
    }

    /// Returns variant spellings for the arbitrary selector variant `raw`, such as
    /// `in-[p]` for `[p_&]`, `*:data-x` for `[&>[data-x]]`, and `focus` for
    /// `[&:focus]`.
    ///
    /// The method compares the test CSS of `raw` with the named variants and with the
    /// templates of the canonical index. `depth` is the nesting level of `raw`, and the
    /// method returns nothing after 64 levels.
    fn selector_spellings(&self, raw: &str, depth: usize) -> Vec<String> {
        if depth > 64 {
            return Vec::new();
        }
        // Invalid input has no CSS. Its text can still give a valid spelling, which
        // `canonical_variant` accepts as a repair.
        let Some(css) = test_css(&self.context, raw).or_else(|| written_test_css(raw)) else {
            return Vec::new();
        };
        let index = &self.canonical_index().variants;
        let mut output: Vec<String> = index
            .named(&self.context, raw)
            .into_iter()
            .map(String::from)
            .collect();
        for template in &index.templates {
            output
                .extend(template.spellings(&css, |child| self.variant_spellings(child, depth + 1)));
        }
        let rule = format!("{{{}}}", probe_declaration());
        for relation in &index.relations {
            let Some(related) = css
                .strip_prefix(relation.before.as_str())
                .and_then(|rest| rest.strip_suffix(rule.as_str()))
                .filter(|related| !related.is_empty() && *related != "*")
            else {
                continue;
            };
            let child = format!("[&{}]", print_arbitrary_value(related));
            let names = self.variant_spellings(&child, depth + 1);
            if names.is_empty() {
                output.push(format!(
                    "{}:[{}]",
                    relation.name,
                    print_arbitrary_value(related)
                ));
            }
            for name in names {
                output.push(format!("{}:{name}", relation.name));
            }
        }
        output
    }
}
