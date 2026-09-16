//! Source variants keep ref identity until they are lowered to CSS applications.
//!
//! A [`StyleVariantPlan`] walks the wrappers around one utility literal. Ordinary
//! prefixes are collected into a string and handed to the ordinary candidate resolver,
//! which is what makes a utility prefixed by `hover:` behave exactly like the same
//! candidate written as one token. A ref wrapper cannot be spelled as a string: the
//! selector depends on which element declares the ref and where it sits relative to the
//! target, so the wrapper is lowered into [`VariantApplication`] alternatives that
//! carry the generated marker and boundary selectors.
//!
//! [`StyleVariantPlan::canonical`] is the identity spelling used for class hashing. It
//! records the ref's generated identifiers and the authored prefix, independent of how
//! the source spelled them. This is one part of class identity; module, callsite, and
//! branch information also participate, so equivalent wrappers alone do not guarantee
//! equal class names.

use std::fmt::Write as _;

use stillwind_css::instantiate_selector;
use stillwind_design_system::{
    ThemeIndex, VariantApplication, compare_variant_sequences, resolve_variant,
};
use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_ir::{RefDefinition, StyleVariant};
use stillwind_source::Spanned;
use stillwind_syntax::{Variant, parse_candidate};

use super::symbols::{boundary_marker_name, ref_container_name, ref_marker_name};
use super::utilities::resolve::CompiledUtility;
use super::variants::{VariantStep, variant_paths};
use crate::identifiers::{IdentifierContext, RefMarker};

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
    /// Compares resolved wrappers with the shared registry ordering and identity tie-break.
    pub fn compare_order(&self, other: &Self, theme: &ThemeIndex) -> std::cmp::Ordering {
        compare_variant_sequences(&self.variants, &other.variants, theme)
            .then_with(|| self.canonical.cmp(&other.canonical))
    }

    /// Resolved alternatives, retaining every selector and condition for context validation.
    pub fn applications(&self) -> &[Vec<VariantApplication>] {
        &self.paths
    }

    /// Resolves the wrappers of one utility literal, reporting unusable prefixes.
    ///
    /// A ref wrapper with no known ref identity is an error rather than a fallthrough
    /// to a selector without a boundary, because that would silently target the wrong
    /// element.
    ///
    /// Returns `None` after pushing a diagnostic; the caller must abandon the
    /// contribution. A runtime condition inside a ref wrapper also rejects, since
    /// computed-value conditions test the target rather than the referenced element.
    pub fn resolve(
        context: &IdentifierContext,
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
            match wrapper {
                StyleVariant::Prefix(prefix) => {
                    plan.add_prefix(theme, prefix, diagnostics)?;
                }
                StyleVariant::Ref { refs, prefix } => {
                    plan.add_ref(context, theme, refs, prefix, diagnostics)?;
                }
                StyleVariant::Container { refs, query } => {
                    plan.add_container(context, refs, query, diagnostics)?;
                }
            }
        }
        Some(plan)
    }

    /// Applies one ordinary prefix wrapper: its text joins the canonical spelling and its
    /// variants extend every path.
    fn add_prefix(
        &mut self,
        theme: &ThemeIndex,
        prefix: &Spanned<String>,
        diagnostics: &mut Vec<CompileError>,
    ) -> Option<()> {
        let (variants, applications) =
            collect_variant_and_applications(theme, prefix, diagnostics)?;
        self.prefix.push_str(&prefix.value);
        self.canonical.push_str(&prefix.value);
        self.ordinary_count += variants.len();
        for path in &mut self.paths {
            path.extend(applications.clone());
        }
        self.variants.extend(variants);
        Some(())
    }

    /// Applies one ref wrapper, expanding one alternative path per matched ref identity.
    ///
    /// Pushes a diagnostic and returns `None` for a ref without an identity and for a path
    /// that carries a runtime condition, which a static selector cannot express.
    fn add_ref(
        &mut self,
        context: &IdentifierContext,
        theme: &ThemeIndex,
        refs: &[RefDefinition],
        prefix: &Spanned<String>,
        diagnostics: &mut Vec<CompileError>,
    ) -> Option<()> {
        let (variants, applications) =
            collect_variant_and_applications(theme, prefix, diagnostics)?;
        if refs.is_empty() {
            diagnostics.push(CompileError::new(
                CompileErrorKind::UnidentifiedRef,
                prefix.span,
            ));
            return None;
        }
        self.has_refs = true;
        let paths = variant_paths(&applications);
        if paths
            .iter()
            .flatten()
            .any(|step| matches!(step, VariantStep::Conditional(_)))
        {
            diagnostics.push(CompileError::new(
                CompileErrorKind::UnavailableVariantPrefix,
                prefix.span,
            ));
            return None;
        }
        let mut alternatives = Vec::new();
        // The canonical spelling is hashed, so the pair keeps tokens as text: a branded
        // name would otherwise contribute its type name to the hash.
        let mut identities: Vec<(String, Option<String>)> = Vec::new();
        for definition in refs {
            let marker = match ref_marker_name(context, definition) {
                Ok(marker) => marker,
                Err(failure) => {
                    diagnostics.push(CompileError::new(failure, definition.call_span));
                    return None;
                }
            };
            let boundary = match definition.boundary.as_ref() {
                Some(boundary) => match boundary_marker_name(context, boundary) {
                    Ok(name) => Some(name.selector()),
                    Err(failure) => {
                        diagnostics.push(CompileError::new(failure, boundary.call_span));
                        return None;
                    }
                },
                None => None,
            };
            identities.push((marker.as_str().to_owned(), boundary.clone()));
            for path in &paths {
                alternatives.push(reference_path(&marker, boundary.as_deref(), path));
            }
        }
        // Names and boundaries are generated identifiers; prefix length makes
        // the variable-length authored portion unambiguous.
        let _ = write!(
            self.canonical,
            "ref({identities:?};{}:{}):",
            prefix.value.len(),
            prefix.value
        );
        self.paths = self
            .paths
            .iter()
            .flat_map(|path| {
                alternatives.iter().map(move |alternative| {
                    let mut path = path.clone();
                    path.extend(alternative.clone());
                    path
                })
            })
            .collect();
        self.variants.extend(variants);
        Some(())
    }

    fn add_container(
        &mut self,
        context: &IdentifierContext,
        refs: &[RefDefinition],
        query: &Spanned<String>,
        diagnostics: &mut Vec<CompileError>,
    ) -> Option<()> {
        if refs.is_empty() {
            diagnostics.push(CompileError::new(
                CompileErrorKind::UnidentifiedRef,
                query.span,
            ));
            return None;
        }
        self.has_refs = true;
        let conditions = refs
            .iter()
            .map(|definition| {
                ref_container_name(context, definition)
                    .map(|name| format!("{} ({})", name.as_str(), query.value))
            })
            .collect::<Result<Vec<_>, _>>();
        let conditions = match conditions {
            Ok(conditions) => conditions,
            Err(failure) => {
                diagnostics.push(CompileError::new(failure, query.span));
                return None;
            }
        };
        let _ = write!(self.canonical, "container({conditions:?}):");
        self.paths = std::mem::take(&mut self.paths)
            .into_iter()
            .flat_map(|path| {
                conditions.iter().map(move |condition| {
                    let mut path = path.clone();
                    path.push(VariantApplication::Container(condition.clone()));
                    path
                })
            })
            .collect();
        Some(())
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

    /// Applies the plan to an keyframes use, which has no candidate text to re-parse.
    pub fn apply_keyframes(&self, utility: &mut CompiledUtility) {
        utility.variants.clone_from(&self.variants);
        if self.has_refs {
            utility.variant_applications.clone_from(&self.paths);
        }
    }
}

