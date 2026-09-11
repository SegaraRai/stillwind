//! Structure-preserving style emission.
//!
//! The conditional and array structure of every class expression is kept: each
//! user expression appears once in the output, is evaluated once, and keeps its
//! source position. Class contributions are emitted either as a plain class
//! value (Tier 1) or as a fragment (Tier 2); the renderer picks the shape.

use std::collections::BTreeSet;

use stillwind_design_system::{
    ThemeIndex, compatibility_marker_class, is_tailwind_relationship_candidate,
};
use stillwind_ir::{
    AttributeEntry, ClassExpression, ConditionalKind, GroupEntry, StyleCall, StyleInput,
    StyleVariant,
};
use stillwind_semantic::{AssetKind, IdentityId};
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
    asset_symbol_name, class_name, generated_class_symbol, handle_projection_name, ref_symbol_name,
    runtime_projection_class,
};
use super::utilities::emission::emit_utility_rules;
use super::utilities::resolve::{
    CompiledUtility, resolve_utility_targets, unresolved_utility_rule, view_transition_utility,
};
use crate::CompilerOptions;
use crate::runtime::{RuntimeBindings, RuntimeHelper};

pub(super) struct CallEmitter<'a, 'b> {
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
    symbols: &'a mut Vec<crate::GeneratedSymbol>,
    emitted_utility_classes: &'a mut BTreeSet<(String, String)>,
    runtime_assets: &'a mut BTreeSet<IdentityId>,
    runtime: &'a mut RuntimeBindings<'b>,
    static_canonical: String,
    static_utilities: Vec<CompiledUtility>,
    static_names: Vec<String>,
    parts: Vec<StyleEmissionPart>,
    classes: Vec<GeneratedClass>,
    class_index: u32,
    conditional_props: bool,
    view_transition_seen: bool,
}

pub(super) struct CallEmitterInputs<'a, 'b> {
    pub source: &'a SourceFile,
    pub options: &'a CompilerOptions,
    pub theme: &'a ThemeIndex,
    pub call: &'a StyleCall,
    pub class_property: &'static str,
    pub diagnostics: &'a mut Vec<CompileError>,
    pub lint_findings: &'a mut Vec<LintFinding>,
    pub utility_css: &'a mut String,
    pub symbols: &'a mut Vec<crate::GeneratedSymbol>,
    pub emitted_utility_classes: &'a mut BTreeSet<(String, String)>,
    pub runtime_assets: &'a mut BTreeSet<IdentityId>,
    pub runtime: &'a mut RuntimeBindings<'b>,
}

impl<'a, 'b> CallEmitter<'a, 'b> {
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

    fn emit(mut self) -> Option<StyleEmission> {
        for input in self.call.inputs.clone() {
            self.emit_input(&input)?;
        }
        Some(self.finish())
    }

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
                if !self.begin_view_transition() {
                    return Some(());
                }
                let name = asset_symbol_name(
                    &transition.module_path,
                    AssetKind::ViewTransition,
                    &transition.name,
                );
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
                    self.runtime_assets.insert(IdentityId::new(
                        AssetKind::ViewTransition,
                        &transition.module_path,
                        transition.name.span.start.0,
                        transition.name.span.end.0,
                        transition.name.value.clone(),
                    ));
                    candidates.push(asset_symbol_name(
                        &transition.module_path,
                        AssetKind::ViewTransition,
                        &transition.name,
                    ));
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

    fn push_expression(&mut self, text: String, position: u32) {
        self.parts.push(StyleEmissionPart::Class {
            emitted: Emitted::Expression(text),
            position,
            user_code: true,
            user_value: true,
        });
    }

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
                let plan = StyleVariantPlan::resolve(wrappers, self.theme, self.diagnostics)?;
                let mut utility =
                    resolve_animation_use(animation, options, self.theme, self.diagnostics);
                plan.apply_animation(&mut utility);
                let canonical = format!(
                    "{}{}:{}:{}\u{0}",
                    plan.canonical, "animation:", animation.name.value, options.value
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
                    self.runtime_assets.insert(IdentityId::new(
                        AssetKind::Animation,
                        &animation.module_path,
                        animation.name.span.start.0,
                        animation.name.span.end.0,
                        animation.name.value.clone(),
                    ));
                    let identity = asset_symbol_name(
                        &animation.module_path,
                        AssetKind::Animation,
                        &animation.name,
                    );
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

    fn fold(&mut self, contribution: LiteralContribution) {
        self.static_canonical.push_str(&contribution.canonical);
        self.static_utilities.extend(contribution.utilities);
        self.static_names.extend(contribution.markers);
        self.static_names.extend(contribution.external);
        self.static_names.extend(contribution.plain);
    }

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

    fn render_attribute(&mut self, attribute: &AttributeEntry) -> Option<String> {
        let value = self.slice(attribute.value)?;
        Some(format!("\"{}\":{value}", attribute.name.value))
    }

    fn emit_view_transition_class(&mut self) {
        self.static_canonical.push_str("view-transition");
        self.static_canonical.push('\u{0}');
        self.static_utilities.push(view_transition_utility());
    }

    fn slice(&self, span: Span) -> Option<String> {
        self.source.slice(span).ok().map(str::to_owned)
    }

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

#[derive(Default)]
struct LiteralContribution {
    canonical: String,
    utilities: Vec<CompiledUtility>,
    markers: Vec<String>,
    external: Vec<String>,
    plain: Vec<String>,
}

/// Renders a plain class value: strings that clsx can join directly.
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

fn fragment(runtime: &mut RuntimeBindings<'_>, span: Span, values: &[String]) -> String {
    let key = runtime.require(RuntimeHelper::ClassKey, span);
    format!("{{[{key}]:\"{}\"}}", values.join(" "))
}

/// Whether the emitted text evaluates user expressions.
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
        ClassExpression::AnimationUse { animation, .. } => animation.name.span.start.0,
        ClassExpression::External(value)
        | ClassExpression::DynamicHandleProjection { value, .. }
        | ClassExpression::DynamicAnimationUse { value, .. } => value.start.0,
    }
}
