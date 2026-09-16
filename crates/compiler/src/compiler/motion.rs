//! Animation and view-transition definitions, and the declarations their uses emit.
//!
//! A definition is emitted once per declaration under a name hashed from the declaration's
//! identity, and every use references that name, so a keyframe pair or view-transition pair is
//! written to the stylesheet once however many call sites use it. `animation` emits complete
//! conditional keyframe definitions. Frame utilities and view-transition buckets require
//! unprefixed utilities; conditions on animations belong to their definition branches.
//!
//! Options are resolved through the shared candidate resolver, which is why a theme duration
//! is usable as an animation option even though it is not a utility.

use stillwind_design_system::{
    ThemeIndex, UtilityOrder, VariantApplication, resolve_animation_option, resolve_variant,
};
use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_ir::{AssetReference, ViewTransitionDefinition};
use stillwind_source::{SourceFile, Spanned};
use stillwind_syntax::{Variant, parse_candidate};

use super::CssContribution;
use super::lexer::lex_utilities;
use super::symbols::{identity_declaration, named_declaration};
use super::utilities::declaration_order::combine_declarations_for_theme;
use super::utilities::resolve::{CompiledUtility, resolve_utility};
use super::variants::{VariantStep, conditions_only, variant_paths};
use crate::identifiers::IdentifierContext;

mod animation;

pub(super) use animation::emit_animation_definition;

/// Builds the declarations one animation use contributes.
///
/// The `animation-name` always names this declaration's keyframes, and the options follow in
/// their written order.
pub(super) fn resolve_animation_use(
    identifiers: &IdentifierContext,
    animation: &AssetReference,
    options: &Spanned<String>,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
) -> CompiledUtility {
    let mut declaration = format!(
        "animation-name:{};",
        match identifiers.animation_name(&identity_declaration(&animation.identity)) {
            Ok(name) => name.custom_ident(),
            Err(failure) => {
                diagnostics.push(CompileError::new(failure, options.span));
                String::new()
            }
        }
    );
    declaration.push_str(
        &resolve_motion_options(options, OptionVariants::Rejected, theme, diagnostics).declaration,
    );
    CompiledUtility {
        required_properties: Vec::new(),
        variants: Vec::new(),
        variant_applications: Vec::new(),
        declaration,
        selector: None,
        container: None,
        nested: Vec::new(),
        order: UtilityOrder::Authored(None),
        candidate: String::new(),
    }
}

/// How an option list treats the variants an option carries.
#[derive(Clone, Copy, Eq, PartialEq)]
enum OptionVariants {
    /// A variant makes the option unknown: an animation option describes a value.
    Rejected,
    /// Media and supports conditions wrap the option's own rule.
    Conditions,
}

/// Declarations one option list contributes, split by the conditions its options carry.
#[derive(Default)]
struct ResolvedMotionOptions {
    /// Declarations of the options that apply unconditionally, in written order.
    declaration: String,
    /// One entry per conditioned option, in written order. Always empty under
    /// [`OptionVariants::Rejected`].
    conditioned: Vec<ConditionedOption>,
}

/// Declarations one option contributes under the conditions it carries.
struct ConditionedOption {
    /// Condition paths this option applies under, outermost first.
    paths: Vec<Vec<OptionCondition>>,
    /// Declarations the option contributes.
    declaration: String,
}

/// Resolves a whitespace-separated option list, keeping the conditions its options carry.
///
/// Each option contributes its own declarations, so writing two durations is not rejected here
/// but resolves in source order. An option that cannot apply — text that is not candidate syntax,
/// a negative value, `!important`, or an unknown name — is reported as an unknown option at its
/// own span and contributes nothing. A variant that cannot condition a rule is reported as an
/// unknown option under [`OptionVariants::Rejected`] and as an invalid view-transition
/// variant under [`OptionVariants::Conditions`].
fn resolve_motion_options(
    options: &Spanned<String>,
    variants: OptionVariants,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
) -> ResolvedMotionOptions {
    let mut resolved = ResolvedMotionOptions::default();
    for option in lex_utilities(options, diagnostics) {
        let raw = option.value.raw.clone();
        let unknown = || {
            CompileError::new(
                CompileErrorKind::UnknownAnimationOption {
                    option: raw.clone(),
                },
                option.span,
            )
        };
        let Some(candidate) = parse_candidate(&raw) else {
            diagnostics.push(unknown());
            continue;
        };
        if candidate.important || candidate.negative {
            diagnostics.push(unknown());
            continue;
        }
        let Some(declaration) =
            resolve_animation_option(strip_variants(&raw, &candidate.variants), theme)
        else {
            diagnostics.push(unknown());
            continue;
        };
        if candidate.variants.is_empty() {
            resolved.declaration.push_str(&declaration);
            continue;
        }
        if variants == OptionVariants::Rejected {
            diagnostics.push(unknown());
            continue;
        }
        let Some(paths) = condition_paths(&candidate.variants, theme) else {
            diagnostics.push(CompileError::new(
                CompileErrorKind::InvalidViewTransitionVariant,
                option.span,
            ));
            continue;
        };
        resolved
            .conditioned
            .push(ConditionedOption { paths, declaration });
    }
    resolved
}