/// Parses a wrapper prefix and resolves each of its variants.
///
/// Returns `None` after pushing the diagnostic for a prefix that is not a complete
/// `name:` list and for a variant the registry does not know.
fn collect_variant_and_applications(
    theme: &ThemeIndex,
    prefix: &Spanned<String>,
    diagnostics: &mut Vec<CompileError>,
) -> Option<(Vec<Variant>, Vec<VariantApplication>)> {
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
    Some((variants, applications))
}

/// Parses a wrapper prefix into its variants, requiring complete `name:` segments.
///
/// The prefix is parsed as a candidate followed by a placeholder utility, which reuses
/// the candidate grammar instead of duplicating it here. A prefix that does not end in
/// `:` or that yields an empty variant is rejected, so a malformed wrapper cannot
/// silently shift the meaning of the utility it wraps.
pub(super) fn parse_prefix(prefix: &Spanned<String>) -> Option<Vec<Variant>> {
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
/// not outrank an ordinary variant by accident. A bounded ref requires the boundary to
/// contain the reference, which is how an unrelated element with a matching marker
/// outside the boundary is excluded.
fn reference_path(
    marker: &RefMarker,
    boundary: Option<&str>,
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
            VariantStep::Conditional(condition) => {
                conditions.push(VariantApplication::Conditional((*condition).to_owned()));
            }
            VariantStep::StartingStyle => {
                conditions.push(VariantApplication::StartingStyle);
            }
        }
    }
    let reference = match reference.strip_prefix('&') {
        Some("") => marker.selector(),
        Some(suffix)
            if suffix.starts_with([':', '['])
                && !suffix.contains(['&', ' ', '>', '+', '~', ',']) =>
        {
            format!("{}:where({suffix})", marker.selector())
        }
        _ => format!(
            ":where({})",
            instantiate_selector(&reference, &marker.selector())
        ),
    };
    let selector = match boundary {
        Some(boundary) => format!("{boundary}:has({reference}) &"),
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
