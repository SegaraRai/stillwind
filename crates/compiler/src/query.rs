//! Editor requests use precisely the compiler's candidate resolution and emitter.
//!
//! [`DesignSystemQuery`] answers hover, completion, and diagnostic questions about
//! single candidates without compiling a module. It resolves against one immutable
//! snapshot and reports that snapshot's revision with every answer, so an editor can
//! discard results that belong to a superseded design system. Completion labels are
//! verified by resolving them before they are offered, so inserting a completion never
//! produces a diagnostic.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use stillwind_design_system::{
    DesignSystemRevision, DesignSystemSnapshot, ThemeNamespace, ThemeTokenKind,
    builtin_property_definitions, builtin_utility_token_prefixes, compare_variant_sequences,
    resolve_variant,
};
use stillwind_diagnostics::{LintFinding, LintKind};
use stillwind_source::{FileId, Span};
use stillwind_syntax::{Candidate, parse_candidate};

use crate::candidate::CandidateResolver;
use crate::compiler::{emit_utility_rules, resolve_utility_targets, unresolved_utility_kind};
use crate::identifiers::{ClassContext, IdentifierContext};
use crate::{CompilerOptions, GeneratedRule, ModuleCssArtifact};

/// Everything one candidate produces for an editor.
///
/// Use `diagnostics` as the validity check. The rules are a candidate preview, without
/// the document's theme and property-support closure.
#[derive(Clone, Debug)]
pub struct CandidateResolution {
    /// Snapshot revision this answer was computed from; compare before reusing it.
    pub revision: DesignSystemRevision,
    /// The parsed candidate, `None` when the text is not candidate syntax at all.
    pub parsed: Option<Candidate>,
    /// Rendered rules for the candidate, keyed by their own serialization.
    pub generated_rules: Vec<GeneratedRule>,
    /// Registered property names textually present in the preview. This is a lexical
    /// hint, not the renderer's activation, dependency, and group closure.
    pub required_properties: Vec<String>,
    /// Theme tokens the rendered CSS references.
    pub referenced_tokens: Vec<String>,
    /// Resolution or generated-CSS failures in synthetic file 0. The primary range is
    /// `0..min(candidate.len(), u32::MAX)` in UTF-8 bytes, not a host document location.
    pub diagnostics: Vec<LintFinding>,
}

/// A partially typed candidate, as spelled in the editor.
#[derive(Clone, Debug)]
pub struct CompletionInput {
    /// Text before the cursor, variant prefix included.
    pub prefix: String,
}

/// One label the editor may insert.
#[derive(Clone, Debug)]
pub struct CompletionItem {
    /// Text to insert, ending in `:` for a variant.
    pub label: String,
    /// Snapshot revision the label is valid for.
    pub revision: DesignSystemRevision,
}

/// Rendered CSS for a candidate that resolves.
#[derive(Clone, Debug)]
pub struct CandidateHover {
    /// The candidate that was asked about, as the caller spelled it.
    pub candidate: String,
    /// Concatenated rules for the candidate.
    pub css: String,
    /// Snapshot revision the CSS was rendered from.
    pub revision: DesignSystemRevision,
}

/// Answers candidate questions against one immutable snapshot.
///
/// Holding the snapshot rather than a project keeps editor answers independent of
/// module state and deterministic for as long as the revision does not change.
#[derive(Clone, Debug)]
pub struct DesignSystemQuery {
    /// Immutable revision against which every answer from this query object is computed.
    design_system: DesignSystemSnapshot,
    /// Compiler configuration used for preview generation and name allocation.
    options: CompilerOptions,
}

impl DesignSystemQuery {
    /// Compares two valid candidates using the compiler's variant and utility order.
    ///
    /// Returns `None` if either utility or any of its variants cannot be resolved.
    /// Callers decide how to preserve unresolved text and expression boundaries;
    /// this comparison does not rewrite source or normalize candidate spelling.
    pub fn compare_candidates(&self, left: &str, right: &str) -> Option<Ordering> {
        let theme = self.design_system.theme();
        let resolve = |raw| {
            CandidateResolver::resolve(raw, theme).filter(|resolved| {
                resolved
                    .candidate
                    .variants
                    .iter()
                    .all(|variant| resolve_variant(variant, theme).is_some())
            })
        };
        let left_resolved = resolve(left)?;
        let right_resolved = resolve(right)?;
        Some(
            compare_variant_sequences(
                &left_resolved.candidate.variants,
                &right_resolved.candidate.variants,
                theme,
            )
            .then_with(|| {
                left_resolved
                    .order
                    .compare(left, &right_resolved.order, right)
            }),
        )
    }