/// The option text without its variant prefixes.
///
/// A parsed candidate keeps each prefix's own spelling, so dropping them in order leaves the text
/// the option registries resolve.
fn strip_variants<'a>(raw: &'a str, variants: &[Variant]) -> &'a str {
    let prefixes: usize = variants.iter().map(|variant| variant.raw.len() + 1).sum();
    &raw[prefixes..]
}

/// Writes the view-transition rules for one declaration.
///
/// The group rule carries the transition options, and the `old`, `new`, and `image-pair`
/// pseudo-element rules carry their utility lists. Every pseudo-element is keyed by the same
/// generated name, which is what pairs them at runtime.
pub(super) fn emit_view_transition_definition(
    contribution: &mut CssContribution,
    source: &SourceFile,
    identifiers: &IdentifierContext,
    transition: &ViewTransitionDefinition,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
) {
    let output = &mut contribution.css;
    let class = match identifiers
        .view_transition_class(&named_declaration(source.path(), &transition.name))
    {
        Ok(class) => class,
        Err(failure) => {
            diagnostics.push(CompileError::new(failure, transition.call_span));
            return;
        }
    };
    let class = class.selector();
    if let Some(group) = &transition.group {
        let selector = format!("::view-transition-group({class})");
        let resolved =
            resolve_motion_options(group, OptionVariants::Conditions, theme, diagnostics);
        emit_option_rules(output, &selector, &resolved);
    }
    for (pseudo, utilities) in [
        ("old", transition.old.as_ref()),
        ("new", transition.new.as_ref()),
        ("image-pair", transition.image_pair.as_ref()),
    ] {
        let Some(utilities) = utilities else {
            continue;
        };
        emit_view_transition_utilities(
            output,
            &format!("::view-transition-{pseudo}({class})"),
            utilities,
            theme,
            diagnostics,
            &mut contribution.required_properties,
        );
    }
}

/// Writes one pseudo-element phase's rules for `selector`.
///
/// Utilities without variants share one rule. A utility carrying condition variants emits its
/// own rule per condition path, in source order after the shared rule, so a condition selects
/// its declarations without changing what the unconditional rule says. Rejected tokens
/// contribute nothing; successful tokens still append their property requirements, so callers
/// must check diagnostics before publishing.
fn emit_view_transition_utilities(
    output: &mut String,
    selector: &str,
    input: &Spanned<String>,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
    required_properties: &mut Vec<String>,
) {
    let mut plain = Vec::new();
    let mut conditional = String::new();
    for utility in lex_utilities(input, diagnostics) {
        let Some(resolved) = resolve_utility(&utility.value.raw, theme) else {
            diagnostics.push(CompileError::new(
                CompileErrorKind::UnavailableUtility {
                    utility: utility.value.raw.clone(),
                },
                utility.span,
            ));
            continue;
        };
        if resolved.variants.is_empty() {
            plain.push(resolved);
            continue;
        }
        let Some(paths) = condition_paths(&resolved.variants, theme) else {
            diagnostics.push(CompileError::new(
                CompileErrorKind::InvalidViewTransitionVariant,
                utility.span,
            ));
            continue;
        };
        required_properties.extend(resolved.required_properties.clone());
        let declaration = combine_declarations_for_theme(&[&resolved], theme);
        for path in paths {
            let rule = format!("{selector}{{ {declaration}}}");
            conditional.push_str(&wrap_conditions(&rule, &path));
        }
    }
    if !plain.is_empty() {
        required_properties.extend(
            plain
                .iter()
                .flat_map(|utility| utility.required_properties.clone()),
        );
        output.push_str(selector);
        output.push_str("{ ");
        output.push_str(&combine_declarations_for_theme(
            &plain.iter().collect::<Vec<_>>(),
            theme,
        ));
        output.push('}');
    }
    output.push_str(&conditional);
}

