//! Looks for named spellings of a candidate, such as `w-4` for `w-[1rem]`, and checks
//! that each one produces the same CSS.

use std::collections::BTreeSet;

use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::{CssRule, Declaration, fold_calc_expressions, properties_interfere};
use stillwind_design_system::{
    UtilityResolutionError, declared_properties, named_domain_values, resolve_utility,
};
use stillwind_syntax::{CandidatePrefix, Utility, parse_candidate};

use super::super::index::rank;
use super::super::preview::{CanonicalPreview, SpacingUnit, dimension};
use super::spelling::{arbitrary, value_candidate};
use crate::query::{DesignSystemQuery, QueryContext};

impl DesignSystemQuery {
    /// Returns a named utility that produces the same CSS as the complete candidate
    /// `raw`, including its modifier, or `None` when there is none.
    ///
    /// The method looks up the CSS of `raw` and its declaration values in the canonical
    /// index. It accepts only names in the families that `alias_roots` returns. A named
    /// candidate that uses a theme token other than a scale token keeps its name.
    ///
    /// # Errors
    ///
    /// Returns malformed requirements of `raw` or of a name that the method checks.
    pub(super) fn named_alias(&self, raw: &str) -> Result<Option<LeanStr>, UtilityResolutionError> {
        let Some(parsed) = parse_candidate(raw) else {
            return Ok(None);
        };
        let roots = self.alias_roots(&parsed.utility);
        if roots.is_empty() {
            return Ok(None);
        }
        if matches!(&parsed.utility, Utility::Named { name, .. } if !name.contains(['[', '(']))
            && !parsed.negative
            && let Ok(resolved) = resolve_utility(raw, self.context.design_system.theme())?
        {
            let scales = self.context.theme_scales();
            if resolved
                .diagnostic_data
                .token_uses
                .iter()
                .flat_map(|usage| &usage.tokens)
                .any(|token| {
                    !scales
                        .iter()
                        .any(|scale| scale.reference == format!("var({token})"))
                })
            {
                return Ok(None);
            }
        }
        let split = CandidatePrefix::variant_end(raw);
        let base = raw[split..].trim_matches('!');
        let Some(preview) = self.context.canonical_preview(base)? else {
            return Ok(None);
        };
        let index = self.canonical_index();
        let mut names = index.matching(&self.context, &preview);
        if let Some(css) = &preview.resolved_css {
            for value in declaration_values(css, 0) {
                names.extend(index.value_labels(
                    &self.context,
                    &roots,
                    &fold_calc_expressions(&value, None),
                ));
            }
        }
        names.sort_by(|left, right| rank(left, right));
        names.dedup();
        for name in names {
            let body = name.trim_start_matches('-');
            if let Some(root) = body.match_indices('-').rev().find_map(|(index, _)| {
                self.context
                    .design_system
                    .utilities()
                    .get(&format!("{}-*", &body[..index]))
                    .map(|_| &body[..index])
            }) && !roots.iter().any(|expected| expected == root)
            {
                continue;
            }
            if !roots
                .iter()
                .any(|root| body.starts_with(&format!("{root}-")) || body == root)
            {
                continue;
            }
            let replacement = format!(
                "{}{name}{}",
                &raw[..split],
                if parsed.important { "!" } else { "" }
            )
            .to_lean_str();
            if replacement == raw {
                return Ok(None);
            }
            if !raw.contains('[')
                && replacement.len() > raw.len()
                && !resolve_utility(&replacement, self.context.design_system.theme())?
                    .is_ok_and(|utility| !utility.diagnostic_data.token_uses.is_empty())
            {
                continue;
            }
            if self.equivalent(raw, &replacement)? {
                return Ok(Some(replacement));
            }
        }
        Ok(None)
    }

