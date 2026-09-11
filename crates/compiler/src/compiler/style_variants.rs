//! Source variants keep ref identity until they are lowered to CSS applications.

use std::fmt::Write as _;

use stillwind_design_system::{ThemeIndex, VariantApplication, resolve_variant};
use stillwind_ir::StyleVariant;
use stillwind_source::{CompileError, Spanned};
use stillwind_syntax::{Variant, parse_candidate};

use super::handles::{handle_marker_class, scope_marker_class};
use super::utilities::resolve::CompiledUtility;
use super::variants::{VariantStep, variant_paths};

pub(super) struct StyleVariantPlan {
    /// Only ordinary prefixes enter the string-based utility resolver.
    pub prefix: String,
    /// Identity-aware spelling for class hashing; independent of source syntax.
    pub canonical: String,
    pub has_refs: bool,
    variants: Vec<Variant>,
    paths: Vec<Vec<VariantApplication>>,
    ordinary_count: usize,
}

impl StyleVariantPlan {
    pub fn resolve(
        wrappers: &[StyleVariant],
        theme: &ThemeIndex,
        diagnostics: &mut Vec<CompileError>,
    ) -> Option<Self> {
        let mut plan = Self {
            prefix: String::new(),
            canonical: String::new(),
            has_refs: false,
            variants: Vec::new(),
            paths: vec![Vec::new()],
            ordinary_count: 0,
        };
        for wrapper in wrappers {
            let prefix = match wrapper {
                StyleVariant::Prefix(prefix) | StyleVariant::Ref { prefix, .. } => prefix,
            };
            let Some(variants) = parse_prefix(prefix) else {
                diagnostics.push(CompileError::new(
                    "SHREF003",
                    "invalid variant prefix",
                    prefix.span,
                ));
                return None;
            };
            let Some(applications) = variants
                .iter()
                .map(|variant| resolve_variant(variant, theme))
                .collect::<Option<Vec<_>>>()
            else {
                diagnostics.push(CompileError::new(
                    "SHREF003",
                    "unknown or unavailable variant prefix",
                    prefix.span,
                ));
                return None;
            };
            match wrapper {
                StyleVariant::Prefix(_) => {
                    plan.prefix.push_str(&prefix.value);
                    plan.canonical.push_str(&prefix.value);
                    plan.ordinary_count += variants.len();
                    for path in &mut plan.paths {
                        path.extend(applications.clone());
                    }
                }
                StyleVariant::Ref { handles, .. } => {
                    if handles.is_empty() {
                        diagnostics.push(CompileError::new(
                            "SHREF003",
                            "ref variant has no known ref identity",
                            prefix.span,
                        ));
                        return None;
                    }
                    plan.has_refs = true;
                    let paths = variant_paths(&applications);
                    let mut alternatives = Vec::new();
                    let mut identities = Vec::new();
                    for handle in handles {
                        let marker = handle_marker_class(handle);
                        let scope = handle.scope.as_ref().map(scope_marker_class);
                        identities.push((marker.clone(), scope.clone()));
                        for path in &paths {
                            alternatives.push(reference_path(&marker, scope.as_deref(), path));
                        }
                    }
                    // Names and scopes are generated identifiers; prefix length makes
                    // the variable-length authored portion unambiguous.
                    let _ = write!(
                        plan.canonical,
                        "ref({identities:?};{}:{}):",
                        prefix.value.len(),
                        prefix.value
                    );
                    plan.paths = plan
                        .paths
                        .into_iter()
                        .flat_map(|path| {
                            alternatives.iter().map(move |alternative| {
                                let mut path = path.clone();
                                path.extend(alternative.clone());
                                path
                            })
                        })
                        .collect();
                }
            }
            plan.variants.extend(variants);
        }
        Some(plan)
    }

    pub fn apply(&self, utility: &mut CompiledUtility, theme: &ThemeIndex) {
        // The utility resolver already parsed the ordinary wrapper prefixes.
        let suffix = utility
            .variants
            .iter()
            .skip(self.ordinary_count)
            .cloned()
            .collect::<Vec<_>>();
        if self.has_refs {
            let applications = suffix
                .iter()
                .filter_map(|variant| resolve_variant(variant, theme))
                .collect::<Vec<_>>();
            utility.variant_applications = self
                .paths
                .iter()
                .map(|path| {
                    let mut path = path.clone();
                    path.extend(applications.clone());
                    path
                })
                .collect();
            utility.variants = self.variants.iter().cloned().chain(suffix).collect();
        }
    }

    pub fn apply_animation(&self, utility: &mut CompiledUtility) {
        utility.variants.clone_from(&self.variants);
        if self.has_refs {
            utility.variant_applications.clone_from(&self.paths);
        }
    }
}

fn parse_prefix(prefix: &Spanned<String>) -> Option<Vec<Variant>> {
    if !prefix.value.is_empty() && !prefix.value.ends_with(':') {
        return None;
    }
    let candidate = parse_candidate(&format!("{}utility", prefix.value))?;
    if candidate
        .variants
        .iter()
        .any(|variant| variant.raw.is_empty())
    {
        return None;
    }
    Some(candidate.variants)
}

fn reference_path(
    marker: &str,
    scope: Option<&str>,
    path: &[VariantStep<'_>],
) -> Vec<VariantApplication> {
    let mut reference = "&".to_owned();
    let mut conditions = Vec::new();
    let mut declarations = Vec::new();
    for step in path {
        match step {
            VariantStep::SelectorSuffix(suffix) => {
                reference.push_str(suffix);
            }
            VariantStep::SelectorTemplate(template) => {
                reference = template.replace('&', &reference);
            }
            VariantStep::SelectorRule {
                selector,
                declarations: additions,
            } => {
                reference = selector.replace('&', &reference);
                declarations.extend_from_slice(additions);
            }
            VariantStep::SelectorMedia {
                selector,
                condition,
            } => {
                reference = selector.replace('&', &reference);
                conditions.push(VariantApplication::Media((*condition).to_owned()));
            }
            VariantStep::SelectorMediaRule {
                selector,
                declarations: additions,
                condition,
            } => {
                reference = selector.replace('&', &reference);
                declarations.extend_from_slice(additions);
                conditions.push(VariantApplication::Media((*condition).to_owned()));
            }
            VariantStep::Media(condition) => {
                conditions.push(VariantApplication::Media((*condition).to_owned()));
            }
            VariantStep::Container(condition) => {
                conditions.push(VariantApplication::Container((*condition).to_owned()));
            }
            VariantStep::Supports(condition) => {
                conditions.push(VariantApplication::Supports((*condition).to_owned()));
            }
            VariantStep::StartingStyle => {
                conditions.push(VariantApplication::StartingStyle);
            }
        }
    }
    let reference = match reference.strip_prefix('&') {
        Some("") => format!(".{marker}"),
        Some(suffix)
            if suffix.starts_with([':', '['])
                && !suffix.contains(['&', ' ', '>', '+', '~', ',']) =>
        {
            format!(".{marker}:where({suffix})")
        }
        _ => format!(":where({})", reference.replace('&', &format!(".{marker}"))),
    };
    let selector = match scope {
        Some(scope) => format!("{scope}:has({reference}) &"),
        None => format!("{reference} &"),
    };
    conditions.push(if declarations.is_empty() {
        VariantApplication::SelectorTemplate(selector)
    } else {
        VariantApplication::SelectorRule {
            selector,
            declarations,
        }
    });
    conditions
}
