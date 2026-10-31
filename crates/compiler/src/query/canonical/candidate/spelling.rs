//! Suggests replacements that change only the spelling of a candidate: its value, its
//! type hint, its modifier, or a legacy utility name.

use std::collections::BTreeSet;

use cssparser::Token;
use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::{
    ValueDomain, fold_calc_expressions, matching_css_delimiter, split_once_top_level,
};
use stillwind_design_system::{UtilityResolutionError, placeholder_namespaces, resolve_utility};
use stillwind_syntax::{
    CandidatePrefix, Utility, Value, decode_arbitrary_value, parse_candidate, print_arbitrary_value,
};

use super::super::preview::{SpacingUnit, dimension};
use crate::query::DesignSystemQuery;

impl DesignSystemQuery {
    /// Returns `value` with theme functions and theme variables replaced by their
    /// values, and with `calc()` expressions folded. When a theme variable cannot be
    /// resolved, the method folds `value` as written.
    pub(super) fn expanded_value(&self, value: &LeanStr) -> LeanString {
        let lowered = self
            .context
            .design_system
            .theme()
            .resolve_css_functions(value)
            .unwrap_or_else(|| value.to_lean_string());
        let expanded = self
            .context
            .resolve_theme_values(&lowered)
            .unwrap_or_else(|| value.clone());
        fold_calc_expressions(&expanded, None)
    }

    /// Checks whether removing the type hint of `left` gives `right`, and whether the
    /// family of `left` accepts that hint. For example, `bg-[color:red]` and `bg-[red]`
    /// qualify when the `bg-*` family accepts `color` values.
    ///
    /// The caller uses this check to repair input whose hint makes it invalid. The
    /// caller must ensure that `left` and `right` have the same variants, `!`, `-`, and
    /// modifier.
    ///
    /// # Errors
    ///
    /// Returns malformed requirements of the family.
    pub(super) fn recover_hint(
        &self,
        left: &str,
        right: &str,
    ) -> Result<bool, UtilityResolutionError> {
        let Some(left) = parse_candidate(left) else {
            return Ok(false);
        };
        let Some(right) = parse_candidate(right) else {
            return Ok(false);
        };
        let (Utility::Named { name: left, .. }, Utility::Named { name: right, .. }) =
            (&left.utility, &right.utility)
        else {
            return Ok(false);
        };
        let (Some((root, Some(hint), value)), Some((target_root, None, target_value))) =
            (arbitrary(left), arbitrary(right))
        else {
            return Ok(false);
        };
        if root != target_root || value != target_value || ValueDomain::from_hint(hint).is_none() {
            return Ok(false);
        }
        let probe = value_candidate(root, &format!("{hint}:"), "var(--sw-canonical-hint-probe)");
        Ok(resolve_utility(&probe, self.context.design_system.theme())?.is_ok())
    }

