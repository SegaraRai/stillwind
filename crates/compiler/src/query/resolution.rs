//! Candidate previews share utility resolution and CSS emission with module compilation.

use stillwind_design_system::{
    DesignSystemRevision, UtilityResolutionError, builtin_property_definitions, resolve_variant,
};
use stillwind_diagnostics::{LintDiagnostic, LintFinding};
use stillwind_source::{FileId, ModuleId, Span};
use stillwind_syntax::{Candidate, parse_candidate};

use super::{DesignSystemQuery, QueryContext};
use crate::compiler::{
    RuleBoundaries, emit_utility_rules, resolve_utility_targets, unresolved_utility_kind,
};
use crate::identifiers::{ClassContext, DEFAULT_HASH_LENGTH, IdentifierContext};
use crate::{GeneratedRule, GeneratorSet, IdentifierMode, ModuleCssArtifact};

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

impl DesignSystemQuery {
    /// Resolves one candidate, including its variant prefix, into rendered rules.
    ///
    /// The preview uses the compiler's own emitter and a class name derived from the
    /// candidate text alone, so the declarations a caller sees are the declarations the
    /// candidate would contribute, independent of any module. A candidate whose
    /// variants do not all resolve is reported as unresolved rather than previewed
    /// partially. Failures are typed lint findings over synthetic file 0; hosts must map
    /// their labels to the editor buffer before reporting or applying source suppression.
    ///
    /// # Errors
    ///
    /// Returns a malformed requirement from a matching utility definition.
    pub fn resolve_candidate(
        &self,
        candidate: &str,
    ) -> Result<CandidateResolution, UtilityResolutionError> {
        self.context.resolve_with_selector(candidate, None, true)
    }

    /// CSS for a candidate that resolves, `None` otherwise.
    ///
    /// Diagnostics are deliberately not part of a hover result: an unresolved candidate
    /// has no CSS to show.
    ///
    /// # Errors
    ///
    /// Returns a malformed requirement from a matching utility definition.
    pub fn hover_candidate(
        &self,
        candidate: &str,
    ) -> Result<Option<CandidateHover>, UtilityResolutionError> {
        let result = self
            .context
            .resolve_with_selector(candidate, Some("&"), false)?;
        Ok(result.diagnostics.is_empty().then(|| CandidateHover {
            candidate: candidate.into(),
            css: result
                .generated_rules
                .iter()
                .map(GeneratedRule::to_css)
                .collect(),
            revision: result.revision,
        }))
    }

    /// Returns the resolution and generated-CSS findings from [`Self::resolve_candidate`].
    ///
    /// The finding carries a synthetic span over the candidate text, because a query
    /// has no source buffer to point into. A successful preview returns an empty list.
    ///
    /// # Errors
    ///
    /// Returns a malformed requirement from a matching utility definition.
    pub fn diagnose_candidate(
        &self,
        candidate: &str,
    ) -> Result<Vec<LintFinding>, UtilityResolutionError> {
        Ok(self.resolve_candidate(candidate)?.diagnostics)
    }
}

impl QueryContext {
    pub(super) fn resolve_with_selector(
        &self,
        candidate: &str,
        preview_selector: Option<&str>,
        include_referenced_tokens: bool,
    ) -> Result<CandidateResolution, UtilityResolutionError> {
        let span = Span::new(
            FileId(0),
            0,
            u32::try_from(candidate.len()).unwrap_or(u32::MAX),
        );
        let selector =
            preview_selector.map_or_else(|| candidate_selector(candidate), str::to_owned);
        let selector = selector.as_str();
        let parsed = parse_candidate(candidate);
        let theme = self.design_system.theme();
        let targets = resolve_utility_targets(candidate, theme)?.filter(|_| {
            parsed.as_ref().is_some_and(|parsed| {
                parsed
                    .variants
                    .iter()
                    .all(|variant| resolve_variant(variant, theme).is_some())
            })
        });
        let mut contributions = Vec::new();
        let mut diagnostics = if let Some(targets) = targets {
            // Helper names use the built-in generator, which accepts every identifier request.
            let identifiers = IdentifierContext::new(
                GeneratorSet::default(),
                IdentifierMode::Release,
                DEFAULT_HASH_LENGTH,
            );
            let _ = emit_utility_rules(&mut contributions, selector, &targets, theme, &identifiers);
            let mut boundaries = RuleBoundaries::new(format!("preview\0{candidate}"), &identifiers);
            for contribution in &mut contributions {
                contribution.css = boundaries.render(&contribution.css).unwrap_or_default();
            }
            Vec::new()
        } else {
            vec![LintFinding::new(
                LintDiagnostic::UnresolvedCandidate {
                    candidate: candidate.to_owned(),
                    classification: unresolved_utility_kind(candidate),
                },
                span,
            )]
        };
        let css: String = contributions.iter().map(|part| part.css.as_str()).collect();
        let referenced_tokens = if include_referenced_tokens {
            theme
                .tokens_required_by(&css, false, &[])
                .iter()
                .map(|token| token.name.value.clone())
                .collect()
        } else {
            Vec::new()
        };
        let properties = theme
            .properties()
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
                    LintDiagnostic::InvalidCandidateCss {
                        offset,
                        classification: unresolved_utility_kind(candidate),
                    },
                    span,
                ));
                Vec::new()
            }
        };
        Ok(CandidateResolution {
            revision: self.design_system.revision(),
            parsed,
            generated_rules,
            required_properties,
            referenced_tokens,
            diagnostics,
        })
    }
}

