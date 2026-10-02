//! Source variants keep ref identity until they are lowered to CSS applications.
//!
//! A [`StyleVariantPlan`] walks the wrappers around one rule contribution. Ordinary
//! prefixes around utility literals are collected into a string and handed to the
//! candidate resolver, which is what makes a utility prefixed by `hover:` behave
//! exactly like the same candidate written as one token. A ref wrapper cannot be
//! spelled as a string: the selector depends on which element declares the ref and
//! where it sits relative to the target, so the wrapper is lowered into
//! [`VariantApplication`] alternatives that carry the generated markers and shared scope
//! conditions.
//!
//! [`StyleVariantPlan::canonical`] is the identity spelling used for class hashing. It
//! records the ref's generated identifiers and the authored prefix, independent of how
//! the source spelled them. This is one part of class identity; module, callsite, and
//! branch information also participate, so equivalent wrappers alone do not guarantee
//! equal class names.

use std::fmt::Write as _;

use stillwind_css::{instantiate_selector, selector_is_element_condition, selector_uses_subject};
use stillwind_design_system::{
    ThemeIndex, VariantApplication, compare_variant_sequences, resolve_variant,
};
use stillwind_diagnostics::{CompileDiagnostic, CompileError};
use stillwind_ir::{RefCondition, RefDefinition, StyleVariant};
use stillwind_source::Spanned;
use stillwind_syntax::{Variant, parse_candidate};

use super::symbols::{ref_container_name, ref_marker_name};
use super::utilities::resolve::CompiledUtility;
use super::variants::{VariantStep, variant_paths};
use crate::identifiers::{IdentifierContext, RefMarker};