/// Writes one rule for the options that apply unconditionally, then one rule per conditioned
/// option.
///
/// Every rule keeps the same selector, so a condition changes only the properties its own options
/// state: the conditioned rule follows the unconditional one and wins for the properties both
/// declare. `view-transition-group` carries no element state the condition could select through,
/// so the condition wraps the rule rather than the selector.
fn emit_option_rules(output: &mut String, selector: &str, options: &ResolvedMotionOptions) {
    if !options.declaration.is_empty() {
        output.push_str(selector);
        output.push_str("{ ");
        output.push_str(&options.declaration);
        output.push('}');
    }
    for option in &options.conditioned {
        let rule = format!("{selector}{{ {}}}", option.declaration);
        for path in &option.paths {
            output.push_str(&wrap_conditions(&rule, path));
        }
    }
}

/// One condition wrapper a rule is nested under.
#[derive(Debug)]
enum OptionCondition {
    /// `@media <condition>`
    Media(String),
    /// `@supports (<condition>)`
    Supports(String),
}

impl OptionCondition {
    /// The wrapper one resolved variant step produces.
    ///
    /// A step that cannot wrap a rule is not a condition, so the caller drops the utility rather
    /// than emitting it unconditionally.
    fn from_step(step: &VariantStep<'_>) -> Option<Self> {
        match step {
            VariantStep::Media(condition) => Some(Self::Media((*condition).to_owned())),
            VariantStep::Supports(condition) => Some(Self::Supports((*condition).to_owned())),
            _ => None,
        }
    }

    /// Wraps one rule in this condition.
    fn wrap(&self, rule: &str) -> String {
        match self {
            Self::Media(condition) => format!("@media {condition}{{{rule}}}"),
            Self::Supports(condition) => format!("@supports ({condition}){{{rule}}}"),
        }
    }
}

/// Wraps one rule in a condition path, outermost condition first.
fn wrap_conditions(rule: &str, path: &[OptionCondition]) -> String {
    path.iter()
        .rev()
        .fold(rule.to_owned(), |wrapped, condition| {
            condition.wrap(&wrapped)
        })
}

/// The condition paths one variant list produces, or `None` when a variant cannot wrap a rule.
///
/// A view-transition pseudo-element has no element state of its own and no container ancestor,
/// so only media and supports conditions can wrap it. Any other variant fails the utility rather
/// than dropping its condition silently.
fn condition_paths(variants: &[Variant], theme: &ThemeIndex) -> Option<Vec<Vec<OptionCondition>>> {
    let applications = condition_applications(variants, theme)?;
    variant_paths(&applications)
        .iter()
        .map(|path| path.iter().map(OptionCondition::from_step).collect())
        .collect()
}

/// Resolves a bucket utility's variants into the applications a pseudo-element rule can carry.
fn condition_applications(
    variants: &[Variant],
    theme: &ThemeIndex,
) -> Option<Vec<VariantApplication>> {
    let mut applications = Vec::new();
    for variant in variants {
        match resolve_variant(variant, theme)? {
            VariantApplication::Compound(children) => applications.extend(children),
            application => applications.push(application),
        }
    }
    conditions_only(&variant_paths(&applications)).then_some(applications)
}

#[cfg(test)]
mod tests {
    use stillwind_js_oxc::OxcSourceAnalyzer;
    use stillwind_source::{FileId, SourceFile, Span};

    use super::*;
    use crate::Compiler;

