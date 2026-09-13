//! Structure-preserving style emission.
//!
//! The conditional and array structure of every class expression is kept: each
//! user expression appears once in the output, is evaluated once, and keeps its
//! source position. Class contributions are emitted either as a plain class
//! value (Tier 1) or as a fragment (Tier 2); the renderer picks the shape.
//!
//! The stylesheet order is decided here and not by the JavaScript shape: the base
//! unit's rules are written to the stylesheet before every conditional unit's rules,
//! so the cascade result never depends on the class attribute order.

use std::collections::BTreeSet;

use stillwind_design_system::{
    ThemeIndex, compatibility_marker_class, is_tailwind_relationship_candidate,
};
use stillwind_ir::{
    AttributeEntry, ClassExpression, ConditionalKind, GroupEntry, StyleCall, StyleInput,
    StyleVariant,
};
use stillwind_semantic::IdentityId;
use stillwind_source::{CompileError, LintFinding, SourceFile, Span};

use super::handles::{
    handle_marker_class, ref_identity, render_handle_projection, scope_identity, scope_marker_class,
};
use super::lexer::lex_utilities;
use super::motion::resolve_animation_use;
use super::props::{
    Emitted, EmittedEntry, GeneratedClass, GuardStep, StyleEmission, StyleEmissionPart,
};
use super::style_variants::StyleVariantPlan;
use super::symbols::{
    class_name, generated_class_symbol, handle_projection_name, motion_symbol_name,
    ref_symbol_name, runtime_projection_class,
};
use super::utilities::emission::emit_utility_rules;
use super::utilities::resolve::{
    CompiledUtility, resolve_utility_targets, unresolved_utility_rule, view_transition_utility,
};
use crate::runtime::{RuntimeBindings, RuntimeHelper};
use crate::{CompilerOptions, GeneratedSymbol};

/// One `style()` call being lowered.
///
/// The emitter accumulates state rather than building a tree in one pass, because the base unit
/// has to be collected across argument positions while each conditional leaf needs its own class
/// unit and guard path.
pub(super) struct CallEmitter<'a, 'b> {
    /// The buffer the module's original text is sliced from.
    source: &'a SourceFile,
    options: &'a CompilerOptions,
    theme: &'a ThemeIndex,
    call: &'a StyleCall,
    class_property: &'static str,
    diagnostics: &'a mut Vec<CompileError>,
    lint_findings: &'a mut Vec<LintFinding>,
    utility_css: &'a mut String,
    /// Leaf rules wait here so the base unit stays first in the stylesheet.
    runtime_css: String,
    symbols: &'a mut Vec<GeneratedSymbol>,
    /// Class units already generated in this compilation, keyed by name and canonical input, so
    /// two identical units are emitted once while a collision would still be visible.
    emitted_utility_classes: &'a mut BTreeSet<(String, String)>,
    /// Identities the module references at runtime; the pipeline uses this set to decide which
    /// declarations are replaced by a runtime call and which are erased.
    runtime_assets: &'a mut BTreeSet<IdentityId>,
    motion_assets: &'a mut BTreeSet<IdentityId>,
    runtime: &'a mut RuntimeBindings<'b>,
    /// Canonical candidate and asset spelling for the base unit, in source order.
    static_canonical: String,
    /// Utilities that will form the base unit's rule.
    static_utilities: Vec<CompiledUtility>,
    /// Class names that are always applied: markers, external names, and the base unit.
    static_names: Vec<String>,
    parts: Vec<StyleEmissionPart>,
    classes: Vec<GeneratedClass>,
    /// Emission-local index of the next leaf, making each branch its own class identity.
    class_index: u32,
    /// Set when a props object sits inside a conditional expression, which forces Tier 2.
    conditional_props: bool,
    /// Set once the call contributed a view transition, so a second one is reported.
    view_transition_seen: bool,
}

/// The borrows one call needs, grouped so the emitter can be constructed in one place.
pub(super) struct CallEmitterInputs<'a, 'b> {
    pub source: &'a SourceFile,
    pub options: &'a CompilerOptions,
    pub theme: &'a ThemeIndex,
    pub call: &'a StyleCall,
    pub class_property: &'static str,
    pub diagnostics: &'a mut Vec<CompileError>,
    pub lint_findings: &'a mut Vec<LintFinding>,
    pub utility_css: &'a mut String,
    pub symbols: &'a mut Vec<GeneratedSymbol>,
    pub emitted_utility_classes: &'a mut BTreeSet<(String, String)>,
    pub runtime_assets: &'a mut BTreeSet<IdentityId>,
    /// Motion definitions reached by static or dynamic uses.
    pub motion_assets: &'a mut BTreeSet<IdentityId>,
    pub runtime: &'a mut RuntimeBindings<'b>,
}

