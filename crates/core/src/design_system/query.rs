//! Editor requests use precisely the compiler's candidate resolution and emitter.

use std::collections::BTreeSet;

use stillwind_css::{ThemeNamespace, ThemeTokenKind};
use stillwind_grammar::{Candidate, parse_candidate};
use stillwind_source::{FileId, LintFinding, Span};
use stillwind_tailwind::{
    builtin_property_definitions, builtin_utility_token_prefixes, resolve_variant,
};

use crate::{
    DesignSystemRevision, DesignSystemSnapshot, GeneratedRule, ModuleCssArtifact, class_name,
    emit_utility_rules, resolve_utility_targets, unresolved_utility_rule,
};

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
        let targets = resolve_utility_targets(candidate, theme).filter(|_| {
            parsed.as_ref().is_some_and(|parsed| {
                parsed
                    .variants
                    .iter()
                    .all(|variant| resolve_variant(variant, theme).is_some())
            })
        });
        let mut css = String::new();
        let mut diagnostics = if let Some(targets) = targets {
            emit_utility_rules(
                &mut css,
                &class_name(&format!("{candidate}\0")),
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
            .chain(builtin_property_definitions(theme));
        let required_properties = properties
            .filter(|property| css.contains(&property.name))
            .map(|property| property.name.clone())
            .collect();
        let css = self.design_system.config().rewrite_generated(&css);
        let generated_rules = match ModuleCssArtifact::from_css(&css) {
            Ok(artifact) => artifact.rules,
            Err(error) => {
                diagnostics.push(CandidateDiagnostic {
                    message: error.to_string(),
                });
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
                    unresolved_utility_rule(candidate),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completions_include_theme_values_breakpoints_and_variant_prefixes() {
        let snapshot = DesignSystemSnapshot::from_source(
            &crate::css_fixture::source(
                FileId(1),
                "theme",
                "@import 'stillwind'; @theme { --color-brand: red; --breakpoint-tablet: 48rem; }",
            )
            .unwrap(),
            crate::CompilerOptions::default(),
        )
        .unwrap();
        let query = DesignSystemQuery::new(snapshot.clone());
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
