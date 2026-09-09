//! Editor requests use precisely the compiler's candidate resolution and emitter.

use std::collections::BTreeSet;

use stillwind_grammar::{Candidate, parse_candidate};
use stillwind_source::{FileId, LintFinding, Span};

use crate::{DesignSystemRevision, DesignSystemSnapshot, GeneratedRule, ModuleCssArtifact};

pub(crate) struct CandidateResolver;

impl CandidateResolver {
    pub(crate) fn resolve(
        raw: &str,
        theme: &stillwind_css::ThemeIndex,
    ) -> Option<stillwind_tailwind::ResolvedUtility> {
        stillwind_tailwind::resolve_utility(raw, theme)
    }
}

#[derive(Clone, Debug)]
pub struct CandidateDiagnostic {
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct CandidateResolution {
    pub revision: DesignSystemRevision,
    pub parsed: Option<Candidate>,
    pub generated_rules: Vec<GeneratedRule>,
    pub required_properties: Vec<String>,
    pub referenced_tokens: Vec<String>,
    pub diagnostics: Vec<CandidateDiagnostic>,
}

#[derive(Clone, Debug)]
pub struct CompletionInput {
    pub prefix: String,
}

#[derive(Clone, Debug)]
pub struct CompletionItem {
    pub label: String,
    pub revision: DesignSystemRevision,
}

#[derive(Clone, Debug)]
pub struct CandidateHover {
    pub candidate: String,
    pub css: String,
    pub revision: DesignSystemRevision,
}

#[derive(Clone, Debug)]
pub struct DesignSystemQuery {
    design_system: DesignSystemSnapshot,
}

impl DesignSystemQuery {
    pub fn new(design_system: DesignSystemSnapshot) -> Self {
        Self { design_system }
    }

    pub fn resolve_candidate(&self, candidate: &str) -> CandidateResolution {
        let parsed = parse_candidate(candidate);
        let theme = self.design_system.theme();
        let targets = crate::resolve_utility_targets(candidate, theme).filter(|_| {
            parsed.as_ref().is_some_and(|parsed| {
                parsed
                    .variants
                    .iter()
                    .all(|variant| stillwind_tailwind::resolve_variant(variant, theme).is_some())
            })
        });
        let mut css = String::new();
        let diagnostics = if let Some(targets) = targets {
            crate::emit_utility_rules(
                &mut css,
                &crate::class_name(&format!("{candidate}\0")),
                &targets,
                theme,
            );
            Vec::new()
        } else {
            vec![CandidateDiagnostic {
                message: format!("unresolved candidate: {candidate}"),
            }]
        };
        let referenced_tokens = theme
            .tokens_required_by(&css, false)
            .iter()
            .map(|token| token.name.value.clone())
            .collect();
        let properties = theme
            .properties
            .iter()
            .chain(stillwind_tailwind::builtin_property_definitions());
        let required_properties = properties
            .filter(|property| css.contains(&property.name))
            .map(|property| property.name.clone())
            .collect();
        let css = self.design_system.config().rewrite_generated(&css);
        CandidateResolution {
            revision: self.design_system.revision(),
            parsed,
            generated_rules: ModuleCssArtifact::from_css(&css).rules,
            required_properties,
            referenced_tokens,
            diagnostics,
        }
    }

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
        if theme.imports.builtins {
            let token_prefixes = stillwind_tailwind::builtin_utility_token_prefixes();
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
        if theme.imports.builtins {
            variant_names.extend(theme.active_tokens().iter().filter_map(
                |token| match &token.kind {
                    stillwind_css::ThemeTokenKind::Token {
                        namespace: stillwind_css::ThemeNamespace::Breakpoint,
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

    pub fn diagnose_candidate(&self, candidate: &str) -> Vec<LintFinding> {
        self.resolve_candidate(candidate)
            .diagnostics
            .into_iter()
            .map(|diagnostic| {
                LintFinding::new(
                    crate::unresolved_utility_rule(candidate),
                    diagnostic.message,
                    Span::new(
                        FileId(0),
                        0,
                        u32::try_from(candidate.len()).unwrap_or(u32::MAX),
                    ),
                )
            })
            .collect()
    }
}