impl<'a, 'b> CallEmitter<'a, 'b> {
    /// Lowers one call, or returns `None` after pushing a diagnostic.
    #[allow(clippy::needless_pass_by_value)] // The struct owns the borrows this call consumes.
    pub(super) fn run(inputs: CallEmitterInputs<'a, 'b>) -> Option<StyleEmission> {
        let emitter = Self {
            source: inputs.source,
            options: inputs.options,
            theme: inputs.theme,
            call: inputs.call,
            class_property: inputs.class_property,
            diagnostics: inputs.diagnostics,
            lint_findings: inputs.lint_findings,
            utility_css: inputs.utility_css,
            runtime_css: String::new(),
            symbols: inputs.symbols,
            emitted_utility_classes: inputs.emitted_utility_classes,
            runtime_assets: inputs.runtime_assets,
            motion_assets: inputs.motion_assets,
            runtime: inputs.runtime,
            static_canonical: String::new(),
            static_utilities: Vec::new(),
            static_names: Vec::new(),
            parts: Vec::new(),
            classes: Vec::new(),
            class_index: 0,
            conditional_props: false,
            view_transition_seen: false,
        };
        emitter.emit()
    }

    /// Walks the call's inputs in source order and finishes the emission.
    fn emit(mut self) -> Option<StyleEmission> {
        for input in self.call.inputs.clone() {
            self.emit_input(&input)?;
        }
        Some(self.finish())
    }

    /// Dispatches one style input.
    ///
    /// Inputs that carry no user expressions fold into the base unit; everything else becomes a
    /// part whose source position decides where it sits in the emitted expression.
    #[allow(clippy::too_many_lines)] // One exhaustive match over the style inputs.
    fn emit_input(&mut self, input: &StyleInput) -> Option<()> {
        match input {
            StyleInput::Class(ClassExpression::Group { entries, .. }) => {
                for entry in entries {
                    match entry {
                        GroupEntry::Attribute(attribute) => {
                            let text = self.render_attribute(attribute)?;
                            self.parts.push(StyleEmissionPart::Attributes {
                                text,
                                position: attribute.value.start.0,
                            });
                        }
                        GroupEntry::Class(expression) => {
                            let position = expression_position(expression);
                            self.emit_item(expression, position);
                        }
                    }
                }
            }
            StyleInput::Class(expression) => {
                let position = expression_position(expression);
                self.emit_item(expression, position);
            }
            StyleInput::Composition { value, .. } => {
                let text = self.slice(*value)?;
                self.parts.push(StyleEmissionPart::Base {
                    text,
                    position: value.start.0,
                });
            }
            StyleInput::ScopeAttachment(scope) => {
                let marker = scope_marker_class(scope).trim_start_matches('.').to_owned();
                if !self.static_names.contains(&marker) {
                    self.static_names.push(marker);
                }
                self.static_canonical.push_str("scope:");
                self.static_canonical.push_str(&scope.name.value);
                self.static_canonical.push('\u{0}');
            }
            StyleInput::HandleAttachment(handle) => {
                let marker = handle_marker_class(handle);
                if !self.static_names.contains(&marker) {
                    self.static_names.push(marker);
                }
                self.static_canonical.push_str("ref:");
                self.static_canonical.push_str(&handle.name.value);
                self.static_canonical.push('\u{0}');
            }
            StyleInput::DynamicScopeAttachment { value, scopes } => {
                self.runtime_assets
                    .extend(scopes.iter().map(scope_identity));
                let helper = self.runtime.require(RuntimeHelper::HandleClass, *value);
                let text = format!("{helper}({})", self.slice(*value)?);
                self.push_expression(text, value.start.0);
            }
            StyleInput::DynamicHandleAttachment { value, handles } => {
                self.runtime_assets.extend(handles.iter().map(ref_identity));
                let helper = self.runtime.require(RuntimeHelper::HandleClass, *value);
                let text = format!("{helper}({})", self.slice(*value)?);
                self.push_expression(text, value.start.0);
            }
            StyleInput::ContainerQuery {
                handle,
                query,
                value,
            } => {
                self.static_canonical.push_str("container:");
                self.static_canonical.push_str(&ref_symbol_name(handle));
                self.static_canonical.push(':');
                self.static_canonical.push_str(&query.value);
                self.static_canonical.push('\u{0}');
                for utility in lex_utilities(value, self.diagnostics) {
                    self.static_canonical.push_str(&utility.value.raw);
                    self.static_canonical.push('\u{0}');
                    if let Some(mut utilities) =
                        resolve_utility_targets(&utility.value.raw, self.theme)
                    {
                        for resolved in &mut utilities {
                            resolved.container =
                                Some(format!("{} ({})", ref_symbol_name(handle), query.value));
                        }
                        self.static_utilities.append(&mut utilities);
                    } else {
                        self.lint_findings.push(LintFinding::new(
                            unresolved_utility_rule(&utility.value.raw),
                            format!("unknown or unavailable utility `{}`", utility.value.raw),
                            utility.span,
                        ));
                        self.static_names.push(utility.value.raw.clone());
                    }
                }
            }
            StyleInput::ViewTransitionUse { transition, key } => {
                self.motion_assets.insert(transition.identity.clone());
                if !self.begin_view_transition() {
                    return Some(());
                }
                let name = motion_symbol_name(&transition.identity);
                self.emit_view_transition_class();
                let key = match key {
                    Some(key) => self.slice(*key)?,
                    None => "undefined".to_owned(),
                };
                let helper = self
                    .runtime
                    .require(RuntimeHelper::ViewTransitionStyle, self.call.span);
                let text = format!(
                    "{helper}(\"{name}\",{key},[\"{name}\"],\"--sw-vt-name\",\"--sw-vt-class\")"
                );
                self.parts.push(StyleEmissionPart::Style {
                    text,
                    position: self.call.span.start.0,
                });
            }
            StyleInput::DynamicViewTransitionUse {
                value,
                transitions,
                key,
            } => {
                if !self.begin_view_transition() {
                    return Some(());
                }
                let mut candidates = Vec::new();
                for transition in transitions {
                    self.runtime_assets.insert(transition.identity.clone());
                    candidates.push(motion_symbol_name(&transition.identity));
                }
                self.emit_view_transition_class();
                let key = match key {
                    Some(key) => self.slice(*key)?,
                    None => "undefined".to_owned(),
                };
                let candidates = candidates
                    .iter()
                    .map(|candidate| format!("\"{candidate}\""))
                    .collect::<Vec<_>>()
                    .join(",");
                let helper = self
                    .runtime
                    .require(RuntimeHelper::ViewTransitionStyle, self.call.span);
                let text = format!(
                    "{helper}({},{key},[{candidates}],\"--sw-vt-name\",\"--sw-vt-class\")",
                    self.slice(*value)?
                );
                self.parts.push(StyleEmissionPart::Style {
                    text,
                    position: value.start.0,
                });
            }
        }
        Some(())
    }