    /// Creates a query pinned to `design_system`.
    ///
    /// The options' prefixes are resolved against the snapshot so rendered previews match
    /// what a compilation would emit.
    pub fn new(design_system: DesignSystemSnapshot, options: CompilerOptions) -> Self {
        let options = options.resolved(&design_system);
        Self {
            design_system,
            options,
        }
    }

    /// Resolves one candidate, including its variant prefix, into rendered rules.
    ///
    /// The preview uses the compiler's own emitter and a class name derived from the
    /// candidate text alone, so the declarations a caller sees are the declarations the
    /// candidate would contribute, independent of any module. A candidate whose
    /// variants do not all resolve is reported as unresolved rather than previewed
    /// partially. Failures are typed lint findings over synthetic file 0; hosts must map
    /// their labels to the editor buffer before reporting or applying source suppression.
    pub fn resolve_candidate(&self, candidate: &str) -> CandidateResolution {
        let span = Span::new(
            FileId(0),
            0,
            u32::try_from(candidate.len()).unwrap_or(u32::MAX),
        );
        // The built-in generator cannot reject an input, so the fallback selector is
        // unreachable; previews keep the empty string rather than panicking a host.
        let selector = IdentifierContext::builtin()
            .class_name(&ClassContext {
                module: String::new(),
                namespace: String::new(),
                call: 0,
                branch: format!("{candidate}\0"),
            })
            .map(|name| name.selector())
            .unwrap_or_default();
        let parsed = parse_candidate(candidate);
        let theme = self.design_system.theme();
        let targets = resolve_utility_targets(candidate, theme).filter(|_| {
            parsed.as_ref().is_some_and(|parsed| {
                parsed
                    .variants
                    .iter()
                    .all(|variant| resolve_variant(variant, theme).is_some())
            })
        });
        let mut contributions = Vec::new();
        let mut diagnostics = if let Some(targets) = targets {
            emit_utility_rules(&mut contributions, &selector, &targets, theme);
            Vec::new()
        } else {
            vec![LintFinding::new(
                LintKind::UnresolvedCandidate {
                    candidate: candidate.to_owned(),
                    classification: unresolved_utility_kind(candidate),
                },
                span,
            )]
        };
        let css: String = contributions.iter().map(|part| part.css.as_str()).collect();
        let referenced_tokens = theme
            .tokens_required_by(&css, false)
            .iter()
            .map(|token| token.name.value.clone())
            .collect();
        let properties = theme
            .properties
            .iter()
            .chain(builtin_property_definitions(theme));
        let required_properties = properties
            .filter(|property| {
                css.contains(&property.name)
                    || contributions
                        .iter()
                        .any(|part| part.required_properties.contains(&property.name))
            })
            .map(|property| property.name.clone())
            .collect();
        for contribution in &mut contributions {
            contribution.css = self.options.rewrite_generated(&contribution.css);
            for name in &mut contribution.required_properties {
                *name = self.options.rewrite_generated(name);
            }
        }
        let generated_rules = match ModuleCssArtifact::from_contributions(&contributions) {
            Ok(artifact) => artifact.rules,
            Err(offset) => {
                diagnostics.push(LintFinding::new(
                    LintKind::InvalidCandidateCss {
                        offset,
                        classification: unresolved_utility_kind(candidate),
                    },
                    span,
                ));
                Vec::new()
            }
        };
        CandidateResolution {
            revision: self.design_system.revision(),
            parsed,
            generated_rules,
            required_properties,
            referenced_tokens,
            diagnostics,
        }
    }

