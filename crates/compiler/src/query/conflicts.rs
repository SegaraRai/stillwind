//! Suggests a logical utility for a `conflicting-utilities` finding.
//!
//! A physical utility such as `pt-2` and a logical utility such as `px-2` overlap only
//! in some writing modes. [`DesignSystemQuery::explain_conflicts`] replaces the physical
//! declaration with its counterpart in the `horizontal-tb` writing mode with the `ltr`
//! direction, and asks the canonical search of this snapshot for a named utility with
//! the same CSS. It offers only a candidate that this search finds, such as `pbs-2`, as
//! the way to use logical properties. It does not recommend logical properties over
//! physical ones, and it offers nothing when the `writingModes` option of the rule
//! leaves out `horizontal-tb`.

use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::{
    WritingMode, declaration_ranges, horizontal_logical_counterpart, writing_mode_group,
};
use stillwind_design_system::{UtilityResolutionError, resolve_utility};
use stillwind_diagnostics::{LintDiagnostic, LintFinding};
use stillwind_syntax::{CandidatePrefix, print_arbitrary_value};

use crate::query::DesignSystemQuery;
use crate::utility_conflicts::{conflict_policy, writing_mode_note};

/// A named logical utility that can replace a physical utility of a finding.
struct LogicalReplacement {
    /// Text that names the replacement, starting with `to use logical properties`.
    text: LeanStr,
    /// Notes about the logical groups that the replacement resolves. Each note has the
    /// spelling of [`writing_mode_note`] for both property orders.
    group_notes: Vec<LeanStr>,
}

impl DesignSystemQuery {
    /// Explains how to use logical properties for each `conflicting-utilities` finding
    /// in `findings` whose candidates are a physical utility and a logical utility that
    /// overlap only in some writing modes. The finding receives the replacement when a
    /// named utility of this snapshot writes the logical form of the physical
    /// declaration with the same value.
    ///
    /// The replacement extends the note about the logical group that the finding already
    /// has, or becomes a new note when the finding has none. The candidates of each
    /// finding must belong to this query's snapshot. Other findings stay unchanged. A
    /// finding also stays unchanged when no named utility matches, or when a utility
    /// definition that the search checks is malformed; compilation reports such a
    /// definition separately. All findings stay unchanged when the `writingModes` option
    /// of the rule in the compiler options of this query leaves out `horizontal-tb`,
    /// because the replacement uses the sides of that writing mode.
    pub fn explain_conflicts(&self, findings: &mut [LintFinding]) {
        if !conflict_policy(&self.context.options.lint_settings())
            .writing_modes
            .contains(&WritingMode::HorizontalTb)
        {
            return;
        }
        for finding in findings {
            let LintDiagnostic::ConflictingUtilities {
                previous, utility, ..
            } = &finding.diagnostic
            else {
                continue;
            };
            let (previous, utility) = (previous.clone(), utility.clone());
            for (physical, other) in [(&previous, &utility), (&utility, &previous)] {
                let Ok(Some(replacement)) = self.logical_replacement(physical, other) else {
                    continue;
                };
                let notes = &mut finding.info.notes;
                match notes
                    .iter()
                    .position(|note| replacement.group_notes.contains(note))
                {
                    Some(index) => {
                        notes[index] =
                            format_args!("{}; {}", notes[index], replacement.text).to_lean_str();
                    }
                    None => notes.push(replacement.text),
                }
            }
        }
    }

    /// Returns the logical replacement of `physical`, when `physical` writes one
    /// physical declaration and `other` writes a property that overlaps it only through
    /// a Webref logical group.
    fn logical_replacement(
        &self,
        physical: &str,
        other: &str,
    ) -> Result<Option<LogicalReplacement>, UtilityResolutionError> {
        let Some(declarations) = self.plain_declarations(physical)? else {
            return Ok(None);
        };
        let [(property, value)] = declarations.as_slice() else {
            return Ok(None);
        };
        let Some(logical) = horizontal_logical_counterpart(property) else {
            return Ok(None);
        };
        let group_notes: Vec<_> = self
            .plain_declarations(other)?
            .unwrap_or_default()
            .iter()
            .filter_map(|(other, _)| {
                let group = writing_mode_group(property, other)?;
                Some([
                    writing_mode_note(property, other, group),
                    writing_mode_note(other, property, group),
                ])
            })
            .flatten()
            .collect();
        if group_notes.is_empty() {
            return Ok(None);
        }
        let variants = &physical[..CandidatePrefix::variant_end(physical)];
        let arbitrary = format!("{variants}[{logical}:{}]", print_arbitrary_value(value));
        let Some(replacement) = self.canonical_candidate(&arbitrary)? else {
            return Ok(None);
        };
        if replacement[variants.len()..].starts_with('[') {
            return Ok(None);
        }
        Ok(Some(LogicalReplacement {
            text: format_args!(
                "to use logical properties, replace `{physical}` with `{replacement}`, which sets `{logical}` in place of `{property}` in horizontal left-to-right writing modes"
            )
            .to_lean_str(),
            group_notes,
        }))
    }

    /// Returns the declarations of the utility part of `candidate` without custom
    /// properties, or `None` when the utility does not resolve, has `!`, or writes
    /// anything other than base declarations on its own class.
    fn plain_declarations(
        &self,
        candidate: &str,
    ) -> Result<Option<Vec<(LeanStr, LeanStr)>>, UtilityResolutionError> {
        let utility = &candidate[CandidatePrefix::variant_end(candidate)..];
        let Ok(resolved) = resolve_utility(utility, self.context.design_system.theme())? else {
            return Ok(None);
        };
        if resolved.candidate.important
            || resolved.selector.is_some()
            || !resolved.nested.is_empty()
            || !resolved.additional.is_empty()
        {
            return Ok(None);
        }
        let css = &resolved.declaration;
        Ok(Some(
            declaration_ranges(css)
                .into_iter()
                .filter_map(|range| {
                    let (property, value) = css[range].split_once(':')?;
                    let property = property.trim();
                    (!property.starts_with("--"))
                        .then(|| (property.to_ascii_lowercase().into(), value.trim().into()))
                })
                .collect(),
        ))
    }
}