    /// Reports the second view transition of one call, matching the
    /// pre-existing diagnostic.
    ///
    /// A call can contribute only one view-transition pair, because the pair shares one generated
    /// name and a second use would overwrite the first.
    fn begin_view_transition(&mut self) -> bool {
        if self.view_transition_seen {
            self.diagnostics.push(CompileError::new(
                "SHVIEW004",
                "style() accepts at most one view transition",
                self.call.span,
            ));
            return false;
        }
        self.view_transition_seen = true;
        true
    }

    /// Adds a class value that is a helper call over user expressions.
    fn push_expression(&mut self, text: String, position: u32) {
        self.parts.push(StyleEmissionPart::Class {
            emitted: Emitted::Expression(text),
            position,
            user_code: true,
            user_value: true,
        });
    }

    /// Lowers one top-level class item of a call or of an attribute object.
    fn emit_item(&mut self, expression: &ClassExpression, position: u32) {
        let mut wrappers = Vec::new();
        let mut guard = Vec::new();
        if let Some(emitted) = self.emit_expression(expression, &mut guard, &mut wrappers, true) {
            self.parts.push(StyleEmissionPart::Class {
                emitted,
                position,
                user_code: uses_user_code(expression),
                user_value: is_user_value(expression),
            });
        }
    }