    /// Returns the roots of the families whose names may replace `utility`.
    ///
    /// A named utility without an arbitrary value, or a named utility with a modifier,
    /// returns the root of its own family. When several families match, the family with
    /// the longest root is used. An arbitrary property with a modifier returns the
    /// roots of the families that declare its property. Any other utility returns no
    /// roots.
    fn alias_roots(&self, utility: &Utility) -> BTreeSet<LeanStr> {
        match utility {
            Utility::Named { name, modifier }
                if !name.contains(['[', '(']) || modifier.is_some() =>
            {
                name.match_indices('-')
                    .rev()
                    .find_map(|(index, _)| {
                        self.context
                            .design_system
                            .utilities()
                            .get(&format!("{}-*", &name[..index]))
                            .map(|_| name[..index].to_lean_str())
                    })
                    .into_iter()
                    .collect()
            }
            Utility::ArbitraryProperty {
                property,
                modifier: Some(_),
                ..
            } => self.canonical_roots("", Some(property)),
            _ => BTreeSet::new(),
        }
    }

    /// Returns a replacement for the value or the variants of the candidate `raw`, or
    /// `None` when there is none.
    ///
    /// For a candidate with an arbitrary value, the method tries the labels from
    /// `canonical_labels` in order. It skips a label that would make a candidate
    /// without brackets longer. When no label fits, or when `raw` has no arbitrary
    /// value, the method tries `canonical_variant`. A candidate that cannot be parsed
    /// has no replacement.
    ///
    /// # Errors
    ///
    /// Returns malformed requirements of `raw` or of a replacement that the method
    /// checks.
    pub(super) fn find_canonical(
        &self,
        raw: &str,
    ) -> Result<Option<LeanStr>, UtilityResolutionError> {
        let Some(parsed) = parse_candidate(raw) else {
            return Ok(None);
        };
        let Some(original) = self.context.canonical_preview(raw)? else {
            return self.canonical_variant(raw);
        };
        if matches!(&parsed.utility, Utility::Named { name, .. } if !name.contains(['[', '('])) {
            return self.canonical_variant(raw);
        }
        let split = CandidatePrefix::variant_end(raw);
        let prefix = &raw[..split];
        let utility = raw[split..].trim_matches('!').trim_start_matches('-');
        let suffix = CandidatePrefix::parse(utility)
            .and_then(|prefix| prefix.last_separator('/'))
            .map_or("", |index| &utility[index..]);
        let (root, value, property) = match &parsed.utility {
            Utility::Named { name, .. } => {
                if let Some((root, _, value)) = arbitrary(name) {
                    (
                        root.to_lean_str(),
                        fold_calc_expressions(&value, None).into(),
                        None,
                    )
                } else if let Some(index) = name
                    .match_indices('-')
                    .rev()
                    .map(|(index, _)| index)
                    .find(|index| {
                        self.context
                            .design_system
                            .utilities()
                            .get(&format!("{}-*", &name[..*index]))
                            .is_some()
                    })
                {
                    (name[..index].to_lean_str(), name[index + 1..].into(), None)
                } else {
                    return self.canonical_variant(raw);
                }
            }
            Utility::ArbitraryProperty {
                property, value, ..
            } => (LeanStr::new(), value.to_lean_str(), Some(property.as_str())),
        };
        let expanded = self.expanded_value(&value);
        let bare_value = expanded.as_str();
        let roots = self.canonical_roots(&root, property);
        let base_raw = format_args!(
            "{}{}",
            if parsed.negative { "-" } else { "" },
            match &parsed.utility {
                Utility::Named { name, .. } => name.to_lean_str(),
                Utility::ArbitraryProperty {
                    property, value, ..
                } => format_args!("[{property}:{value}]").to_lean_str(),
            }
        )
        .to_lean_str();
        let base = if base_raw == raw {
            Some(original.clone())
        } else {
            self.context.canonical_preview(&base_raw)?
        };
        let search = CanonicalSearch {
            roots: &roots,
            value: bare_value,
            written: &value,
            property,
            negative: parsed.negative,
            modifier: suffix,
        };
        let labels = self.canonical_labels(&search, &original, base.as_deref());
        for label in labels {
            let replacement = format_args!(
                "{prefix}{}{label}{suffix}{}",
                if parsed.negative { "-" } else { "" },
                if parsed.important { "!" } else { "" }
            )
            .to_lean_str();
            if replacement == raw || (!raw.contains('[') && replacement.len() >= raw.len()) {
                continue;
            }
            if self.equivalent(raw, &replacement)? {
                return Ok(Some(replacement));
            }
        }
        self.canonical_variant(raw)
    }

