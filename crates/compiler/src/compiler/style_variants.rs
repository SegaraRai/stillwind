//! Source variants keep ref identity until they are lowered to CSS applications.
//!
//! A [`StyleVariantPlan`] walks the wrappers around one utility literal. Ordinary
//! prefixes are collected into a string and handed to the ordinary candidate resolver,
//! which is what makes a utility prefixed by `hover:` behave exactly like the same
//! candidate written as one token. A ref wrapper cannot be spelled as a string: the
//! selector depends on which element declares the ref and where it sits relative to the
//! target, so the wrapper is lowered into [`VariantApplication`] alternatives that
//! carry the generated marker and scope selectors.
//!
//! [`StyleVariantPlan::canonical`] is the identity spelling used for class hashing. It
//! records the ref's generated identifiers and the authored prefix, independent of how
//! the source spelled them. This is one part of class identity; module, callsite, and
//! branch information also participate, so equivalent wrappers alone do not guarantee
//! equal class names.

use std::fmt::Write as _;

use stillwind_css::instantiate_selector;
use stillwind_design_system::{ThemeIndex, VariantApplication, resolve_variant};
use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_ir::StyleVariant;
use stillwind_source::Spanned;
use stillwind_syntax::{Variant, parse_candidate};

use super::handles::{handle_marker_class, scope_marker_class};
use super::utilities::resolve::CompiledUtility;
use super::variants::{VariantStep, variant_paths};

/// The lowered form of the variant wrappers around one utility literal.
pub(super) struct StyleVariantPlan {
    /// Only ordinary prefixes enter the string-based utility resolver.
    pub prefix: String,
    /// Identity-aware spelling for class hashing; independent of source syntax.
    pub canonical: String,
    /// Whether any wrapper uses ref identity and requires lowered selector applications.
    pub has_refs: bool,
    /// Parsed wrapper variants in traversal order, retained for utility ordering.
    variants: Vec<Variant>,
    /// Alternative application paths in expansion order; each inner vector preserves
    /// wrapper application order.
    paths: Vec<Vec<VariantApplication>>,
    /// Number of parsed ordinary-prefix variants already passed to the candidate resolver.
    ordinary_count: usize,
}

impl StyleVariantPlan {
    /// Resolves the wrappers of one utility literal, reporting unusable prefixes.
    ///
    /// A ref wrapper with no known ref identity is an error rather than a fallthrough
    /// to an unscoped selector, because that would silently target the wrong element.
    ///
    /// Returns `None` after pushing a diagnostic; the caller must abandon the
    /// contribution.
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
                    CompileErrorKind::InvalidVariantPrefix,
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
                    CompileErrorKind::UnavailableVariantPrefix,
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
                            CompileErrorKind::UnidentifiedRef,
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

    /// Applies the plan to a utility the ordinary resolver already prefixed.
    ///
    /// The ordinary count is skipped, so only the wrappers the string resolver could
    /// not express are converted into applications; the utility's own variant list is
    /// rebuilt from the ref wrappers plus that suffix.
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

    /// Applies the plan to an animation use, which has no candidate text to re-parse.
    pub fn apply_animation(&self, utility: &mut CompiledUtility) {
        utility.variants.clone_from(&self.variants);
        if self.has_refs {
            utility.variant_applications.clone_from(&self.paths);
        }
    }
}

/// Parses a wrapper prefix into its variants, requiring complete `name:` segments.
///
/// The prefix is parsed as a candidate followed by a placeholder utility, which reuses
/// the candidate grammar instead of duplicating it here. A prefix that does not end in
/// `:` or that yields an empty variant is rejected, so a malformed wrapper cannot
/// silently shift the meaning of the utility it wraps.
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

/// Builds the selector and conditions that make the utility apply under a ref variant.
///
/// The reference element's own variants are folded into one `:where()`-guarded
/// reference, and the target keeps the combinator relative to it. Wrapping in
/// `:where()` keeps the specificity of the target's own selector, so a ref variant does
/// not outrank an ordinary variant by accident. A scoped ref requires the scope marker
/// to contain the reference, which is how an unrelated element with a matching marker
/// outside the scope is excluded.
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
                reference = instantiate_selector(template, &reference);
            }
            VariantStep::SelectorRule {
                selector,
                declarations: additions,
            } => {
                reference = instantiate_selector(selector, &reference);
                declarations.extend_from_slice(additions);
            }
            VariantStep::SelectorMedia {
                selector,
                condition,
            } => {
                reference = instantiate_selector(selector, &reference);
                conditions.push(VariantApplication::Media((*condition).to_owned()));
            }
            VariantStep::SelectorMediaRule {
                selector,
                declarations: additions,
                condition,
            } => {
                reference = instantiate_selector(selector, &reference);
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
        _ => format!(
            ":where({})",
            instantiate_selector(&reference, &format!(".{marker}"))
        ),
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
