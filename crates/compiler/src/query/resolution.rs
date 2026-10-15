//! Candidate previews share utility resolution and CSS emission with module
//! compilation.

use std::cell::RefCell;
use std::rc::Rc;

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_design_system::{
    DesignSystemRevision, UtilityResolutionError, builtin_property_definitions, resolve_variant,
};
use stillwind_diagnostics::{LintDiagnostic, LintFinding};
use stillwind_source::{FileId, ModuleId, Span};
use stillwind_syntax::{Candidate, parse_candidate};

use super::{DesignSystemQuery, QueryContext};
use crate::compiler::{
    RuleBoundaries, emit_utility_rules, lower_conditionals, preview_resource_urls,
    resolve_utility_targets, unresolved_utility_kind,
};
use crate::identifiers::{ClassContext, DEFAULT_HASH_LENGTH, IdentifierContext, PreviewNames};
use crate::{CssContribution, GeneratedRule, GeneratorSet, IdentifierMode, ModuleCssArtifact};

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
    pub required_properties: Vec<LeanStr>,
    /// Theme tokens the rendered CSS references.
    pub referenced_tokens: Vec<LeanStr>,
    /// Resolution or generated-CSS failures in synthetic file 0. The primary range is
    /// `0..min(candidate.len(), u32::MAX)` in UTF-8 bytes, not a host document
    /// location.
    pub diagnostics: Vec<LintFinding>,
}

/// Rendered CSS for a candidate that resolves.
#[derive(Clone, Debug)]
pub struct CandidateHover {
    /// The candidate that was asked about, as the caller spelled it.
    pub candidate: LeanStr,
    /// Concatenated rules for the candidate.
    pub css: LeanStr,
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
    /// partially. Failures are typed lint findings over synthetic file 0; hosts must
    /// map their labels to the editor buffer before reporting or applying source
    /// suppression.
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
}

impl QueryContext {
    /// Lowers runtime conditionals in balanced declaration CSS using the configured
    /// mode and fresh display names, then restores authored URLs. `css` must already
    /// use the intended subject and prefix spelling. Failed lowering returns `None`
    /// without partial output; this operation does not validate arbitrary CSS or add
    /// document support rules.
    pub(super) fn preview_declarations(&self, css: &str) -> Option<LeanStr> {
        lower_conditionals(css, self.options.use_css_if, &candidate_identifiers(true))
            .ok()
            .map(|css| LeanStr::from(preview_resource_urls(css, &self.design_system)))
    }

