//! The plan that lowers the variant wrappers around one rule contribution.

use std::fmt::Write as _;

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_design_system::{
    ThemeIndex, VariantApplication, compare_variant_sequences, resolve_variant,
};
use stillwind_diagnostics::{CompileError, LintDiagnostic, LintFinding};
use stillwind_ir::{RefCondition, RefDefinition, StyleVariant};
use stillwind_source::Spanned;
use stillwind_syntax::Variant;

use super::super::symbols::ref_container_name;
use super::super::utilities::CompiledUtility;
use super::prefix::{collect_variant_and_applications, parse_prefix};
use super::reference::element_conditions;
use crate::identifiers::IdentifierContext;

/// The lowered form of the variant wrappers around one rule contribution.
pub(in crate::compiler) struct StyleVariantPlan {
    /// Only ordinary prefixes enter the string-based utility resolver.
    pub(in crate::compiler) prefix: LeanString,
    /// Identity-aware spelling for class hashing; independent of source syntax.
    pub(in crate::compiler) canonical: LeanString,
    /// Whether any wrapper uses ref identity and requires lowered selector
    /// applications.
    pub(in crate::compiler) has_refs: bool,
    /// Parsed wrapper variants in traversal order, retained for utility ordering.
    variants: Vec<Variant>,
    /// Alternative application paths in expansion order; each inner vector preserves
    /// wrapper application order.
    paths: Vec<Vec<VariantApplication>>,
    /// Number of parsed ordinary-prefix variants already passed to the candidate
    /// resolver.
    ordinary_count: usize,
}

impl StyleVariantPlan {
    /// Compares resolved wrappers with the shared registry ordering and identity tie
    /// break.
    pub fn compare_order(&self, other: &Self, theme: &ThemeIndex) -> std::cmp::Ordering {
        compare_variant_sequences(&self.variants, &other.variants, theme)
            .then_with(|| self.canonical.cmp(&other.canonical))
    }

    /// Resolved alternatives, retaining every selector and condition for context
    /// validation.
    pub fn applications(&self) -> &[Vec<VariantApplication>] {
        &self.paths
    }