    /// Lowers one class expression, tracking the guard path and the active wrappers.
    ///
    /// `fold` is set at the top level of a call: a leaf reached without a guard can join the base
    /// unit, while a leaf under any test keeps its own class unit. Conditional arms clear the flag
    /// for their contents, so a nested leaf never merges into the base unit by accident.
    #[allow(clippy::too_many_lines)] // One exhaustive match over the class expression.
    fn emit_expression(
        &mut self,
        expression: &ClassExpression,
        guard: &mut Vec<GuardStep>,
        wrappers: &mut Vec<StyleVariant>,
        fold: bool,
    ) -> Option<Emitted> {
        match expression {
            ClassExpression::Utilities(value) => {
                let plan = StyleVariantPlan::resolve(wrappers, self.theme, self.diagnostics)?;
                let contribution = self.resolve_literal(value, &plan);
                if fold && guard.is_empty() {
                    self.fold(contribution);
                    return None;
                }
                Some(self.leaf(contribution, guard))
            }
            ClassExpression::Array(elements) => {
                let mut items = Vec::new();
                let mut any = false;
                for element in elements {
                    let item = self.emit_expression(element, guard, wrappers, fold);
                    any |= item.is_some();
                    items.push(item);
                }
                any.then_some(Emitted::Array(items))
            }
            ClassExpression::Conditional {
                test,
                kind,
                consequent,
                alternate,
            } => {
                let test_span = *test;
                guard.push(GuardStep {
                    test: test_span,
                    when_true: true,
                });
                let consequent = self.emit_sequence(consequent, guard, wrappers);
                guard.pop();
                let test = self.slice(test_span)?;
                match kind {
                    ConditionalKind::LogicalAnd => Some(Emitted::Conditional {
                        test,
                        kind: *kind,
                        consequent: Some(Box::new(consequent?)),
                        alternate: None,
                    }),
                    ConditionalKind::Ternary => {
                        guard.push(GuardStep {
                            test: test_span,
                            when_true: false,
                        });
                        let alternate = self.emit_sequence(alternate, guard, wrappers);
                        guard.pop();
                        if consequent.is_none() && alternate.is_none() {
                            return None;
                        }
                        Some(Emitted::Conditional {
                            test,
                            kind: *kind,
                            consequent: consequent.map(Box::new),
                            alternate: alternate.map(Box::new),
                        })
                    }
                }
            }
            ClassExpression::Variant { variant, inputs } => {
                wrappers.push(variant.clone());
                let mut items = Vec::new();
                let mut any = false;
                for input in inputs {
                    let item = self.emit_expression(input, guard, wrappers, fold);
                    any |= item.is_some();
                    items.push(item);
                }
                wrappers.pop();
                if !any {
                    return None;
                }
                if items.len() == 1 {
                    items.remove(0)
                } else {
                    Some(Emitted::Array(items))
                }
            }
            ClassExpression::Group { entries, .. } => {
                let mut items = Vec::new();
                let mut any = false;
                for entry in entries {
                    let item = match entry {
                        GroupEntry::Attribute(attribute) => {
                            let text = self.render_attribute(attribute)?;
                            self.conditional_props = true;
                            Some(EmittedEntry::Attribute(text))
                        }
                        GroupEntry::Class(expression) => self
                            .emit_expression(expression, guard, wrappers, fold)
                            .map(EmittedEntry::Class),
                    };
                    any |= item.is_some();
                    items.push(item);
                }
                any.then_some(Emitted::Group(items))
            }
            ClassExpression::External(value) => {
                let text = self.slice(*value)?;
                Some(Emitted::Expression(text))
            }
            ClassExpression::HandleProjection { handle, projection } => {
                let name = handle_projection_name(projection);
                let utility = CompiledUtility {
                    required_properties: Vec::new(),
                    variants: Vec::new(),
                    variant_applications: Vec::new(),
                    declaration: render_handle_projection(handle, projection),
                    selector: None,
                    container: None,
                    nested: Vec::new(),
                    builtin_order: None,
                    candidate: String::new(),
                };
                let marker = handle_marker_class(handle);
                if fold && guard.is_empty() {
                    self.static_canonical.push_str("handle:");
                    self.static_canonical.push_str(&ref_symbol_name(handle));
                    self.static_canonical.push(':');
                    self.static_canonical.push_str(name);
                    self.static_canonical.push('\u{0}');
                    self.static_utilities.push(utility);
                    if !self.static_names.contains(&marker) {
                        self.static_names.push(marker);
                    }
                    return None;
                }
                let contribution = LiteralContribution {
                    canonical: format!("handle:{}:{name}\u{0}", ref_symbol_name(handle)),
                    utilities: vec![utility],
                    markers: Vec::new(),
                    external: Vec::new(),
                    plain: vec![marker],
                };
                Some(self.leaf(contribution, guard))
            }
            ClassExpression::DynamicHandleProjection {
                value,
                handles,
                projection,
            } => {
                let mut table = Vec::new();
                for handle in handles {
                    self.runtime_assets.insert(ref_identity(handle));
                    let marker = runtime_projection_class(handle, projection);
                    let canonical = format!(
                        "dynamic-handle:{}:{}",
                        ref_symbol_name(handle),
                        handle_projection_name(projection)
                    );
                    let contribution = LiteralContribution {
                        canonical,
                        utilities: vec![CompiledUtility {
                            required_properties: Vec::new(),
                            variants: Vec::new(),
                            variant_applications: Vec::new(),
                            declaration: render_handle_projection(handle, projection),
                            selector: None,
                            container: None,
                            nested: Vec::new(),
                            builtin_order: None,
                            candidate: String::new(),
                        }],
                        markers: Vec::new(),
                        external: Vec::new(),
                        plain: Vec::new(),
                    };
                    let class = self.leaf(contribution, guard);
                    let class = render_plain(&class)?;
                    table.push(format!("\"{marker}\":{class}"));
                }
                let table = table.join(",");
                let helper = self.runtime.require(RuntimeHelper::HandleClass, *value);
                let text = format!("{helper}({},{table})", self.slice(*value)?);
                Some(Emitted::Expression(text))
            }
            ClassExpression::AnimationUse { animation, options } => {
                self.motion_assets.insert(animation.identity.clone());
                let plan = StyleVariantPlan::resolve(wrappers, self.theme, self.diagnostics)?;
                let mut utility =
                    resolve_animation_use(animation, options, self.theme, self.diagnostics);
                plan.apply_animation(&mut utility);
                let canonical = format!(
                    "{}{}:{}:{}\u{0}",
                    plan.canonical, "animation:", animation.identity.binding_name, options.value
                );
                let contribution = LiteralContribution {
                    canonical,
                    utilities: vec![utility],
                    markers: Vec::new(),
                    external: Vec::new(),
                    plain: Vec::new(),
                };
                if fold && guard.is_empty() {
                    self.fold(contribution);
                    return None;
                }
                Some(self.leaf(contribution, guard))
            }
            ClassExpression::DynamicAnimationUse {
                value,
                animations,
                options,
            } => {
                let plan = StyleVariantPlan::resolve(wrappers, self.theme, self.diagnostics)?;
                let mut table = Vec::new();
                for animation in animations {
                    self.runtime_assets.insert(animation.identity.clone());
                    let identity = motion_symbol_name(&animation.identity);
                    let mut utility =
                        resolve_animation_use(animation, options, self.theme, self.diagnostics);
                    plan.apply_animation(&mut utility);
                    let canonical = if plan.canonical.is_empty() {
                        format!("dynamic-animation:{identity}:{}", options.value)
                    } else {
                        format!(
                            "{}\u{0}dynamic-animation:{identity}:{}",
                            plan.canonical, options.value
                        )
                    };
                    let contribution = LiteralContribution {
                        canonical,
                        utilities: vec![utility],
                        markers: Vec::new(),
                        external: Vec::new(),
                        plain: Vec::new(),
                    };
                    let class = self.leaf(contribution, guard);
                    let class = render_plain(&class)?;
                    table.push(format!("\"{identity}\":{class}"));
                }
                let table = table.join(",");
                let helper = self.runtime.require(RuntimeHelper::AssetClass, *value);
                let text = format!("{helper}({},{table})", self.slice(*value)?);
                Some(Emitted::Expression(text))
            }
        }
    }