    /// Rewrites contributions in place with merge barriers shared by this `candidate`.
    /// When `preview` is true, runtime conditionals are also lowered using this query's
    /// configured mode. `identifiers` supplies names consistently across the
    /// contributions. Failed contributions are cleared and reported against `span`,
    /// which addresses the synthetic candidate input. Successful contributions remain
    /// even if another fails.
    fn finalize_rules(
        &self,
        candidate: &str,
        contributions: &mut [CssContribution],
        identifiers: &IdentifierContext,
        preview: bool,
        span: Span,
    ) -> Vec<LintFinding> {
        let mut boundaries = RuleBoundaries::new(
            format_args!("preview\0{candidate}").to_lean_string(),
            identifiers,
        );
        let mut diagnostics = Vec::new();
        for contribution in contributions {
            let css = boundaries.render(&contribution.css).and_then(|css| {
                if preview {
                    lower_conditionals(&css, self.options.use_css_if, identifiers)
                } else {
                    Ok(css)
                }
            });
            if let Ok(css) = css {
                contribution.css = (css).into();
            } else {
                contribution.css = LeanStr::new();
                diagnostics.push(invalid_candidate_css(candidate, span, 0));
            }
        }
        diagnostics
    }

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
            preview_selector.map_or_else(|| candidate_selector(candidate), LeanString::from);
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
            let identifiers = candidate_identifiers(preview_selector.is_some());
            let emission =
                emit_utility_rules(&mut contributions, selector, &targets, theme, &identifiers);
            let mut diagnostics = self.finalize_rules(
                candidate,
                &mut contributions,
                &identifiers,
                preview_selector.is_some(),
                span,
            );
            if emission.is_err() {
                diagnostics.push(invalid_candidate_css(candidate, span, 0));
            }
            diagnostics
        } else {
            vec![LintFinding::new(
                LintDiagnostic::UnresolvedCandidate {
                    candidate: candidate.to_lean_str(),
                    classification: unresolved_utility_kind(candidate),
                },
                span,
            )]
        };
        let css: LeanString = contributions.iter().map(|part| part.css.as_str()).collect();
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
                css.contains(property.name.as_str())
                    || contributions
                        .iter()
                        .any(|part| part.required_properties.contains(&property.name))
            })
            .map(|property| property.name.clone())
            .collect();
        for contribution in &mut contributions {
            contribution.css = self.options.rewrite_generated(&contribution.css);
            if preview_selector.is_some() {
                contribution.css = (preview_resource_urls(
                    std::mem::take(&mut contribution.css).into(),
                    &self.design_system,
                ))
                .into();
            }
            for name in &mut contribution.required_properties {
                *name = self.options.rewrite_generated(name);
            }
        }
        let generated_rules = match ModuleCssArtifact::from_contributions(&contributions) {
            Ok(artifact) => artifact.rules,
            Err(offset) => {
                diagnostics.push(invalid_candidate_css(candidate, span, offset));
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

fn candidate_selector(candidate: &str) -> LeanString {
    IdentifierContext::new(
        GeneratorSet::default(),
        IdentifierMode::Release,
        DEFAULT_HASH_LENGTH,
    )
    .class_name(&ClassContext {
        module: ModuleId::from(""),
        namespace: (LeanString::new()).into(),
        call: 0,
        branch: format_args!("{candidate}\0").to_lean_str(),
    })
    .map(|name| name.selector())
    .unwrap_or_default()
}

/// Starts an independent context with built-in release naming and the default hash
/// length. A true `preview` adds fresh readable names for helpers; it does not select
/// a CSS subject.
fn candidate_identifiers(preview: bool) -> IdentifierContext {
    let identifiers = IdentifierContext::new(
        GeneratorSet::default(),
        IdentifierMode::Release,
        DEFAULT_HASH_LENGTH,
    );
    if preview {
        identifiers.with_preview_names(Rc::new(RefCell::new(PreviewNames::default())))
    } else {
        identifiers
    }
}

/// Reports invalid generated CSS at `span`, classified from the original `candidate`.
/// `offset` is a UTF-8 byte offset within the failed CSS contribution. Zero also
/// represents an emission failure without a parser location. It is not added to the
/// synthetic candidate span.
fn invalid_candidate_css(candidate: &str, span: Span, offset: usize) -> LintFinding {
    LintFinding::new(
        LintDiagnostic::InvalidCandidateCss {
            offset,
            classification: unresolved_utility_kind(candidate),
        },
        span,
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
    use stillwind_diagnostics::UtilityFailure;
    use stillwind_ir::{ScriptLanguage, ScriptModuleKind, SourceAnalyzer};
    use stillwind_js_oxc::OxcSourceAnalyzer;
    use stillwind_source::{CssSourceName, ReplaceText, SourceFile};
    use stillwind_test_support::css_source;

    use super::super::CandidateContext;
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
            assert_eq!(queried.syntax().prelude, generated.syntax().prelude);
            let declarations = |body: &Option<LeanStr>| {
                body.as_ref()
                    .unwrap()
                    .split_once('{')
                    .unwrap()
                    .1
                    .to_lean_string()
            };
            assert_eq!(
                declarations(&queried.syntax().body),
                declarations(&generated.syntax().body)
            );
        }
        assert!(
            query
                .hover_candidate("brand")
                .unwrap()
                .unwrap()
                .css
                .replace_char(' ', "")
                .contains("color:red")
        );
        assert!(query.hover_candidate("missing:brand").unwrap().is_none());
        assert_ne!(
            query
                .resolve_candidate("this-does-not-exist")
                .unwrap()
                .diagnostics,
            [] as [LintFinding; 0]
        );
        for (candidate, classification) in [
            ("hover:[", UtilityFailure::Invalid),
            ("不存在", UtilityFailure::Unknown),
        ] {
            let findings = query.resolve_candidate(candidate).unwrap().diagnostics;
            assert_eq!(
                findings[0].diagnostic,
                LintDiagnostic::UnresolvedCandidate {
                    candidate: candidate.into(),
                    classification
                }
            );
            assert_eq!(
                findings[0].to_lean_string(),
                format_args!("unresolved candidate: {candidate}").to_lean_string()
            );
            assert_eq!(findings[0].labels[0].span.file, FileId(0));
            assert_eq!(findings[0].labels[0].span.start.0, 0);
            assert_eq!(findings[0].labels[0].span.end.0 as usize, candidate.len());
        }
        assert!(
            query
                .complete_candidate(CompletionInput {
                    prefix: "br".into(),
                    context: CandidateContext::Utility,
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