    #[test]
    fn resolves_themed_duration_as_an_animation_option_only() {
        let theme: SourceFile = crate::css_fixture::source(
            FileId(1),
            "stillwind.css",
            "@theme { --duration-normal: 180ms; }",
        )
        .unwrap();
        let theme = Compiler::read_theme(&theme).unwrap();
        let options = Spanned::new(
            "duration-normal both".to_owned(),
            Span::new(FileId(2), 0, 20),
        );
        let mut diagnostics = Vec::new();

        assert_eq!(
            resolve_motion_options(&options, OptionVariants::Rejected, &theme, &mut diagnostics)
                .declaration,
            "animation-duration:var(--duration-normal);animation-fill-mode:both;"
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn splits_a_conditioned_option_from_the_unconditional_ones() {
        let theme = crate::css_fixture::source(FileId(2), "theme", "").unwrap();
        let theme = Compiler::read_theme(&theme).unwrap();
        let options = Spanned::new(
            "duration-180 motion-reduce:duration-0".to_owned(),
            Span::new(FileId(2), 0, 34),
        );
        let mut diagnostics = Vec::new();

        let resolved = resolve_motion_options(
            &options,
            OptionVariants::Conditions,
            &theme,
            &mut diagnostics,
        );

        assert_eq!(resolved.declaration, "animation-duration:calc(180 * 1ms);");
        assert_eq!(resolved.conditioned.len(), 1);
        assert_eq!(
            resolved.conditioned[0].declaration,
            "animation-duration:calc(0 * 1ms);"
        );
        let [path] = resolved.conditioned[0].paths.as_slice() else {
            panic!("one condition path: {:?}", resolved.conditioned[0].paths);
        };
        assert_eq!(
            wrap_conditions("rule", path),
            "@media (prefers-reduced-motion: reduce){rule}"
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn rejects_a_variant_on_an_animation_option() {
        let theme = crate::css_fixture::source(FileId(2), "theme", "").unwrap();
        let theme = Compiler::read_theme(&theme).unwrap();
        let options = Spanned::new(
            "motion-reduce:duration-0".to_owned(),
            Span::new(FileId(2), 0, 24),
        );
        let mut diagnostics = Vec::new();

        let resolved =
            resolve_motion_options(&options, OptionVariants::Rejected, &theme, &mut diagnostics);

        assert!(resolved.declaration.is_empty() && resolved.conditioned.is_empty());
        assert_eq!(
            diagnostics
                .iter()
                .map(|error| error.kind.clone())
                .collect::<Vec<_>>(),
            vec![CompileErrorKind::UnknownAnimationOption {
                option: "motion-reduce:duration-0".to_owned(),
            }]
        );
    }

    #[test]
    fn strips_every_variant_prefix_from_an_option() {
        for (option, expected) in [
            ("duration-180", "duration-180"),
            ("motion-reduce:duration-0", "duration-0"),
            (
                "md:motion-reduce:[transition-duration:75ms]",
                "[transition-duration:75ms]",
            ),
        ] {
            let candidate = parse_candidate(option).expect("a candidate");
            assert_eq!(strip_variants(option, &candidate.variants), expected);
        }
    }

    #[test]
    fn motion_variants_report_the_exact_source_span_through_compilation() {
        for (definition, kind, expected) in [
            (
                "style.animation({ from: 'hover:opacity-0' })",
                CompileErrorKind::AnimationFrameVariants,
                "hover:opacity-0",
            ),
            (
                "style.viewTransition({ old: 'hover:opacity-0' })",
                CompileErrorKind::InvalidViewTransitionVariant,
                "hover:opacity-0",
            ),
            (
                "style.viewTransition({ imagePair: 'focus:opacity-50' })",
                CompileErrorKind::InvalidViewTransitionVariant,
                "focus:opacity-50",
            ),
            (
                "style.viewTransition({ group: 'hover:duration-180' })",
                CompileErrorKind::InvalidViewTransitionVariant,
                "hover:duration-180",
            ),
            (
                "style.viewTransition({ group: 'motion-reduce:nope-0' })",
                CompileErrorKind::UnknownAnimationOption {
                    option: "motion-reduce:nope-0".to_owned(),
                },
                "motion-reduce:nope-0",
            ),
        ] {
            let source = SourceFile::new(
                FileId(1),
                "motion.tsx",
                format!("import {{ style }} from 'stillwind/react'; const motion = {definition};"),
            )
            .unwrap();
            let theme = crate::css_fixture::source(FileId(2), "theme", "").unwrap();
            let result = Compiler::compile_source(&OxcSourceAnalyzer, &source, &theme);
            let errors = result
                .diagnostics
                .iter()
                .filter(|error| error.kind == kind)
                .collect::<Vec<_>>();
            assert_eq!(errors.len(), 1, "{:?}", result.diagnostics);
            assert_eq!(source.slice(errors[0].labels[0].span).unwrap(), expected);
        }
    }
}