    /// Lowers a whole branch of a conditional expression.
    ///
    /// Every element must be emittable: a branch is a single value, so dropping one element would
    /// silently change what the test selects.
    fn emit_sequence(
        &mut self,
        expressions: &[ClassExpression],
        guard: &mut Vec<GuardStep>,
        wrappers: &mut Vec<StyleVariant>,
    ) -> Option<Emitted> {
        let mut items = Vec::new();
        for expression in expressions {
            items.push(self.emit_expression(expression, guard, wrappers, false)?);
        }
        match items.len() {
            0 => None,
            1 => items.pop(),
            _ => Some(Emitted::Array(items.into_iter().map(Some).collect())),
        }
    }

    /// Lowers one utility literal into its canonical input, resolved utilities, and classes.
    ///
    /// A compatibility marker is emitted instead of utilities when the candidate only marks a
    /// Tailwind relationship, and an unresolved candidate becomes an external class name plus a
    /// lint finding, which keeps the user's class in the output rather than dropping it.
    fn resolve_literal(
        &mut self,
        value: &stillwind_source::Spanned<String>,
        plan: &StyleVariantPlan,
    ) -> LiteralContribution {
        let mut contribution = LiteralContribution::default();
        for utility in lex_utilities(value, self.diagnostics) {
            let canonical_candidate = format!("{}{}", plan.canonical, utility.value.raw);
            let mut raw = utility.value.raw.clone();
            raw.insert_str(0, &plan.prefix);
            if (self.options.forbid_tailwind_relationships
                || self.theme.config.lint_enabled("prefer-typed-relationships"))
                && is_tailwind_relationship_candidate(&raw, self.theme)
            {
                self.lint_findings.push(LintFinding::new(
                    "prefer-typed-relationships",
                    format!(
                        "Tailwind relationship utility `{raw}` is forbidden; use style.ref() or style.scope()"
                    ),
                    utility.span,
                ));
            }
            if !plan.has_refs
                && let Some(marker) = compatibility_marker_class(&raw, self.theme)
            {
                contribution.markers.push(marker);
                continue;
            }
            contribution.canonical.push_str(&canonical_candidate);
            contribution.canonical.push('\u{0}');
            let resolved_input = raw.strip_prefix("tw:").unwrap_or(&raw);
            if let Some(mut resolved) = resolve_utility_targets(resolved_input, self.theme) {
                for resolved_utility in &mut resolved {
                    plan.apply(resolved_utility, self.theme);
                }
                contribution.utilities.append(&mut resolved);
            } else {
                self.lint_findings.push(LintFinding::new(
                    unresolved_utility_rule(&utility.value.raw),
                    format!("unknown or unavailable utility `{}`", utility.value.raw),
                    utility.span,
                ));
                contribution.external.push(utility.value.raw.clone());
            }
        }
        contribution
    }