    /// Returns the labels that may replace the candidate of `search`, in the order to
    /// try them.
    ///
    /// Names from the canonical index come first, then labels without brackets, and
    /// then the other labels. Labels with the same priority are sorted by `rank`.
    /// `original` is the preview of the candidate. `base` is the preview of the same
    /// utility without variants, `!`, or modifier. It is `None` when that utility does
    /// not resolve.
    fn canonical_labels(
        &self,
        search: &CanonicalSearch<'_>,
        original: &CanonicalPreview,
        base: Option<&CanonicalPreview>,
    ) -> Vec<LeanStr> {
        let CanonicalSearch {
            roots,
            value: bare_value,
            property,
            negative,
            modifier,
            ..
        } = *search;
        let mut labels = self.numeric_labels(search);
        let index = self.canonical_index();
        let mut named: BTreeSet<_> = index
            .value_labels(&self.context, roots, bare_value)
            .into_iter()
            .collect();
        labels.extend(named.iter().cloned());
        if let Some(base) = base {
            for record in index.matching(&self.context, base) {
                if let Some(label) =
                    eligible_label(&record, roots, property, negative, &self.context)
                {
                    named.insert(label.to_lean_str());
                    labels.insert(label.to_lean_str());
                }
            }
        }
        if !index.errors.is_empty() {
            let properties = declared_properties(&original.css);
            for (label, error) in &index.errors {
                if let Some(label) = eligible_label(label, roots, property, negative, &self.context)
                    && self
                        .context
                        .design_system
                        .utility_search()
                        .definitions_with_prefix(&error.name)
                        .any(|(name, entries)| {
                            name == error.name
                                && entries
                                    .iter()
                                    .any(|entry| !entry.properties.is_disjoint(&properties))
                        })
                {
                    labels.insert(label.to_lean_str());
                }
            }
        }
        if !modifier.is_empty() {
            // Modifiers can erase value differences or select a branch absent from the base output.
            let preserves_value = index.modifier_preserves_value(roots, modifier);
            let properties = declared_properties(&original.css);
            let contextual: Vec<_> = index
                .context_labels(roots, negative, preserves_value)
                .iter()
                .filter(|label| self.contextual_properties_match(label, &properties))
                .filter_map(|label| {
                    eligible_label(label, roots, property, negative, &self.context)
                        .map(LeanStr::from)
                })
                .collect();
            named.extend(contextual.iter().cloned());
            labels.extend(contextual);
        }
        let mut labels: Vec<_> = labels.into_iter().collect();
        labels.sort_by(|left, right| {
            (!named.contains(left), left.contains('['))
                .cmp(&(!named.contains(right), right.contains('[')))
                .then_with(|| rank(left, right))
        });
        labels
    }

    /// Checks whether the utility `label` may declare one of `properties`.
    ///
    /// The method returns true without a check when `label` has no definition, when a
    /// definition uses `@apply`, when a definition declares no properties, or when
    /// `properties` is empty. In these cases, the declared properties are unknown.
    fn contextual_properties_match(&self, label: &str, properties: &BTreeSet<LeanStr>) -> bool {
        let Some(definitions) = self.context.design_system.utilities().get(label) else {
            return true;
        };
        // Applied utilities can add properties that the definition does not declare.
        if properties.is_empty()
            || definitions
                .iter()
                .any(|definition| definition.body.contains("@apply"))
        {
            return true;
        }
        self.context
            .design_system
            .utility_search()
            .definitions_with_prefix(label)
            .find(|(name, _)| *name == label)
            .is_none_or(|(_, entries)| {
                entries.iter().any(|entry| {
                    entry.properties.is_empty()
                        || entry.properties.iter().any(|property| {
                            properties
                                .iter()
                                .any(|source| properties_interfere(property, source))
                        })
                })
            })
    }