    /// Returns the modifiers to try with each replacement, each with its leading `/`.
    /// `written` is the modifier as the author wrote it, or an empty string when there
    /// is no modifier.
    ///
    /// When `modifier` is `None`, the only result is `written`. Otherwise the first
    /// result is the empty string, which removes the modifier, and the second result is
    /// `written`. An arbitrary modifier also adds:
    ///
    /// - the keys of the `--modifier` namespaces of each family of `utility`,
    /// - multiples of each scale step when the modifier is a length,
    /// - the arbitrary modifier in compact spelling,
    /// - a percentage or a fraction written as a number, such as `/50` for `50%` or
    ///   `0.5`.
    fn modifier_spellings(
        &self,
        utility: &Utility,
        modifier: Option<&Value>,
        written: &str,
    ) -> Vec<String> {
        let mut output = vec![written.to_string()];
        let Some(modifier) = modifier else {
            return output;
        };
        output.insert(0, String::new());
        let Value::Arbitrary(value) = modifier else {
            return output;
        };
        let expanded = self.expanded_value(value);
        if let Utility::Named { name, .. } = utility {
            for (index, _) in name.match_indices('-') {
                if let Some(definitions) = self
                    .context
                    .design_system
                    .utilities()
                    .get(&format!("{}-*", &name[..index]))
                {
                    for definition in definitions {
                        for prefix in placeholder_namespaces(&definition.body, "--modifier") {
                            for token in self
                                .context
                                .design_system
                                .tokens()
                                .names_with_prefix(&prefix)
                            {
                                if let Some(key) = token.strip_prefix(prefix.as_str()) {
                                    output.push(format!("/{key}"));
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Some((number, unit)) = dimension(&expanded) {
            for scale in self.context.theme_scales() {
                if let Some((step, scale_unit)) = dimension(&scale.value) {
                    let factor = if unit == scale_unit {
                        Some(number / step)
                    } else if self.canonical_options().px_to_rem {
                        let font = self.context.options.root_font_size.get();
                        Some(if unit == SpacingUnit::Px {
                            number / font / step
                        } else {
                            number * font / step
                        })
                    } else {
                        None
                    };
                    if let Some(factor) =
                        factor.filter(|value| value.is_finite() && (value * 4.0).fract() == 0.0)
                    {
                        output.push(format!("/{factor}"));
                    }
                }
            }
        }
        output.push(format!("/[{}]", print_arbitrary_value(value)));
        let value = value.trim();
        if let Some(number) = value
            .strip_suffix('%')
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| (value * 4.0).fract() == 0.0)
        {
            output.push(format!("/{number}"));
        }
        if let Ok(number) = value.parse::<f64>()
            && number.is_finite()
        {
            output.push(format!("/{}", number * 100.0));
        }
        output
    }

    /// Returns other spellings of the named arbitrary utility `name`, without variants
    /// and modifier. `raw` is the complete candidate. Returns `None` when `name` has no
    /// arbitrary value.
    ///
    /// When `is_negative` is true, the method moves the sign into the value, such as
    /// `mt-[-1rem]` for `-mt-[1rem]`. A variable value keeps the sign in front of the
    /// name.
    ///
    /// The spellings include more specific families that declare the resolved
    /// properties, the value with and without its type hint, and the forms from
    /// `scalar_forms`.
    fn arbitrary_bases(&self, raw: &str, name: &str, is_negative: bool) -> Option<Vec<String>> {
        let (root, hint, value) = arbitrary(name)?;
        let mut bases = Vec::new();
        let variable = value.starts_with("var(");
        let folded = fold_calc_expressions(&value, None);
        let value = if is_negative && !variable {
            if let Some(positive) = value
                .strip_prefix('-')
                .filter(|value| !value.contains(['(', ' ']))
            {
                positive.into()
            } else if !value.contains(['(', ' ']) {
                format!("-{value}").into()
            } else {
                fold_calc_expressions(&format!("calc({folded} * -1)"), None)
            }
        } else {
            folded
        };
        let negative = if is_negative && variable { "-" } else { "" };
        let hint = hint.map_or(String::new(), |hint| format!("{hint}:"));
        let family = format!("{root}-*");
        let properties: BTreeSet<_> = self
            .context
            .design_system
            .utility_search()
            .definitions_with_prefix(&family)
            .filter(|(name, _)| *name == family)
            .flat_map(|(_, entries)| entries.iter().flat_map(|entry| entry.properties.iter()))
            .collect();
        if properties.len() > 1
            && let Ok(Ok(resolved)) = resolve_utility(raw, self.context.design_system.theme())
        {
            for declaration in resolved.policy_declarations {
                for family in self
                    .context
                    .design_system
                    .utility_search()
                    .names_for_property(&declaration.property)
                {
                    if let Some(specific) = family
                        .strip_suffix("-*")
                        .filter(|specific| specific.starts_with(&format!("{root}-")))
                    {
                        bases.push(format!(
                            "{negative}{}",
                            value_candidate(specific, "", &value)
                        ));
                    }
                }
            }
        }
        bases.push(format!(
            "{negative}{}",
            value_candidate(root, &hint, &value)
        ));
        if !hint.is_empty() {
            bases.push(format!("{negative}{}", value_candidate(root, "", &value)));
        }
        scalar_forms(root, &hint, &value, negative, &mut bases);
        Some(bases)
    }

    /// Returns spellings of the candidate `raw` that change only its value, its type
    /// hint, its modifier, or a legacy utility name. The list is small, and it is not
    /// checked. The caller must check that each spelling produces the same CSS. Returns
    /// an empty list when `raw` cannot be parsed.
    pub(super) fn local_candidates(&self, raw: &str) -> Vec<LeanStr> {
        let Some(parsed) = parse_candidate(raw) else {
            return Vec::new();
        };
        let split = CandidatePrefix::variant_end(raw);
        let prefix = &raw[..split];
        let important = if parsed.important { "!" } else { "" };
        let sign = if parsed.negative { "-" } else { "" };
        let utility = raw[split..].trim_matches('!');
        let modifier = CandidatePrefix::parse(utility)
            .and_then(|prefix| prefix.last_separator('/'))
            .map_or("", |index| &utility[index..]);
        let mut bases = Vec::new();
        let parsed_modifier = match &parsed.utility {
            Utility::Named { modifier, .. } | Utility::ArbitraryProperty { modifier, .. } => {
                modifier.as_ref()
            }
        };
        let modifiers = self.modifier_spellings(&parsed.utility, parsed_modifier, modifier);
        match &parsed.utility {
            Utility::ArbitraryProperty {
                property, value, ..
            } => {
                bases.push(format!("[{property}:{}]", print_arbitrary_value(value)));
            }
            Utility::Named { name, .. } => {
                if let Some(arbitrary) = self.arbitrary_bases(raw, name, parsed.negative) {
                    bases.extend(arbitrary);
                } else {
                    bases.push(format!("{sign}{name}"));
                }
            }
        }
        let mut candidates = Vec::new();
        for base in bases {
            for modifier in &modifiers {
                candidates.push(format!("{prefix}{base}{modifier}{important}").to_lean_str());
            }
        }
        candidates
    }
}

/// Adds named forms of the arbitrary `value` of the family `root` to `bases`.
///
/// The forms are a negative label for `calc(x * -1)`, a count for `repeat(n, ...)`, a
/// percentage label such as `w-50%`, and `root-0` for zero. `hint` is empty or ends
/// with `:`. `negative` is `-` when the value is a negative variable, and empty
/// otherwise. Preferred forms are inserted before the existing entries of `bases`.
fn scalar_forms(root: &str, hint: &str, value: &str, negative: &str, bases: &mut Vec<String>) {
    let printed = print_arbitrary_value(value);
    if let Some(inner) = value
        .strip_prefix("calc(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let (left, right) = split_once_top_level(inner, |token| matches!(token, Token::Delim('*')));
        if let Some(right) = right {
            let operand = if left.trim() == "-1" {
                Some(right.trim())
            } else if right.trim() == "-1" {
                Some(left.trim())
            } else {
                None
            };
            if let Some(operand) = operand {
                bases.insert(0, format!("-{}", value_candidate(root, hint, operand)));
            }
        }
    }
    if let Some(inner) = value
        .strip_prefix("repeat(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let (count, _) = split_once_top_level(inner, |token| matches!(token, Token::Comma));
        if count.trim().parse::<u32>().is_ok() {
            bases.insert(0, format!("{root}-{}", count.trim()));
        }
    }
    if printed.ends_with('%') && !printed.contains('.') {
        bases.push(format!("{negative}{root}-{printed}"));
    }
    if value
        .strip_suffix("rem")
        .or_else(|| value.strip_suffix("px"))
        .unwrap_or(value)
        .parse::<f64>()
        .is_ok_and(|value| value == 0.0)
    {
        bases.insert(0, format!("{root}-0"));
    }
}

/// Splits the named arbitrary utility `name` into its root, its type hint, and its
/// decoded value. For example, `bg-[color:red]` gives `bg`, `color`, and `red`. A
/// variable shorthand such as `bg-(--x)` gives the value `var(--x)`. Returns `None`
/// when `name` has no arbitrary value.
pub(super) fn arbitrary(name: &str) -> Option<(&str, Option<&str>, String)> {
    let (root, inner) = if let Some(index) = name.find("-[") {
        (&name[..index], name[index + 2..].strip_suffix(']')?)
    } else {
        let index = name.find("-(")?;
        let inner = name[index + 2..].strip_suffix(')')?;
        let (hint, inner) = type_hint(inner);
        return Some((&name[..index], hint, format!("var({inner})")));
    };
    let (hint, value) = type_hint(inner);
    Some((root, hint, decode_arbitrary_value(value).to_string()))
}

/// Splits a leading type hint, such as `length` in `length:1px`, from `value`. A hint
/// contains only ASCII letters and `-`. When `value` has no hint, the method returns
/// `None` and `value`.
fn type_hint(value: &str) -> (Option<&str>, &str) {
    value
        .split_once(':')
        .filter(|(hint, _)| {
            !hint.is_empty()
                && hint
                    .bytes()
                    .all(|byte| byte.is_ascii_alphabetic() || byte == b'-')
        })
        .map_or((None, value), |(hint, value)| (Some(hint), value))
}

/// Returns a functional utility of the family `root` with the decoded `value`, such as
/// `w-[1px]`. `hint` must be empty or end with `:`. When `value` is one complete
/// `var(--name)` call, the result uses the variable shorthand, such as `w-(--name)`.
pub(super) fn value_candidate(root: &str, hint: &str, value: &str) -> String {
    if let Some(value) = value
        .strip_prefix("var(")
        .and_then(|value| value.strip_suffix(')'))
        .filter(|inner| {
            inner.starts_with("--")
                && matching_css_delimiter(value, "var".len(), '(', ')')
                    .is_ok_and(|close| close + 1 == value.len())
        })
    {
        let (name, fallback) = value
            .split_once(',')
            .map_or((value, String::new()), |(name, fallback)| {
                (name, format!(",{}", print_arbitrary_value(fallback)))
            });
        format!("{root}-({hint}{name}{fallback})")
    } else {
        format!("{root}-[{hint}{}]", print_arbitrary_value(value))
    }
}