    /// Merges a contribution into the unconditional base unit.
    fn fold(&mut self, contribution: LiteralContribution) {
        self.static_canonical.push_str(&contribution.canonical);
        self.static_utilities.extend(contribution.utilities);
        self.static_names.extend(contribution.markers);
        self.static_names.extend(contribution.external);
        self.static_names.extend(contribution.plain);
    }

    /// Turns one contribution into a leaf expression, generating its class unit.
    ///
    /// The branch index is part of the class identity, so identical utilities in two branches
    /// still produce distinct units and the later unit can override the earlier one.
    fn leaf(&mut self, contribution: LiteralContribution, guard: &[GuardStep]) -> Emitted {
        let mut names = Vec::new();
        if !contribution.utilities.is_empty() {
            self.class_index += 1;
            let canonical = format!("{}:{}", self.class_index, contribution.canonical);
            let canonical = self.options.style_class_input(
                self.source.path(),
                self.call.span.start.0,
                &canonical,
            );
            let name = class_name(&canonical);
            if self
                .emitted_utility_classes
                .insert((name.clone(), canonical.clone()))
            {
                emit_utility_rules(
                    &mut self.runtime_css,
                    &name,
                    &contribution.utilities,
                    self.theme,
                );
                self.symbols.push(generated_class_symbol(
                    self.source,
                    &name,
                    &canonical,
                    self.call.span,
                ));
            }
            names.push(name.clone());
            self.classes.push(GeneratedClass {
                class_name: name,
                call_span: self.call.span,
                guard_path: guard.to_vec(),
            });
        }
        names.extend(contribution.markers);
        names.extend(contribution.external);
        names.extend(contribution.plain);
        Emitted::Leaf { names }
    }

    /// Renders one attribute entry of an object literal, with its value verbatim.
    fn render_attribute(&mut self, attribute: &AttributeEntry) -> Option<String> {
        let value = self.slice(attribute.value)?;
        Some(format!("\"{}\":{value}", attribute.name.value))
    }

    /// Contributes the class that carries a view transition's name and class properties.
    fn emit_view_transition_class(&mut self) {
        self.static_canonical.push_str("view-transition");
        self.static_canonical.push('\u{0}');
        self.static_utilities.push(view_transition_utility());
    }

    /// Slices an original span, or reports that it cannot be read.
    fn slice(&self, span: Span) -> Option<String> {
        self.source.slice(span).ok().map(str::to_owned)
    }