    /// Resolves the wrappers of one utility literal.
    ///
    /// Returns `None` when a wrapper condition is false or cannot be generated, and the
    /// caller must lower the contribution as empty. A malformed or unknown prefix, a
    /// ref condition with no ref declaration, and a prefix that cannot test the
    /// referenced element add lint findings. A ref condition never falls back to an
    /// unrelated selector, because that would apply the rules to the wrong element. A
    /// failure to generate a ref marker name adds a fatal diagnostic.
    pub fn resolve(
        context: &IdentifierContext,
        wrappers: &[StyleVariant],
        theme: &ThemeIndex,
        diagnostics: &mut Vec<CompileError>,
        lint_findings: &mut Vec<LintFinding>,
    ) -> Option<Self> {
        let mut plan = Self {
            prefix: (LeanString::new()),
            canonical: (LeanString::new()),
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
                plan.add_prefix(theme, prefix, lint_findings)?;
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
                    plan.add_prefix(theme, prefix, lint_findings)?;
                }
                StyleVariant::Ref { condition, scopes } => {
                    plan.add_ref(
                        context,
                        theme,
                        condition,
                        scopes,
                        diagnostics,
                        lint_findings,
                    )?;
                }
                StyleVariant::Container { refs, query } => {
                    plan.add_container(context, refs, query, diagnostics, lint_findings)?;
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
        prefix: &Spanned<LeanStr>,
        lint_findings: &mut Vec<LintFinding>,
    ) -> Option<()> {
        let (variants, applications) =
            collect_variant_and_applications(theme, prefix, lint_findings)?;
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
        lint_findings: &mut Vec<LintFinding>,
    ) -> Option<()> {
        let references = element_conditions(
            context,
            theme,
            condition,
            !scopes.is_empty(),
            diagnostics,
            lint_findings,
        )?;
        let mut ordered_scopes = scopes.iter().collect::<Vec<_>>();
        ordered_scopes.sort_by_cached_key(|scope| condition_key(scope));
        ordered_scopes.dedup_by(|a, b| condition_key(a) == condition_key(b));
        let scope_conditions = ordered_scopes
            .iter()
            .map(|scope| {
                element_conditions(context, theme, scope, false, diagnostics, lint_findings)
            })
            .collect::<Option<Vec<_>>>()?;
        let mut alternatives = Vec::new();
        for reference in references {
            let mut paths = if scope_conditions.is_empty() {
                let mut path = reference.conditions.clone();
                path.push(VariantApplication::SelectorCondition(
                    format_args!(":is({} *)", reference.selector).to_lean_str(),
                ));
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
                            path.push(VariantApplication::SelectorCondition(
                                format_args!(
                                    ":is({}:has({}) *)",
                                    scope.selector, reference_selector
                                )
                                .to_lean_str(),
                            ));
                            path
                        })
                    })
                    .collect();
            }
            alternatives.extend(paths);
        }
        let mut canonical = alternatives
            .iter()
            .map(|path| format_args!("{path:?}").to_lean_string())
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
        query: &Spanned<LeanStr>,
        diagnostics: &mut Vec<CompileError>,
        lint_findings: &mut Vec<LintFinding>,
    ) -> Option<()> {
        if refs.is_empty() {
            lint_findings.push(LintFinding::new(
                LintDiagnostic::EmptyRefCondition,
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
                        name.as_str().to_lean_string()
                    } else {
                        format_args!("{} ({})", name.as_str(), query.value).to_lean_string()
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
                    path.push(VariantApplication::Container((condition.clone()).into()));
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
            .filter_map(|variant| resolve_variant(variant, theme).ok())
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
        utility.source.wrapper_variant_count = self.variants.len();
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
fn independent_variant_key(variant: &StyleVariant) -> LeanString {
    match variant {
        StyleVariant::Ref { condition, scopes } => {
            let mut scopes = scopes.iter().map(condition_key).collect::<Vec<_>>();
            scopes.sort();
            format_args!("ref({:?},{scopes:?})", condition_key(condition)).to_lean_string()
        }
        StyleVariant::Container { refs, query } => {
            format_args!("container({:?},{:?})", ref_keys(refs), query.value).to_lean_string()
        }
        StyleVariant::Prefix(prefix) => format_args!("prefix({:?})", prefix.value).to_lean_string(),
    }
}

fn condition_key(condition: &RefCondition) -> LeanString {
    format_args!(
        "({:?},{:?})",
        ref_keys(&condition.refs),
        condition.prefix.value
    )
    .to_lean_string()
}

fn ref_keys(refs: &[RefDefinition]) -> Vec<LeanStr> {
    let mut identities = refs
        .iter()
        .map(|definition| {
            format_args!(
                "{:?}",
                (
                    &definition.module_id,
                    &definition.binding.value,
                    definition.binding.span,
                    definition.label.as_ref().map(|label| &label.value),
                )
            )
            .to_lean_str()
        })
        .collect::<Vec<_>>();
    identities.sort();
    identities
}

#[cfg(test)]
mod add_ref_tests {
    use std::rc::Rc;

    use stillwind_diagnostics::CompileDiagnostic;
    use stillwind_source::{FileId, Span};

    use super::super::super::symbols::ref_marker_name;
    use super::*;
    use crate::identifiers::{
        DEFAULT_HASH_LENGTH, DeclarationContext, GeneratedName, GeneratorSet, IdentifierGenerator,
        IdentifierHash, IdentifierMode, RefMarker,
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
            prefix: LeanString::new(),
            canonical: LeanString::new(),
            has_refs: false,
            variants: Vec::new(),
            paths: vec![Vec::new()],
            ordinary_count: 0,
        }
    }

    fn definition(module: &str, binding: &str, call_start: u32) -> RefDefinition {
        RefDefinition {
            module_id: module.into(),
            binding: Spanned::new(binding.to_lean_str(), Span::new(FileId(1), 0, 3)),
            label: None,
            call_span: Span::new(FileId(1), call_start, call_start + 4),
        }
    }

    fn prefix(value: &str) -> Spanned<LeanStr> {
        Spanned::new(
            value.to_lean_str(),
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
        ) -> Result<GeneratedName<RefMarker>, LeanString> {
            Err("no marker for this ref".to_lean_string())
        }
    }

    /// Each ref contributes one alternative path carrying its own marker, and the
    /// canonical spelling records the identities and the authored prefix.
    #[test]
    fn add_ref_expands_one_alternative_per_ref() {
        let context = identifiers();
        let theme = ThemeIndex::default();
        let mut diagnostics = Vec::new();
        let mut lint_findings = Vec::new();
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
            &mut lint_findings,
        );
        assert!(added.is_some(), "{diagnostics:?}");
        assert_eq!(diagnostics, [] as [CompileError; 0]);
        assert!(plan.has_refs);
        let first_marker = ref_marker_name(&context, &first).unwrap();
        let second_marker = ref_marker_name(&context, &second).unwrap();
        let expected = vec![
            vec![VariantApplication::SelectorCondition(
                format_args!(":is(:is(:where({})) *)", first_marker.selector()).to_lean_str(),
            )],
            vec![VariantApplication::SelectorCondition(
                format_args!(":is(:is(:where({})) *)", second_marker.selector()).to_lean_str(),
            )],
        ];
        let mut expected = expected;
        expected.sort_by_key(|path| format_args!("{path:?}").to_lean_string());
        assert_eq!(plan.applications(), expected.as_slice());
    }

    /// A wrapper without a known ref identity is an error instead of a fallthrough to
    /// an unbounded selector.
    #[test]
    fn add_ref_rejects_a_missing_ref_identity() {
        let context = identifiers();
        let theme = ThemeIndex::default();
        let mut diagnostics = Vec::new();
        let mut lint_findings = Vec::new();
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
                &mut diagnostics,
                &mut lint_findings
            )
            .is_none()
        );
        assert!(!plan.has_refs);
        assert_eq!(diagnostics, [] as [CompileError; 0]);
        assert_eq!(lint_findings.len(), 1);
        assert_eq!(
            lint_findings[0].diagnostic,
            LintDiagnostic::EmptyRefCondition
        );
        assert_eq!(lint_findings[0].labels[0].span, prefix.span);
    }

    /// A prefix that is not a complete name list is rejected before any ref work.
    #[test]
    fn add_ref_rejects_an_incomplete_prefix() {
        let context = identifiers();
        let theme = ThemeIndex::default();
        let mut diagnostics = Vec::new();
        let mut lint_findings = Vec::new();
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
                &mut lint_findings
            )
            .is_none()
        );
        assert_eq!(lint_findings.len(), 1);
        assert_eq!(
            lint_findings[0].diagnostic,
            LintDiagnostic::InvalidVariantPrefix
        );
        assert!(!plan.has_refs);
    }

    /// A generator that rejects the marker family reports the ref that failed.
    #[test]
    fn add_ref_reports_a_rejected_marker() {
        let context = IdentifierContext::new(
            GeneratorSet::new(Rc::new(RejectingGenerator)),
            IdentifierMode::Release,
            5,
        );
        let theme = ThemeIndex::default();
        let mut diagnostics = Vec::new();
        let mut lint_findings = Vec::new();
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
                &mut diagnostics,
                &mut lint_findings
            )
            .is_none()
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].labels[0].span, call_span);
        assert!(
            matches!(
                &diagnostics[0].diagnostic,
                CompileDiagnostic::IdentifierGenerationFailed { family, detail }
                    if family.as_ref() == "ref-marker"
                        && detail.as_ref() == "no marker for this ref"
            ),
            "{:?}",
            diagnostics[0].diagnostic
        );
    }
}