    /// Returns labels that write the value of `search` as a number, for each root of
    /// `search`.
    ///
    /// The labels include the value itself, such as `w-12`, and `root-0` for zero. A
    /// negative value also gives a label with a leading `-`. When `search` has a
    /// property, the arbitrary value is included. A length also gives multiples of each
    /// scale step, such as `p-4` for `1rem` with a `0.25rem` step. A multiple must be a
    /// multiple of 0.25 and at most 384. When the `pxToRem` option is enabled, the
    /// method converts between `px` and `rem` with the root font size.
    fn numeric_labels(&self, search: &CanonicalSearch<'_>) -> BTreeSet<LeanStr> {
        let CanonicalSearch {
            roots,
            value: bare_value,
            written,
            property,
            ..
        } = *search;
        let options = self.canonical_options();
        let scales = self.context.theme_scales();
        let mut labels = BTreeSet::new();
        for root in roots {
            if !bare_value
                .strip_suffix('%')
                .is_some_and(|value| value.parse::<f64>().is_ok_and(|value| value.fract() != 0.0))
            {
                labels.insert(format_args!("{root}-{bare_value}").to_lean_str());
            }
            if bare_value.parse::<f64>().is_ok_and(|value| value == 0.0) {
                labels.insert(format!("{root}-0").into());
            }
            if property.is_some() {
                labels.insert(value_candidate(root, "", written).into());
            }
            if let Some(positive) = bare_value.strip_prefix('-') {
                labels.insert(format!("-{root}-{positive}").into());
            }
        }
        if let Some((number, unit)) = dimension(bare_value) {
            for root in roots {
                labels.insert(format_args!("{root}-{number}").to_lean_str());
            }
            for (step, scale_unit) in scales.iter().filter_map(|scale| dimension(&scale.value)) {
                let amount = if unit == scale_unit {
                    Some(number)
                } else if unit == SpacingUnit::Px
                    && scale_unit == SpacingUnit::Rem
                    && options.px_to_rem
                {
                    Some(number / self.context.options.root_font_size.get())
                } else if unit == SpacingUnit::Rem
                    && scale_unit == SpacingUnit::Px
                    && options.px_to_rem
                {
                    Some(number * self.context.options.root_font_size.get())
                } else {
                    None
                };
                if let Some(amount) = amount.filter(|_| step > 0.0) {
                    for root in roots {
                        let multiplier = amount / step;
                        if multiplier.abs() <= 384.0 && (multiplier * 4.0).fract() == 0.0 {
                            labels.insert(
                                format_args!(
                                    "{}{root}-{}",
                                    if multiplier < 0.0 { "-" } else { "" },
                                    multiplier.abs()
                                )
                                .to_lean_str(),
                            );
                        }
                    }
                }
            }
        }
        labels
    }

    /// Returns `root` alone when it is not empty. Otherwise returns the roots of the
    /// functional families that declare `property`. Returns no roots when `root` is
    /// empty and `property` is `None`.
    fn canonical_roots(&self, root: &str, property: Option<&str>) -> BTreeSet<LeanStr> {
        if !root.is_empty() {
            return BTreeSet::from([root.to_lean_str()]);
        }
        property
            .into_iter()
            .flat_map(|property| {
                self.context
                    .design_system
                    .utility_search()
                    .names_for_property(property)
            })
            .filter_map(|name| name.strip_suffix("-*").map(LeanStr::from))
            .collect()
    }
}

