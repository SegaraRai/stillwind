//! Structure-preserving style emission.
//!
//! The conditional and array structure of every class expression is kept: each
//! user expression appears once in the output, is evaluated once, and keeps its
//! source position. Class contributions are emitted either as a plain class
//! value (Tier 1) or as a fragment (Tier 2); the renderer picks the shape.
//!
//! The traversal delegates class allocation and stylesheet publication to [`ClassUnits`],
//! which writes base rules before conditional rules. [`LiteralContribution`] resolves
//! utility text; [`super::expression_render`] serializes lowered class trees
//! independently of CSS order, so the cascade never depends on class attribute order.

use std::collections::BTreeSet;

use stillwind_design_system::ThemeIndex;
use stillwind_diagnostics::{CompileError, CompileErrorKind, LintFinding, LintKind};
use stillwind_ir::{
    AttributeEntry, ClassExpression, ConditionalKind, CustomPropertyEntry, GroupEntry, StyleCall,
    StyleInput, StyleVariant,
};
use stillwind_semantic::IdentityId;
use stillwind_source::{SourceFile, Span};

use super::CssContribution;
use super::class_units::ClassUnits;
use super::expression_render::render_plain;
use super::handles::{
    handle_marker_class, ref_identity, render_handle_projection, scope_identity, scope_marker_class,
};
use super::lexer::lex_utilities;
use super::literal::LiteralContribution;
use super::motion::resolve_animation_use;
use super::props::{Emitted, EmittedEntry, GuardStep, StyleEmission, StyleEmissionPart};
use super::style_variants::StyleVariantPlan;
use super::symbols::{
    handle_projection_name, motion_symbol_name, ref_symbol_name, runtime_projection_class,
};
use super::utilities::resolve::{
    CompiledUtility, resolve_utility_targets, unresolved_utility_kind, view_transition_utility,
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
    /// Compiler configuration for generated identities and output.
    options: &'a CompilerOptions,
    /// Resolved theme and utility registry for this compilation.
    theme: &'a ThemeIndex,
    /// Analyzed style call being lowered.
    call: &'a StyleCall,
    /// Framework class attribute spelling, such as className or class.
    class_property: &'static str,
    /// Compilation errors appended while lowering the call.
    diagnostics: &'a mut Vec<CompileError>,
    /// Nonfatal findings appended in encounter order.
    lint_findings: &'a mut Vec<LintFinding>,
    /// Identities the module references at runtime; the pipeline uses this set to decide which
    /// declarations are replaced by a runtime call and which are erased.
    runtime_assets: &'a mut BTreeSet<IdentityId>,
    /// Motion identities reached by static or dynamic uses.
    motion_assets: &'a mut BTreeSet<IdentityId>,
    /// Per-module allocator of helper bindings and import realms.
    runtime: &'a mut RuntimeBindings<'b>,
    /// Runtime contributions accumulated for source-position ordering during rendering.
    parts: Vec<StyleEmissionPart>,
    /// Owns class allocation and base-before-conditional CSS publication.
    units: ClassUnits<'a>,
    /// Set when a props object sits inside a conditional expression, which forces Tier 2.
    conditional_props: bool,
    /// Set once the call contributed a view transition, so a second one is reported.
    view_transition_seen: bool,
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
            runtime_assets: inputs.runtime_assets,
            motion_assets: inputs.motion_assets,
            runtime: inputs.runtime,
            parts: Vec::new(),
            units: ClassUnits::new(
                inputs.source,
                inputs.options,
                inputs.theme,
                inputs.call.span,
                inputs.utility_css,
                inputs.symbols,
                inputs.emitted_utility_classes,
            ),
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
                        GroupEntry::Class(expression) => {
                            let position = expression_position(expression);
                            self.emit_item(expression, position);
                        }
                        GroupEntry::Attribute(attribute) => {
                            let text = self.render_attribute(attribute)?;
                            self.parts.push(StyleEmissionPart::Attributes {
                                text,
                                position: attribute.value.start.0,
                            });
                        }
                        GroupEntry::CustomProperty(property) => {
                            let text = self.render_custom_property(property)?;
                            self.parts.push(StyleEmissionPart::CustomProperties {
                                text,
                                position: property.value.start.0,
                            });
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
                if !self.units.base.names.contains(&marker) {
                    self.units.base.names.push(marker);
                }
                self.units.base.canonical.push_str("scope:");
                self.units.base.canonical.push_str(&scope.name.value);
                self.units.base.canonical.push('\u{0}');
            }
            StyleInput::HandleAttachment(handle) => {
                let marker = handle_marker_class(handle);
                if !self.units.base.names.contains(&marker) {
                    self.units.base.names.push(marker);
                }
                self.units.base.canonical.push_str("ref:");
                self.units.base.canonical.push_str(&handle.name.value);
                self.units.base.canonical.push('\u{0}');
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
                self.units.base.canonical.push_str("container:");
                self.units.base.canonical.push_str(&ref_symbol_name(handle));
                self.units.base.canonical.push(':');
                self.units.base.canonical.push_str(&query.value);
                self.units.base.canonical.push('\u{0}');
                for utility in lex_utilities(value, self.diagnostics) {
                    self.units.base.canonical.push_str(&utility.value.raw);
                    self.units.base.canonical.push('\u{0}');
                    if let Some(mut utilities) =
                        resolve_utility_targets(&utility.value.raw, self.theme)
                    {
                        for resolved in &mut utilities {
                            resolved.container =
                                Some(format!("{} ({})", ref_symbol_name(handle), query.value));
                        }
                        self.units.base.utilities.append(&mut utilities);
                    } else {
                        self.lint_findings.push(LintFinding::new(
                            LintKind::UnresolvedUtility {
                                utility: utility.value.raw.clone(),
                                classification: unresolved_utility_kind(&utility.value.raw),
                            },
                            utility.span,
                        ));
                        self.units.base.names.push(utility.value.raw.clone());
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

    /// Reserves the call's view-transition attachment, returning whether it is available.
    /// A repeated attachment appends a fatal error and returns `false`.
    ///
    /// A call can contribute only one view-transition pair, because the pair shares one generated
    /// name and a second use would overwrite the first.
    fn begin_view_transition(&mut self) -> bool {
        if self.view_transition_seen {
            self.diagnostics.push(CompileError::new(
                CompileErrorKind::MultipleViewTransitions,
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
                let contribution = LiteralContribution::resolve(
                    value,
                    &plan,
                    self.options,
                    self.theme,
                    self.diagnostics,
                    self.lint_findings,
                );
                if fold && guard.is_empty() {
                    self.units.fold(contribution);
                    return None;
                }
                Some(self.units.leaf(contribution, guard))
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
                        GroupEntry::Class(expression) => self
                            .emit_expression(expression, guard, wrappers, fold)
                            .map(EmittedEntry::Class),
                        GroupEntry::Attribute(attribute) => {
                            let text = self.render_attribute(attribute)?;
                            self.conditional_props = true;
                            Some(EmittedEntry::Attribute(text))
                        }
                        GroupEntry::CustomProperty(property) => {
                            let text = self.render_custom_property(property)?;
                            self.conditional_props = true;
                            Some(EmittedEntry::CustomProperty(text))
                        }
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
                    self.units.base.canonical.push_str("handle:");
                    self.units.base.canonical.push_str(&ref_symbol_name(handle));
                    self.units.base.canonical.push(':');
                    self.units.base.canonical.push_str(name);
                    self.units.base.canonical.push('\u{0}');
                    self.units.base.utilities.push(utility);
                    if !self.units.base.names.contains(&marker) {
                        self.units.base.names.push(marker);
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
                Some(self.units.leaf(contribution, guard))
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
                    let class = self.units.leaf(contribution, guard);
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
                    self.units.fold(contribution);
                    return None;
                }
                Some(self.units.leaf(contribution, guard))
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
                    let class = self.units.leaf(contribution, guard);
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

    /// Emits one custom property through the framework's fragment channel.
    fn render_custom_property(&mut self, property: &CustomPropertyEntry) -> Option<String> {
        let value = self.slice(property.value)?;
        let helper = self
            .runtime
            .require(RuntimeHelper::CssValue, property.value);
        let name = serde_json::to_string(&property.name.value).ok()?;
        Some(format!("[{name}]:{helper}(({value}))"))
    }

    /// Renders one attribute entry of an object literal, with its value verbatim.
    fn render_attribute(&mut self, attribute: &AttributeEntry) -> Option<String> {
        let value = self.slice(attribute.value)?;
        Some(format!("\"{}\":{value}", attribute.name.value))
    }

    /// Contributes the class that carries a view transition's name and class properties.
    fn emit_view_transition_class(&mut self) {
        self.units.base.canonical.push_str("view-transition");
        self.units.base.canonical.push('\u{0}');
        self.units.base.utilities.push(view_transition_utility());
    }

    /// Slices an original span, or reports that it cannot be read.
    fn slice(&self, span: Span) -> Option<String> {
        self.source.slice(span).ok().map(str::to_owned)
    }

    /// Publishes the class units and returns the expression parts in source order.
    fn finish(self) -> StyleEmission {
        let (static_names, classes) = self.units.finish();
        StyleEmission {
            class_property: self.class_property,
            static_names,
            parts: self.parts,
            classes,
            conditional_props: self.conditional_props,
            call_span: self.call.span,
        }
    }
}

/// The borrows one call needs, grouped so the emitter can be constructed in one place.
pub(super) struct CallEmitterInputs<'a, 'b> {
    /// Original buffer used to slice user expressions.
    pub source: &'a SourceFile,
    /// Compiler configuration for this call.
    pub options: &'a CompilerOptions,
    /// Resolved theme and utility registry.
    pub theme: &'a ThemeIndex,
    /// Analyzed style call to lower.
    pub call: &'a StyleCall,
    /// Framework class attribute spelling.
    pub class_property: &'static str,
    /// Shared destination for compilation errors.
    pub diagnostics: &'a mut Vec<CompileError>,
    /// Shared destination for lint findings, appended in encounter order.
    pub lint_findings: &'a mut Vec<LintFinding>,
    /// Shared destination for module CSS contributions.
    pub utility_css: &'a mut Vec<CssContribution>,
    /// Shared destination for generated symbol records.
    pub symbols: &'a mut Vec<GeneratedSymbol>,
    /// Previously emitted (class name, canonical input) pairs used to suppress identical units.
    pub emitted_utility_classes: &'a mut BTreeSet<(String, String)>,
    /// Referenced runtime identities used to retain declarations that need runtime values.
    pub runtime_assets: &'a mut BTreeSet<IdentityId>,
    /// Motion definitions reached by static or dynamic uses.
    pub motion_assets: &'a mut BTreeSet<IdentityId>,
    /// Shared helper binding allocator for the module.
    pub runtime: &'a mut RuntimeBindings<'b>,
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
            GroupEntry::Class(expression) => uses_user_code(expression),
            GroupEntry::Attribute(_) | GroupEntry::CustomProperty(_) => true,
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
            GroupEntry::Class(expression) => is_user_value(expression),
            GroupEntry::Attribute(_) | GroupEntry::CustomProperty(_) => false,
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