/// The lowered form of the variant wrappers around one rule contribution.
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
    /// to an unrelated selector, because that would silently target the wrong
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
        for wrapper in wrappers
            .iter()
            .filter(|wrapper| matches!(wrapper, StyleVariant::Prefix(_)))
        {
            if let StyleVariant::Prefix(prefix) = wrapper {
                plan.add_prefix(theme, prefix, diagnostics)?;
            }
        }
        let mut independent = wrappers
            .iter()
            .filter(|wrapper| !matches!(wrapper, StyleVariant::Prefix(_)))
            .collect::<Vec<_>>();
        independent.sort_by_cached_key(|wrapper| independent_variant_key(wrapper));
        for wrapper in independent {
            match wrapper {
                StyleVariant::Prefix(prefix) => {
                    plan.add_prefix(theme, prefix, diagnostics)?;
                }
                StyleVariant::Ref { condition, scopes } => {
                    plan.add_ref(context, theme, condition, scopes, diagnostics)?;
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

    /// Adds one condition over referenced elements and their optional shared scopes.
    /// Every scope tests the original ref condition. Expands finite alternatives while
    /// retaining the same reference for all scopes in one branch. Reports unresolved
    /// identities or prefixes that cannot be used as element predicates.
    fn add_ref(
        &mut self,
        context: &IdentifierContext,
        theme: &ThemeIndex,
        condition: &RefCondition,
        scopes: &[RefCondition],
        diagnostics: &mut Vec<CompileError>,
    ) -> Option<()> {
        let references =
            element_conditions(context, theme, condition, !scopes.is_empty(), diagnostics)?;
        let mut ordered_scopes = scopes.iter().collect::<Vec<_>>();
        ordered_scopes.sort_by_cached_key(|scope| condition_key(scope));
        ordered_scopes.dedup_by(|a, b| condition_key(a) == condition_key(b));
        let scope_conditions = ordered_scopes
            .iter()
            .map(|scope| element_conditions(context, theme, scope, false, diagnostics))
            .collect::<Option<Vec<_>>>()?;
        let mut alternatives = Vec::new();
        for reference in references {
            let mut paths = if scope_conditions.is_empty() {
                let mut path = reference.conditions.clone();
                path.push(VariantApplication::SelectorCondition(format!(
                    ":is({} *)",
                    reference.selector
                )));
                vec![path]
            } else {
                vec![reference.conditions.clone()]
            };
            let reference_selector = &reference.selector;
            for candidates in &scope_conditions {
                paths = paths
                    .into_iter()
                    .flat_map(|path| {
                        candidates.iter().map(move |scope| {
                            let mut path = path.clone();
                            path.extend(scope.conditions.clone());
                            path.push(VariantApplication::SelectorCondition(format!(
                                ":is({}:has({}) *)",
                                scope.selector, reference_selector
                            )));
                            path
                        })
                    })
                    .collect();
            }
            alternatives.extend(paths);
        }
        let mut canonical = alternatives
            .iter()
            .map(|path| format!("{path:?}"))
            .collect::<Vec<_>>();
        canonical.sort();
        let _ = write!(self.canonical, "ref({canonical:?})\0");
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
        self.has_refs = true;
        self.variants.extend(parse_prefix(&condition.prefix)?);
        for scope in ordered_scopes {
            self.variants.extend(parse_prefix(&scope.prefix)?);
        }
        Some(())
    }

    /// Adds an independent named container query evaluated for the final target.
    /// Empty text emits the container name without a query condition. Reports missing
    /// identities and identifier generation failures.
    fn add_container(
        &mut self,
        context: &IdentifierContext,
        refs: &[RefDefinition],
        query: &Spanned<String>,
        diagnostics: &mut Vec<CompileError>,
    ) -> Option<()> {
        if refs.is_empty() {
            diagnostics.push(CompileError::new(
                CompileDiagnostic::UnidentifiedRef,
                query.span,
            ));
            return None;
        }
        self.has_refs = true;
        let conditions = refs
            .iter()
            .map(|definition| {
                ref_container_name(context, definition).map(|name| {
                    if query.value.is_empty() {
                        name.as_str().to_owned()
                    } else {
                        format!("{} ({})", name.as_str(), query.value)
                    }
                })
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
    /// `utility.variants` must begin with the ordinary variants already supplied by
    /// this plan, and `theme` must be the registry used to resolve the plan. The method
    /// retains the utility suffix, replaces its variants with the wrappers and suffix,
    /// and stores applications for the shared CSS renderer and composition collector.
    /// Suffix variants that do not resolve are skipped. Empty application paths remain
    /// absent so a plain utility can still join the emitter's base declaration group.
    pub fn apply(&self, utility: &mut CompiledUtility, theme: &ThemeIndex) {
        // The utility resolver already parsed the ordinary wrapper prefixes.
        let suffix = utility
            .variants
            .iter()
            .skip(self.ordinary_count)
            .cloned()
            .collect::<Vec<_>>();
        let applications = suffix
            .iter()
            .filter_map(|variant| resolve_variant(variant, theme))
            .collect::<Vec<_>>();
        let paths: Vec<Vec<_>> = self
            .paths
            .iter()
            .map(|path| {
                path[..self.ordinary_count]
                    .iter()
                    .cloned()
                    .chain(applications.iter().cloned())
                    .chain(path[self.ordinary_count..].iter().cloned())
                    .collect()
            })
            .collect();
        utility.variant_applications = if paths.iter().all(Vec::is_empty) {
            Vec::new()
        } else {
            paths
        };
        utility.variants = self.variants.iter().cloned().chain(suffix).collect();
    }

    /// Replaces `utility`'s variants and applications with this plan's resolved wrappers.
    /// The utility must have no suffix variants that need to be retained. Its declarations
    /// and canonical class identity stay unchanged. Empty paths leave applications absent
    /// so an unwrapped rule can join the emitter's base declaration group.
    pub fn apply_rule(&self, utility: &mut CompiledUtility) {
        utility.variants.clone_from(&self.variants);
        if self.paths.iter().all(Vec::is_empty) {
            utility.variant_applications.clear();
        } else {
            utility.variant_applications.clone_from(&self.paths);
        }
    }
}

/// Orders independent target conditions using declaration identities and decoded
/// predicate text. Source use spans do not change the generated condition order.
fn independent_variant_key(variant: &StyleVariant) -> String {
    match variant {
        StyleVariant::Ref { condition, scopes } => {
            let mut scopes = scopes.iter().map(condition_key).collect::<Vec<_>>();
            scopes.sort();
            format!("ref({:?},{scopes:?})", condition_key(condition))
        }
        StyleVariant::Container { refs, query } => {
            format!("container({:?},{:?})", ref_keys(refs), query.value)
        }
        StyleVariant::Prefix(prefix) => format!("prefix({:?})", prefix.value),
    }
}

fn condition_key(condition: &RefCondition) -> String {
    format!(
        "({:?},{:?})",
        ref_keys(&condition.refs),
        condition.prefix.value
    )
}

fn ref_keys(refs: &[RefDefinition]) -> Vec<String> {
    let mut identities = refs
        .iter()
        .map(|definition| {
            format!(
                "{:?}",
                (
                    &definition.module_id,
                    &definition.binding.value,
                    definition.binding.span,
                    definition.label.as_ref().map(|label| &label.value),
                )
            )
        })
        .collect::<Vec<_>>();
    identities.sort();
    identities
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
            CompileDiagnostic::InvalidVariantPrefix,
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
            CompileDiagnostic::UnavailableVariantPrefix,
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

/// One resolved element predicate and its subject-independent environment guards.
struct ElementCondition {
    selector: String,
    conditions: Vec<VariantApplication>,
}

/// Resolves all identity and prefix alternatives of one element condition. The caller
/// supplies whether the selector will be inserted into `:has()`. Rejects target-only
/// conditions, declaration effects, selectors that omit their subject, pseudo-elements
/// and nested `:has()` selectors. Global media and support conditions remain guards.
fn element_conditions(
    context: &IdentifierContext,
    theme: &ThemeIndex,
    condition: &RefCondition,
    inside_has: bool,
    diagnostics: &mut Vec<CompileError>,
) -> Option<Vec<ElementCondition>> {
    let (_, applications) =
        collect_variant_and_applications(theme, &condition.prefix, diagnostics)?;
    if condition.refs.is_empty() {
        diagnostics.push(CompileError::new(
            CompileDiagnostic::UnidentifiedRef,
            condition.prefix.span,
        ));
        return None;
    }
    let paths = variant_paths(&applications);
    let mut alternatives = Vec::new();
    for definition in &condition.refs {
        let marker = match ref_marker_name(context, definition) {
            Ok(marker) => marker,
            Err(error) => {
                diagnostics.push(CompileError::new(error, definition.call_span));
                return None;
            }
        };
        for path in &paths {
            let Some(predicate) = reference_path(&marker, path, inside_has) else {
                diagnostics.push(CompileError::new(
                    CompileDiagnostic::UnavailableVariantPrefix,
                    condition.prefix.span,
                ));
                return None;
            };
            alternatives.push(predicate);
        }
    }
    alternatives.sort_by(|a, b| {
        a.selector
            .cmp(&b.selector)
            .then_with(|| format!("{:?}", a.conditions).cmp(&format!("{:?}", b.conditions)))
    });
    Some(alternatives)
}

/// Builds an element selector from a marker and a resolved variant path. Returns
/// `None` for steps whose meaning requires a styled target or declaration effects.
fn reference_path(
    marker: &RefMarker,
    path: &[VariantStep<'_>],
    inside_has: bool,
) -> Option<ElementCondition> {
    let mut reference = format!(":where({})", marker.selector());
    let mut conditions = Vec::new();
    for step in path {
        match step {
            VariantStep::SelectorSuffix(suffix) => reference.push_str(suffix),
            VariantStep::SelectorTemplate(template) if selector_uses_subject(template) => {
                reference = instantiate_selector(template, &reference);
            }
            VariantStep::SelectorRule {
                selector,
                declarations,
            } if declarations.is_empty() && selector_uses_subject(selector) => {
                reference = instantiate_selector(selector, &reference);
            }
            VariantStep::SelectorMedia {
                selector,
                condition,
            } if selector_uses_subject(selector) => {
                reference = instantiate_selector(selector, &reference);
                conditions.push(VariantApplication::Media((*condition).to_owned()));
            }
            VariantStep::SelectorMediaRule {
                selector,
                declarations,
                condition,
            } if declarations.is_empty() && selector_uses_subject(selector) => {
                reference = instantiate_selector(selector, &reference);
                conditions.push(VariantApplication::Media((*condition).to_owned()));
            }
            VariantStep::Media(condition) => {
                conditions.push(VariantApplication::Media((*condition).to_owned()));
            }
            VariantStep::Supports(condition) => {
                conditions.push(VariantApplication::Supports((*condition).to_owned()));
            }
            _ => return None,
        }
    }
    if !selector_is_element_condition(&reference, inside_has) {
        return None;
    }
    // `:is` binds the full selector to one element before an ancestor or containment
    // relation is added. Its ancestors may lie outside the containing scope.
    Some(ElementCondition {
        selector: format!(":is({reference})"),
        conditions,
    })
}

#[cfg(test)]
mod reference_path_tests {
    use super::*;

    #[test]
    fn element_conditions_keep_global_guards_and_reject_target_conditions() {
        let marker = RefMarker::new("menu").unwrap();
        let path = [
            VariantStep::Media("(min-width: 40rem)"),
            VariantStep::Supports("display:grid"),
        ];
        let predicate = reference_path(&marker, &path, false).unwrap();
        assert_eq!(
            predicate.conditions,
            [
                VariantApplication::Media("(min-width: 40rem)".into()),
                VariantApplication::Supports("display:grid".into())
            ]
        );
        assert!(
            reference_path(&marker, &[VariantStep::Container("(width > 10px)")], false).is_none()
        );
        assert!(reference_path(&marker, &[VariantStep::StartingStyle], false).is_none());
    }

    #[test]
    fn element_predicates_preserve_complex_selectors_and_check_has_placement() {
        let marker = RefMarker::new("menu").unwrap();
        let predicate = reference_path(
            &marker,
            &[VariantStep::SelectorTemplate(".a &, .b &")],
            true,
        )
        .unwrap();
        assert_eq!(
            predicate.selector,
            ":is(.a :where(.menu), .b :where(.menu))"
        );
        assert!(
            reference_path(&marker, &[VariantStep::SelectorSuffix(":has(.x)")], true).is_none()
        );
        assert!(
            reference_path(&marker, &[VariantStep::SelectorSuffix(":has(.x)")], false).is_some()
        );
        assert!(
            reference_path(&marker, &[VariantStep::SelectorSuffix("::before")], false).is_none()
        );
        assert!(
            reference_path(
                &marker,
                &[VariantStep::SelectorTemplate(".document")],
                false
            )
            .is_none()
        );
    }
}

#[cfg(test)]
mod add_ref_tests {
    use std::sync::Arc;

    use stillwind_source::{FileId, Span};

    use super::*;
    use crate::identifiers::{
        DEFAULT_HASH_LENGTH, DeclarationContext, GeneratedName, GeneratorSet, IdentifierGenerator,
        IdentifierHash, IdentifierMode,
    };

    fn identifiers() -> IdentifierContext {
        IdentifierContext::new(
            GeneratorSet::default(),
            IdentifierMode::Release,
            DEFAULT_HASH_LENGTH,
        )
    }

    fn plan() -> StyleVariantPlan {
        StyleVariantPlan {
            prefix: String::new(),
            canonical: String::new(),
            has_refs: false,
            variants: Vec::new(),
            paths: vec![Vec::new()],
            ordinary_count: 0,
        }
    }

    fn definition(module: &str, binding: &str, call_start: u32) -> RefDefinition {
        RefDefinition {
            module_id: module.into(),
            binding: Spanned::new(binding.to_owned(), Span::new(FileId(1), 0, 3)),
            label: None,
            call_span: Span::new(FileId(1), call_start, call_start + 4),
        }
    }

    fn prefix(value: &str) -> Spanned<String> {
        Spanned::new(
            value.to_owned(),
            Span::new(FileId(1), 90, 90 + u32::try_from(value.len()).unwrap()),
        )
    }

    /// Rejects every ref marker so the generator failure path is observable.
    #[derive(Debug)]
    struct RejectingGenerator;

    impl IdentifierGenerator for RejectingGenerator {
        fn ref_marker(
            &self,
            _context: &DeclarationContext,
            _hash: &IdentifierHash,
        ) -> Result<GeneratedName<RefMarker>, String> {
            Err("no marker for this ref".to_owned())
        }
    }

    /// Each ref contributes one alternative path carrying its own marker, and the
    /// canonical spelling records the identities and the authored prefix.
    #[test]
    fn add_ref_expands_one_alternative_per_ref() {
        let context = identifiers();
        let theme = ThemeIndex::default();
        let mut diagnostics = Vec::new();
        let mut plan = plan();
        let first = definition("src/a.tsx", "menu", 10);
        let second = definition("src/b.tsx", "panel", 20);
        let added = plan.add_ref(
            &context,
            &theme,
            &RefCondition {
                refs: vec![first.clone(), second.clone()],
                prefix: prefix(""),
            },
            &[],
            &mut diagnostics,
        );
        assert!(added.is_some(), "{diagnostics:?}");
        assert!(diagnostics.is_empty());
        assert!(plan.has_refs);
        let first_marker = ref_marker_name(&context, &first).unwrap();
        let second_marker = ref_marker_name(&context, &second).unwrap();
        let expected = vec![
            vec![VariantApplication::SelectorCondition(format!(
                ":is(:is(:where({})) *)",
                first_marker.selector()
            ))],
            vec![VariantApplication::SelectorCondition(format!(
                ":is(:is(:where({})) *)",
                second_marker.selector()
            ))],
        ];
        let mut expected = expected;
        expected.sort_by_key(|path| format!("{path:?}"));
        assert_eq!(plan.applications(), expected.as_slice());
    }

    /// A wrapper without a known ref identity is an error instead of a fallthrough to
    /// an unbounded selector.
    #[test]
    fn add_ref_rejects_a_missing_ref_identity() {
        let context = identifiers();
        let theme = ThemeIndex::default();
        let mut diagnostics = Vec::new();
        let mut plan = plan();
        let prefix = prefix("");
        assert!(
            plan.add_ref(
                &context,
                &theme,
                &RefCondition {
                    refs: vec![],
                    prefix: prefix.clone()
                },
                &[],
                &mut diagnostics
            )
            .is_none()
        );
        assert!(!plan.has_refs);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].diagnostic,
            CompileDiagnostic::UnidentifiedRef
        );
        assert_eq!(diagnostics[0].labels[0].span, prefix.span);
    }

    /// A prefix that is not a complete name list is rejected before any ref work.
    #[test]
    fn add_ref_rejects_an_incomplete_prefix() {
        let context = identifiers();
        let theme = ThemeIndex::default();
        let mut diagnostics = Vec::new();
        let mut plan = plan();
        assert!(
            plan.add_ref(
                &context,
                &theme,
                &RefCondition {
                    refs: vec![definition("src/a.tsx", "menu", 10)],
                    prefix: prefix("hover")
                },
                &[],
                &mut diagnostics,
            )
            .is_none()
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].diagnostic,
            CompileDiagnostic::InvalidVariantPrefix
        );
        assert!(!plan.has_refs);
    }

    /// A generator that rejects the marker family reports the ref that failed.
    #[test]
    fn add_ref_reports_a_rejected_marker() {
        let context = IdentifierContext::new(
            GeneratorSet::new(Arc::new(RejectingGenerator)),
            IdentifierMode::Release,
            5,
        );
        let theme = ThemeIndex::default();
        let mut diagnostics = Vec::new();
        let mut plan = plan();
        let definition = definition("src/a.tsx", "menu", 10);
        let call_span = definition.call_span;
        assert!(
            plan.add_ref(
                &context,
                &theme,
                &RefCondition {
                    refs: vec![definition],
                    prefix: prefix("")
                },
                &[],
                &mut diagnostics
            )
            .is_none()
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].labels[0].span, call_span);
        assert!(
            matches!(
                &diagnostics[0].diagnostic,
                CompileDiagnostic::IdentifierGenerationFailed(family, message)
                    if family.as_ref() == "ref-marker"
                        && message.as_ref() == "no marker for this ref"
            ),
            "{:?}",
            diagnostics[0].diagnostic
        );
    }
}