    /// Offers labels that continue `input.prefix`.
    ///
    /// The vocabulary is the design system's utility names plus tokens rewritten
    /// through their utility prefixes, and the variant vocabulary adds breakpoint token
    /// names. A label is only returned when it resolves, so the editor never suggests
    /// text that would be reported as unknown. The variant prefix the user already
    /// typed is kept and the remainder is completed after it.
    pub fn complete_candidate(&self, input: CompletionInput) -> Vec<CompletionItem> {
        let prefix = input.prefix;
        let theme = self.design_system.theme();
        let mut vocabulary: BTreeSet<_> = self
            .design_system
            .utilities()
            .names()
            .filter(|name| !name.contains('*'))
            .map(str::to_owned)
            .collect();
        {
            let token_prefixes = builtin_utility_token_prefixes(theme);
            for token in theme.active_tokens() {
                for (prefix, namespace) in &token_prefixes {
                    if let Some(name) = token.name.value.strip_prefix(&format!("--{namespace}-")) {
                        vocabulary.insert(format!("{prefix}-{name}"));
                    }
                }
            }
        }
        let (variants, partial) = prefix
            .rsplit_once(':')
            .map_or((String::new(), prefix.as_str()), |(variants, partial)| {
                (format!("{variants}:"), partial)
            });
        let mut completions: BTreeSet<String> = vocabulary
            .into_iter()
            .filter(|label| label.starts_with(partial))
            .map(|label| format!("{variants}{label}"))
            .filter(|label| self.resolve_candidate(label).diagnostics.is_empty())
            .collect();
        let mut variant_names: BTreeSet<_> = self
            .design_system
            .variants()
            .names()
            .filter(|name| !name.contains('*'))
            .map(str::to_owned)
            .collect();
        {
            variant_names.extend(theme.active_tokens().iter().filter_map(
                |token| match &token.kind {
                    ThemeTokenKind::Token {
                        namespace: ThemeNamespace::Breakpoint,
                        name,
                    } => Some(name.clone()),
                    _ => None,
                },
            ));
        }
        for variant in variant_names {
            if variant.starts_with(partial) {
                completions.insert(format!("{variants}{variant}:"));
            }
        }
        completions
            .into_iter()
            .map(|label| CompletionItem {
                label,
                revision: self.design_system.revision(),
            })
            .collect()
    }

    /// CSS for a candidate that resolves, `None` otherwise.
    ///
    /// Diagnostics are deliberately not part of a hover result: an unresolved candidate
    /// has no CSS to show.
    pub fn hover_candidate(&self, candidate: &str) -> Option<CandidateHover> {
        let result = self.resolve_candidate(candidate);
        result.diagnostics.is_empty().then(|| CandidateHover {
            candidate: candidate.into(),
            css: result
                .generated_rules
                .iter()
                .map(GeneratedRule::to_css)
                .collect(),
            revision: result.revision,
        })
    }

    /// Returns the resolution and generated-CSS findings from [`Self::resolve_candidate`].
    ///
    /// The finding carries a synthetic span over the candidate text, because a query
    /// has no source buffer to point into. A successful preview returns an empty list.
    pub fn diagnose_candidate(&self, candidate: &str) -> Vec<LintFinding> {
        self.resolve_candidate(candidate).diagnostics
    }
}

#[cfg(test)]
mod tests {
    use stillwind_design_system::DesignSystemOptions;

    use super::*;
    use crate::CompilerOptions;

    #[test]
    fn completions_include_theme_values_breakpoints_and_variant_prefixes() {
        let theme = crate::css_fixture::source(
            FileId(1),
            "theme",
            "@import 'stillwind'; @theme { --color-brand: red; --breakpoint-tablet: 48rem; }",
        )
        .unwrap();
        let snapshot =
            DesignSystemSnapshot::from_source(&theme, DesignSystemOptions::default()).unwrap();
        let query = DesignSystemQuery::new(snapshot.clone(), CompilerOptions::default());
        for (prefix, expected) in [
            ("bg-br", "bg-brand"),
            ("tab", "tablet:"),
            ("hov", "hover:"),
            ("hover:bg-br", "hover:bg-brand"),
            ("hover:tab", "hover:tablet:"),
        ] {
            let items = query.complete_candidate(CompletionInput {
                prefix: prefix.into(),
            });
            assert!(
                items.iter().any(|item| item.label == expected),
                "{prefix}: {items:?}"
            );
            assert!(
                items
                    .iter()
                    .all(|item| item.label.starts_with(prefix)
                        && item.revision == snapshot.revision())
            );
            assert!(items.windows(2).all(|pair| pair[0].label < pair[1].label));
        }
        assert!(
            query
                .complete_candidate(CompletionInput {
                    prefix: "missing:bg-br".into()
                })
                .is_empty()
        );
    }
}