    /// Emits the base unit and returns the parts, in source order.
    //
    // The base unit's rules are pushed to the shared stylesheet now, after every leaf rule was
    // collected in `runtime_css`. That ordering is what keeps a conditional unit's rule after the
    // base rule it overrides.
    fn finish(mut self) -> StyleEmission {
        let mut names = Vec::new();
        if !self.static_utilities.is_empty() {
            let canonical = format!("0:{}", self.static_canonical);
            let canonical = self.options.style_class_input(
                self.source.path(),
                self.call.span.start.0,
                &canonical,
            );
            let name = class_name(&canonical);
            if self
                .emitted_utility_classes
                .insert((name.clone(), canonical.clone()))
            {
                emit_utility_rules(self.utility_css, &name, &self.static_utilities, self.theme);
                self.symbols.push(generated_class_symbol(
                    self.source,
                    &name,
                    &canonical,
                    self.call.span,
                ));
            }
            self.classes.insert(
                0,
                GeneratedClass {
                    class_name: name.clone(),
                    call_span: self.call.span,
                    guard_path: Vec::new(),
                },
            );
            names.push(name);
        }
        names.extend(std::mem::take(&mut self.static_names));
        // The base unit must precede every conditional unit in the stylesheet.
        self.utility_css.push_str(&self.runtime_css);
        StyleEmission {
            class_property: self.class_property,
            static_names: names,
            parts: self.parts,
            classes: self.classes,
            conditional_props: self.conditional_props,
            call_span: self.call.span,
        }
    }
}

/// The pieces one utility literal contributes before it becomes a leaf or a base-unit entry.
///
/// `canonical` is the text appended to the class-hash input. Keeping markers, external names, and
/// resolved utilities in separate lists is what lets an unresolved candidate still reach the
/// output as a class name while contributing no rule.
#[derive(Default)]
struct LiteralContribution {
    /// Canonical candidate and wrapper spelling of the literal, in source order.
    canonical: String,
    /// Utilities to emit as this leaf's rule.
    utilities: Vec<CompiledUtility>,
    /// Compatibility markers, emitted instead of a rule.
    markers: Vec<String>,
    /// Class names Stillwind does not own, passed through unchanged.
    external: Vec<String>,
    /// Names with no rule of their own, such as handle markers.
    plain: Vec<String>,
}

/// Renders a plain class value: strings that clsx can join directly.
///
/// Returns `None` when the value cannot be expressed without the fragment runtime, which is what
/// makes Tier 1 a proof obligation rather than a preference. An attribute can never appear inside
/// one class property, so any attribute entry makes the whole value unrenderable this way.
pub(super) fn render_plain(value: &Emitted) -> Option<String> {
    match value {
        Emitted::Expression(text) => Some(text.clone()),
        Emitted::Leaf { names } => Some(format!("\"{}\"", names.join(" "))),
        Emitted::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                parts.push(match item {
                    Some(item) => render_plain(item)?,
                    None => "null".to_owned(),
                });
            }
            Some(format!("[{}]", parts.join(",")))
        }
        Emitted::Conditional {
            test,
            kind,
            consequent,
            alternate,
        } => {
            let consequent = match consequent {
                Some(value) => render_plain(value)?,
                None => "null".to_owned(),
            };
            match kind {
                ConditionalKind::LogicalAnd => Some(format!("({test}) && ({consequent})")),
                ConditionalKind::Ternary => {
                    let alternate = match alternate {
                        Some(value) => render_plain(value)?,
                        None => "null".to_owned(),
                    };
                    Some(format!("({test}) ? ({consequent}) : ({alternate})"))
                }
            }
        }
        Emitted::Group(entries) => {
            let mut parts = Vec::new();
            for entry in entries {
                match entry {
                    // Attributes cannot live inside one class property.
                    Some(EmittedEntry::Attribute(_)) => return None,
                    Some(EmittedEntry::Class(value)) => {
                        parts.push(render_plain(value)?);
                    }
                    None => {}
                }
            }
            match parts.len() {
                0 => None,
                1 => parts.pop(),
                _ => Some(format!("[{}]", parts.join(","))),
            }
        }
    }
}