fn candidate_selector(candidate: &str) -> String {
    IdentifierContext::new(
        GeneratorSet::default(),
        IdentifierMode::Release,
        DEFAULT_HASH_LENGTH,
    )
    .class_name(&ClassContext {
        module: ModuleId::from(""),
        namespace: String::new(),
        call: 0,
        branch: format!("{candidate}\0"),
    })
    .map(|name| name.selector())
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
    use stillwind_diagnostics::UtilityFailure;
    use stillwind_ir::{ScriptLanguage, ScriptModuleKind, SourceAnalyzer};
    use stillwind_js_oxc::OxcSourceAnalyzer;
    use stillwind_source::{CssSourceName, SourceFile};
    use stillwind_test_support::css_source;

    use super::*;
    use crate::{CompilerOptions, CompletionInput, Identifiers, ModuleCompiler};

    /// A query resolves the same declarations a real compilation emits for the same
    /// candidate, pins the snapshot revision, and classifies unresolved candidates.
    #[test]
    fn queries_share_resolved_custom_definitions_and_pin_revisions() {
        let theme = css_source(FileId(1), "@utility brand { color: red; }").unwrap();
        let snapshot = DesignSystemSnapshot::from_source(
            &theme,
            CssSourceName::from("test.css"),
            DesignSystemOptions::default(),
        )
        .unwrap();
        let query = DesignSystemQuery::new(snapshot.clone(), CompilerOptions::default());
        let resolved = query.resolve_candidate("hover:brand").unwrap();
        assert_eq!(resolved.diagnostics, [] as [LintFinding; 0]);
        assert_eq!(resolved.revision, snapshot.revision());
        let source = SourceFile::new(
            FileId(2),
            None,
            None,
            "import { style } from 'stillwind/react'; const x = style('hover:brand');",
        )
        .unwrap();
        let analysis = SourceAnalyzer::finalize(
            &OxcSourceAnalyzer,
            SourceAnalyzer::scan(
                &OxcSourceAnalyzer,
                Arc::new(source),
                ModuleId::from("test.tsx"),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous,
            ),
            None,
        );
        let compiled = ModuleCompiler::new(snapshot.clone(), CompilerOptions::default())
            .compile(&analysis, &Identifiers::default());
        // Queries use their own synthetic producer. Compare declarations, not identities.
        assert_eq!(resolved.generated_rules.len(), compiled.css.rules.len());
        for (queried, generated) in resolved.generated_rules.iter().zip(&compiled.css.rules) {
            assert_eq!(queried.syntax.prelude, generated.syntax.prelude);
            let declarations = |body: &Option<String>| {
                body.as_ref().unwrap().split_once('{').unwrap().1.to_owned()
            };
            assert_eq!(
                declarations(&queried.syntax.body),
                declarations(&generated.syntax.body)
            );
        }
        assert!(
            query
                .hover_candidate("brand")
                .unwrap()
                .unwrap()
                .css
                .replace(' ', "")
                .contains("color:red")
        );
        assert!(query.hover_candidate("missing:brand").unwrap().is_none());
        assert_ne!(
            query.diagnose_candidate("this-does-not-exist").unwrap(),
            [] as [LintFinding; 0]
        );
        for (candidate, classification) in [
            ("hover:[", UtilityFailure::Invalid),
            ("不存在", UtilityFailure::Unknown),
        ] {
            let findings = query.diagnose_candidate(candidate).unwrap();
            assert_eq!(
                findings,
                query.resolve_candidate(candidate).unwrap().diagnostics
            );
            assert_eq!(
                findings[0].diagnostic,
                LintDiagnostic::UnresolvedCandidate {
                    candidate: candidate.into(),
                    classification
                }
            );
            assert_eq!(
                findings[0].to_string(),
                format!("unresolved candidate: {candidate}")
            );
            assert_eq!(findings[0].labels[0].span.file, FileId(0));
            assert_eq!(findings[0].labels[0].span.start.0, 0);
            assert_eq!(findings[0].labels[0].span.end.0 as usize, candidate.len());
        }
        assert!(
            query
                .complete_candidate(CompletionInput {
                    prefix: "br".into()
                })
                .unwrap()
                .iter()
                .any(|item| item.label == "brand")
        );
    }

    /// An empty utility body leaves the candidate unresolved rather than emitting an
    /// empty rule.
    #[test]
    fn an_empty_utility_is_unresolved_and_produces_no_rules() {
        let theme = css_source(FileId(1), "@utility empty {}").unwrap();
        let snapshot = DesignSystemSnapshot::from_source(
            &theme,
            CssSourceName::from("test.css"),
            DesignSystemOptions::default(),
        )
        .unwrap();
        let query = DesignSystemQuery::new(snapshot, CompilerOptions::default());
        let result = query.resolve_candidate("empty").unwrap();
        assert_ne!(result.diagnostics, [] as [LintFinding; 0]);
        assert_eq!(result.generated_rules, [] as [GeneratedRule; 0]);
    }
}