/// Returns the values of the declarations in `css` and in its nested rules, without the
/// final `;`. `depth` is the nesting level of `css`. CSS that cannot be parsed, and
/// rules nested deeper than 64 levels, add no values.
fn declaration_values(css: &str, depth: usize) -> Vec<String> {
    if depth > 64 {
        return Vec::new();
    }
    if let Some(declarations) = Declaration::parse(css) {
        return declarations
            .iter()
            .filter_map(|declaration| {
                declaration
                    .css
                    .split_once(':')
                    .map(|(_, value)| value.trim().trim_end_matches(';').trim().to_string())
            })
            .collect();
    }
    CssRule::parse(css).map_or_else(
        |_| Vec::new(),
        |rules| {
            rules
                .iter()
                .filter_map(|rule| rule.body.as_deref())
                .flat_map(|body| declaration_values(body, depth + 1))
                .collect()
        },
    )
}

/// The facts about one candidate that `find_canonical` uses to look up labels.
struct CanonicalSearch<'a> {
    /// Roots of the functional families whose labels may replace the candidate.
    roots: &'a BTreeSet<LeanStr>,
    /// The arbitrary value after theme functions and theme variables are replaced and
    /// `calc()` is folded.
    value: &'a str,
    /// The arbitrary value as the author wrote it, after decoding. A replacement with
    /// an arbitrary value uses this text.
    written: &'a str,
    /// The property of an arbitrary property candidate, such as `width` in
    /// `[width:1px]`. It is `None` for other candidates.
    property: Option<&'a str>,
    /// Whether the candidate starts with `-`.
    negative: bool,
    /// The modifier of the candidate, including its leading `/`. It is empty when the
    /// candidate has no modifier.
    modifier: &'a str,
}

/// Returns `label` when it can replace a candidate whose family roots are `roots`.
/// Returns `None` otherwise. `query` provides the registries.
///
/// When `negative` is true, `label` must start with `-`, and the result omits that `-`.
/// When `negative` is false, a `label` that starts with `-` is accepted only when
/// `property` is set. When `property` is `None`, a label from a registered family that
/// is not in `roots` is rejected.
///
/// The label is accepted when it is registered, or when it names a theme token or a
/// named value of a family in `roots`. When `property` is set, a label from a family
/// that declares `property` is also accepted.
fn eligible_label<'a>(
    label: &'a str,
    roots: &BTreeSet<LeanStr>,
    property: Option<&str>,
    negative: bool,
    query: &QueryContext,
) -> Option<&'a str> {
    let body = label.trim_start_matches('-');
    if property.is_none()
        && let Some(root) = body.match_indices('-').rev().find_map(|(index, _)| {
            query
                .design_system
                .utilities()
                .get(&format!("{}-*", &body[..index]))
                .map(|_| &body[..index])
        })
        && !roots.contains(root)
    {
        return None;
    }
    let registered = label;
    let label = if negative {
        label.strip_prefix('-')?
    } else {
        if property.is_none() && label.starts_with('-') {
            return None;
        }
        label
    };
    let eligible = roots.iter().any(|root| {
        let Some(capture) = label.strip_prefix(format_args!("{root}-").to_lean_str().as_str())
        else {
            return false;
        };
        if query.design_system.utilities().get(registered).is_some() {
            return true;
        }
        let family = format_args!("{}{root}-*", if negative { "-" } else { "" }).to_lean_str();
        query
            .design_system
            .utility_search()
            .definitions_with_prefix(&family)
            .any(|(name, entries)| {
                name == family
                    && entries.iter().any(|entry| {
                        entry.value_namespaces.iter().any(|namespace| {
                            query
                                .design_system
                                .tokens()
                                .get(&format_args!("{namespace}{capture}").to_lean_str())
                                .is_some()
                        })
                    })
            })
            || query
                .design_system
                .utilities()
                .get(&family)
                .is_some_and(|definitions| {
                    definitions.iter().any(|definition| {
                        named_domain_values(&definition.body, query.design_system.theme())
                            .iter()
                            .any(|name| name == capture)
                    })
                })
    }) || property.is_some_and(|property| {
        query
            .design_system
            .utility_search()
            .names_for_property(property)
            .any(|name| name == registered)
    });
    eligible.then_some(label)
}