/// Renders a fragment expression that `stillwindMerge` can consume.
///
/// The output is a fresh expression, not a reference to anything already emitted, so the same
/// tree can be rendered once per tier decision without sharing state.
pub(super) fn render_fragment(
    value: &Emitted,
    runtime: &mut RuntimeBindings<'_>,
    span: Span,
) -> String {
    match value {
        Emitted::Expression(text) => {
            let key = runtime.require(RuntimeHelper::ClassKey, span);
            format!("{{[{key}]:({text})}}")
        }
        Emitted::Leaf { names } => fragment(runtime, span, &[names.join(" ")]),
        Emitted::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                parts.push(match item {
                    Some(item) => render_fragment(item, runtime, span),
                    None => "null".to_owned(),
                });
            }
            format!("[{}]", parts.join(","))
        }
        Emitted::Conditional {
            test,
            kind,
            consequent,
            alternate,
        } => {
            let consequent = match consequent {
                Some(value) => render_fragment(value, runtime, span),
                None => "null".to_owned(),
            };
            match kind {
                ConditionalKind::LogicalAnd => format!("({test}) && ({consequent})"),
                ConditionalKind::Ternary => {
                    let alternate = match alternate {
                        Some(value) => render_fragment(value, runtime, span),
                        None => "null".to_owned(),
                    };
                    format!("({test}) ? ({consequent}) : ({alternate})")
                }
            }
        }
        Emitted::Group(entries) => {
            let mut parts = Vec::new();
            for entry in entries {
                match entry {
                    Some(EmittedEntry::Attribute(text)) => parts.push(format!("{{{text}}}")),
                    Some(EmittedEntry::Class(value)) => {
                        parts.push(render_fragment(value, runtime, span));
                    }
                    None => parts.push("null".to_owned()),
                }
            }
            if parts.len() == 1 {
                return parts.remove(0);
            }
            format!("[{}]", parts.join(","))
        }
    }
}

/// Builds a fragment object for one already-joined class string.
fn fragment(runtime: &mut RuntimeBindings<'_>, span: Span, values: &[String]) -> String {
    let key = runtime.require(RuntimeHelper::ClassKey, span);
    format!("{{[{key}]:\"{}\"}}", values.join(" "))
}

/// Whether the emitted text evaluates user expressions.
///
/// This is what separates Tier 1 from Tier 0: a static leaf can be folded into a string, while a
/// test, an external name, or a dynamic asset lookup has to be evaluated where the user wrote it.
fn uses_user_code(expression: &ClassExpression) -> bool {
    match expression {
        ClassExpression::Conditional { .. }
        | ClassExpression::External(_)
        | ClassExpression::DynamicHandleProjection { .. }
        | ClassExpression::DynamicAnimationUse { .. } => true,
        ClassExpression::Array(elements) => elements.iter().any(uses_user_code),
        ClassExpression::Variant { inputs, .. } => inputs.iter().any(uses_user_code),
        ClassExpression::Group { entries, .. } => entries.iter().any(|entry| match entry {
            GroupEntry::Attribute(_) => true,
            GroupEntry::Class(expression) => uses_user_code(expression),
        }),
        _ => false,
    }
}

/// Whether the expression carries a user-owned class or asset value.
///
/// Such a value can only be resolved at runtime, so it cannot be hoisted next to a static class
/// without changing what is evaluated and in which order.
fn is_user_value(expression: &ClassExpression) -> bool {
    match expression {
        ClassExpression::External(_)
        | ClassExpression::DynamicHandleProjection { .. }
        | ClassExpression::DynamicAnimationUse { .. } => true,
        ClassExpression::Array(elements) => elements.iter().any(is_user_value),
        ClassExpression::Variant { inputs, .. } => inputs.iter().any(is_user_value),
        ClassExpression::Conditional {
            consequent,
            alternate,
            ..
        } => consequent.iter().chain(alternate.iter()).any(is_user_value),
        ClassExpression::Group { entries, .. } => entries.iter().any(|entry| match entry {
            GroupEntry::Attribute(_) => false,
            GroupEntry::Class(expression) => is_user_value(expression),
        }),
        _ => false,
    }
}

/// The source position used to order an expression among a call's parts.
///
/// Tests order by their own span, so a conditional part sits where its test is written rather than
/// where its first literal is. A container query or group has no single token, so it orders by its
/// opening span, which keeps evaluation order equal to the source order the user sees.
fn expression_position(expression: &ClassExpression) -> u32 {
    match expression {
        ClassExpression::Utilities(value) => value.span.start.0,
        ClassExpression::Array(elements) => elements.first().map_or(0, expression_position),
        ClassExpression::Conditional { test, .. } => test.start.0,
        ClassExpression::Variant { variant, .. } => match variant {
            StyleVariant::Prefix(prefix) | StyleVariant::Ref { prefix, .. } => prefix.span.start.0,
        },
        ClassExpression::Group { span, .. } => span.start.0,
        ClassExpression::HandleProjection { handle, .. } => handle.call_span.start.0,
        ClassExpression::AnimationUse { animation, .. } => animation.span.start.0,
        ClassExpression::External(value)
        | ClassExpression::DynamicHandleProjection { value, .. }
        | ClassExpression::DynamicAnimationUse { value, .. } => value.start.0,
    }
}
