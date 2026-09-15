//! Structure-preserving style emission.
//!
//! [`CallEmitter::run`] resolves style inputs into class units and typed contributions
//! for fragment folding. It retains guards, source occurrences; subtrees with no
//! contribution are omitted under the macro purity contract. It does not choose final
//! framework keys or allocate runtime helper imports.
//!
//! [`ClassUnits`] owns class identities and base-before-conditional CSS publication.
//! [`LiteralContribution`] resolves utility text, and fragment rendering later chooses
//! a representation without changing those CSS identities or declaration order.

use std::collections::BTreeSet;

use stillwind_design_system::{ThemeIndex, UtilityOrder};
use stillwind_diagnostics::{CompileError, CompileErrorKind, LintFinding};
use stillwind_ir::{
    AttributeEntry, ClassExpression, ConditionalKind, CustomPropertyEntry, GroupEntry, StyleCall,
    StyleInput, StyleVariant,
};
use stillwind_semantic::{AssetKind, IdentityId};
use stillwind_source::{SourceFile, Span};

use super::CssContribution;
use super::class_units::ClassUnits;
use super::fragment::{Emitted, EmittedEntry, Property, ValueExpr};
use super::handles::{ref_identity, render_handle_projection, scope_identity};
use super::literal::LiteralContribution;
use super::motion::resolve_animation_use;
use super::props::{GuardStep, StyleEmission, StyleEmissionPart};
use super::style_variants::StyleVariantPlan;
use super::symbols::{
    anchor_name, handle_projection_name, motion_name_symbol, ref_marker_name,
    runtime_projection_class, scope_marker_name,
};
use super::utilities::resolve::{CompiledUtility, view_transition_utility};
use crate::generated::{Code, js_string};
use crate::identifier::IdentifierContext;
use crate::runtime::RuntimeHelper;
use crate::{CompilerOptions, GeneratedSymbol};

/// One `style()` call being lowered.
///
/// The emitter accumulates state rather than building a tree in one pass, because the base unit
/// has to be collected across argument positions while each conditional leaf needs its own class
/// unit and guard path.
pub(super) struct CallEmitter<'a> {
    /// The buffer the module's original text is sliced from.
    source: &'a SourceFile,
    /// Compiler configuration for generated identities and output.
    options: &'a CompilerOptions,
    /// Name allocation shared with the rest of this compilation.
    identifiers: &'a IdentifierContext,
    /// Resolved theme and utility registry for this compilation.
    theme: &'a ThemeIndex,
    /// Analyzed style call being lowered.
    call: &'a StyleCall,
    /// Compilation errors appended while lowering the call.
    diagnostics: &'a mut Vec<CompileError>,
    /// Nonfatal findings appended in encounter order.
    lint_findings: &'a mut Vec<LintFinding>,
    /// Identities the module references at runtime; the pipeline uses this set to decide which
    /// declarations are replaced by a runtime call and which are erased.
    runtime_assets: &'a mut BTreeSet<IdentityId>,
    /// Motion identities reached by static or dynamic uses.
    motion_assets: &'a mut BTreeSet<IdentityId>,
    /// Runtime contributions accumulated for source-position ordering during rendering.
    parts: Vec<StyleEmissionPart>,
    /// Owns class allocation and base-before-conditional CSS publication.
    units: ClassUnits<'a>,
    /// Set once the call contributed a view transition, so a second one is reported.
    view_transition_seen: bool,
    /// Anchor and container names this call wrote, reported with the class symbols.
    written_name_symbols: Vec<GeneratedSymbol>,
}

impl<'a> CallEmitter<'a> {
    /// Lowers one analyzed call into finalized class units and ordered contributions.
    ///
    /// Input spans must address `source`. Shared CSS, symbol, asset, and diagnostic
    /// destinations may be extended during traversal. `None` means an input could not
    /// be lowered; `Some` is not proof of diagnostic-free compilation. The pipeline
    /// must inspect diagnostics and discard failed output transactionally.
    #[allow(clippy::needless_pass_by_value)] // The struct owns the borrows this call consumes.
    pub(super) fn run(inputs: CallEmitterInputs<'a>) -> Option<StyleEmission> {
        let emitter = Self {
            source: inputs.source,
            options: inputs.options,
            identifiers: inputs.identifiers,
            theme: inputs.theme,
            call: inputs.call,
            diagnostics: inputs.diagnostics,
            lint_findings: inputs.lint_findings,
            runtime_assets: inputs.runtime_assets,
            motion_assets: inputs.motion_assets,
            parts: Vec::new(),
            units: ClassUnits::new(
                inputs.source,
                inputs.options,
                inputs.identifiers,
                inputs.theme,
                inputs.call.span,
                inputs.utility_css,
                inputs.symbols,
                inputs.emitted_utility_classes,
            ),
            view_transition_seen: false,
            written_name_symbols: Vec::new(),
        };
        emitter.emit()
    }

    /// Traverses top-level inputs and publishes accumulated class units on completion.
    /// Stops on an input returning `None`; shared output destinations are not rolled back.
    fn emit(mut self) -> Option<StyleEmission> {
        for input in self.call.inputs.clone() {
            self.emit_input(&input)?;
        }
        let parts = std::mem::take(&mut self.parts);
        let call_span = self.call.span;
        match self.units.finish() {
            Ok((static_names, classes)) => Some(StyleEmission {
                static_names,
                parts,
                classes,
                call_span,
                written_name_symbols: std::mem::take(&mut self.written_name_symbols),
            }),
            Err(failure) => {
                self.diagnostics.push(CompileError::new(failure, call_span));
                None
            }
        }
    }

    /// Dispatches one style input.
    ///
    /// Static utility contributions join class allocation; retained runtime values become
    /// parts with original-source ordering anchors and mappings. `None` propagates a
    /// required lowering/slicing failure; diagnostics and accumulated state remain owned
    /// by the pipeline, including failures represented by omitted nested contributions.
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
                            let property = self.render_attribute(attribute)?;
                            self.parts.push(StyleEmissionPart::Attributes {
                                property,
                                position: attribute.value.start.0,
                            });
                        }
                        GroupEntry::CustomProperty(property) => {
                            let custom_property = self.render_custom_property(property)?;
                            self.parts.push(StyleEmissionPart::CustomProperties {
                                property: custom_property,
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
                let expression = self.source_expression(*value)?;
                self.parts.push(StyleEmissionPart::Base {
                    expression,
                    position: value.start.0,
                });
            }
            StyleInput::ScopeAttachment(scope) => {
                let marker = match scope_marker_name(self.identifiers, scope) {
                    Ok(marker) => marker,
                    Err(failure) => {
                        self.reject_name(failure, self.call.span);
                        return None;
                    }
                };
                if !self.units.base.names.contains(&marker) {
                    self.units.base.names.push(marker);
                }
                self.units.base.canonical.push_str("scope:");
                self.units.base.canonical.push_str(&scope.name.value);
                self.units.base.canonical.push('\u{0}');
            }
            StyleInput::HandleAttachment(handle) => {
                let marker = match ref_marker_name(self.identifiers, handle) {
                    Ok(marker) => marker,
                    Err(failure) => {
                        self.reject_name(failure, self.call.span);
                        return None;
                    }
                };
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
                let expression = ValueExpr::Call {
                    helper: RuntimeHelper::HandleClass,
                    arguments: vec![self.source_expression(*value)?],
                    span: *value,
                };
                self.push_expression(expression, value.start.0);
            }
            StyleInput::DynamicHandleAttachment { value, handles } => {
                self.runtime_assets.extend(handles.iter().map(ref_identity));
                let expression = ValueExpr::Call {
                    helper: RuntimeHelper::HandleClass,
                    arguments: vec![self.source_expression(*value)?],
                    span: *value,
                };
                self.push_expression(expression, value.start.0);
            }
            StyleInput::ViewTransitionUse { transition, key } => {
                self.motion_assets.insert(transition.identity.clone());
                if !self.begin_view_transition() {
                    return Some(());
                }
                let name = match self.record_motion_name(
                    &transition.identity,
                    AssetKind::ViewTransition,
                    transition.span,
                ) {
                    Ok(name) => name,
                    Err(failure) => {
                        self.reject_name(failure, self.call.span);
                        return None;
                    }
                };
                self.emit_view_transition_class();
                let key = match key {
                    Some(key) => self.source_expression(*key)?,
                    None => "undefined".into(),
                };
                let expression = ValueExpr::Call {
                    helper: RuntimeHelper::ViewTransitionStyle,
                    arguments: vec![
                        format!("\"{name}\"").into(),
                        key,
                        format!("[\"{name}\"]").into(),
                        "\"--sw-vt-name\"".into(),
                        "\"--sw-vt-class\"".into(),
                    ],
                    span: self.call.span,
                };
                self.parts.push(StyleEmissionPart::Style {
                    expression,
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
                    match self.record_motion_name(
                        &transition.identity,
                        AssetKind::ViewTransition,
                        transition.span,
                    ) {
                        Ok(name) => candidates.push(name),
                        Err(failure) => {
                            self.reject_name(failure, self.call.span);
                            return None;
                        }
                    }
                }
                self.emit_view_transition_class();
                let key = match key {
                    Some(key) => self.source_expression(*key)?,
                    None => "undefined".into(),
                };
                let candidates = candidates
                    .iter()
                    .map(|candidate| format!("\"{candidate}\""))
                    .collect::<Vec<_>>()
                    .join(",");
                let expression = ValueExpr::Call {
                    helper: RuntimeHelper::ViewTransitionStyle,
                    arguments: vec![
                        self.source_expression(*value)?,
                        key,
                        format!("[{candidates}]").into(),
                        "\"--sw-vt-name\"".into(),
                        "\"--sw-vt-class\"".into(),
                    ],
                    span: self.call.span,
                };
                self.parts.push(StyleEmissionPart::Style {
                    expression,
                    position: value.start.0,
                });
            }
        }
        Some(())
    }

    /// Generates one motion asset's name and records it for pool validation.
    ///
    /// A motion name is written by every use of its declaration, so the report keeps
    /// one record per declaration and the pipeline drops later repeats.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure for the asset's family.
    fn record_motion_name(
        &mut self,
        identity: &IdentityId,
        kind: AssetKind,
        span: Span,
    ) -> Result<String, CompileErrorKind> {
        let symbol = motion_name_symbol(self.identifiers, identity, kind, span)?;
        let name = symbol.name.clone();
        self.written_name_symbols.push(symbol);
        Ok(name)
    }

    /// Converts a generator failure into a fatal diagnostic at `span`.
    ///
    /// The failed call is omitted from the output, and the pipeline clears every
    /// generated artifact once a diagnostic exists, so a rejected name never reaches
    /// generated code.
    fn reject_name(&mut self, failure: CompileErrorKind, span: Span) {
        self.diagnostics.push(CompileError::new(failure, span));
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

    /// Appends a deferred class value at an original-source UTF-8 ordering position.
    /// Records the expression without rendering it or registering its runtime helper.
    fn push_expression(&mut self, expression: ValueExpr, position: u32) {
        self.parts.push(StyleEmissionPart::Class {
            emitted: Emitted::Expression(expression),
            position,
        });
    }

    /// Lowers one top-level class item with empty guard and variant stacks.
    /// Appends retained runtime code at `position`; omitted contributions may instead
    /// have joined the base unit or reported diagnostics during lowering.
    fn emit_item(&mut self, expression: &ClassExpression, position: u32) {
        let mut wrappers = Vec::new();
        let mut guard = Vec::new();
        if let Some(emitted) = self.emit_expression(expression, &mut guard, &mut wrappers, true) {
            self.parts
                .push(StyleEmissionPart::Class { emitted, position });
        }
    }

    /// Lowers one class expression, tracking the guard path and the active wrappers.
    ///
    /// `fold` is set at the top level of a call: a leaf reached without a guard can join the base
    /// unit, while a leaf under any test keeps its own class unit. Conditional arms clear the flag
    /// for their contents, so a nested leaf never merges into the base unit by accident.
    /// `None` covers base-folded, provably empty, and failed contributions. Guard and
    /// wrapper stacks are restored after nested traversal; callers inspect diagnostics
    /// separately rather than treating absence as a successful empty expression.
    fn emit_expression(
        &mut self,
        expression: &ClassExpression,
        guard: &mut Vec<GuardStep>,
        wrappers: &mut Vec<StyleVariant>,
        fold: bool,
    ) -> Option<Emitted> {
        self.emit_expression_inner(expression, guard, wrappers, fold)
            .filter(|value| !value.is_empty())
    }

    /// Resolves one class node while preserving guards, variant context, and source mappings.
    /// May allocate CSS/class units or record asset dependencies and diagnostics.
    /// Returns `None` for a base-folded leaf or a lowering failure; empty runtime nodes
    /// are left for `emit_expression` to filter.
    #[allow(clippy::too_many_lines)] // One exhaustive match over the class expression.
    fn emit_expression_inner(
        &mut self,
        expression: &ClassExpression,
        guard: &mut Vec<GuardStep>,
        wrappers: &mut Vec<StyleVariant>,
        fold: bool,
    ) -> Option<Emitted> {
        match expression {
            ClassExpression::Utilities(value) => {
                let plan = StyleVariantPlan::resolve(
                    self.identifiers,
                    wrappers,
                    self.theme,
                    self.diagnostics,
                )?;
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
                match self.units.leaf(contribution, guard) {
                    Ok(leaf) => Some(leaf),
                    Err(failure) => {
                        self.reject_name(failure, self.call.span);
                        None
                    }
                }
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
                let test = self.source_expression(test_span)?;
                match kind {
                    ConditionalKind::LogicalAnd => Some(Emitted::Conditional {
                        code: test,
                        kind: *kind,
                        consequent: consequent.map(Box::new),
                        alternate: None,
                    }),
                    ConditionalKind::Ternary => {
                        guard.push(GuardStep {
                            test: test_span,
                            when_true: false,
                        });
                        let alternate = self.emit_sequence(alternate, guard, wrappers);
                        guard.pop();
                        Some(Emitted::Conditional {
                            code: test,
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
                            let property = self.render_attribute(attribute)?;
                            Some(EmittedEntry::Attribute(property))
                        }
                        GroupEntry::CustomProperty(property) => {
                            let custom_property = self.render_custom_property(property)?;
                            Some(EmittedEntry::CustomProperty(custom_property))
                        }
                    };
                    any |= item.is_some();
                    items.push(item);
                }
                any.then_some(Emitted::Group(items))
            }
            ClassExpression::External { span, arguments } => {
                if arguments.is_empty() {
                    return None;
                }
                let expression = ValueExpr::Call {
                    helper: RuntimeHelper::Clsx,
                    arguments: arguments
                        .iter()
                        .map(|argument| self.source_expression(*argument))
                        .collect::<Option<Vec<_>>>()?,
                    span: *span,
                };
                Some(Emitted::Expression(expression))
            }
            ClassExpression::HandleProjection { handle, projection } => {
                let name = handle_projection_name(projection);
                let anchor = match anchor_name(self.identifiers, handle) {
                    Ok(anchor) => anchor,
                    Err(failure) => {
                        self.reject_name(failure, self.call.span);
                        return None;
                    }
                };
                let marker = match ref_marker_name(self.identifiers, handle) {
                    Ok(marker) => marker,
                    Err(failure) => {
                        self.reject_name(failure, self.call.span);
                        return None;
                    }
                };
                let (declaration, name_symbol) = match render_handle_projection(
                    self.identifiers,
                    handle,
                    projection,
                    self.source.path(),
                ) {
                    Ok(rendered) => rendered,
                    Err(failure) => {
                        self.reject_name(failure, self.call.span);
                        return None;
                    }
                };
                if let Some(symbol) = name_symbol {
                    self.written_name_symbols.push(symbol);
                }
                let utility = CompiledUtility {
                    required_properties: Vec::new(),
                    variants: Vec::new(),
                    variant_applications: Vec::new(),
                    declaration,
                    selector: None,
                    container: None,
                    nested: Vec::new(),
                    order: UtilityOrder::Authored(None),
                    candidate: String::new(),
                };
                if fold && guard.is_empty() {
                    self.units.base.canonical.push_str("handle:");
                    self.units.base.canonical.push_str(&anchor);
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
                    canonical: format!("handle:{anchor}:{name}\u{0}"),
                    utilities: vec![utility],
                    markers: Vec::new(),
                    external: Vec::new(),
                    plain: vec![marker],
                };
                match self.units.leaf(contribution, guard) {
                    Ok(leaf) => Some(leaf),
                    Err(failure) => {
                        self.reject_name(failure, self.call.span);
                        None
                    }
                }
            }
            ClassExpression::DynamicHandleProjection {
                value,
                handles,
                projection,
            } => {
                let mut table = Vec::new();
                for handle in handles {
                    self.runtime_assets.insert(ref_identity(handle));
                    let marker =
                        match runtime_projection_class(self.identifiers, handle, projection) {
                            Ok(marker) => marker,
                            Err(failure) => {
                                self.reject_name(failure, self.call.span);
                                return None;
                            }
                        };
                    let anchor = match anchor_name(self.identifiers, handle) {
                        Ok(anchor) => anchor,
                        Err(failure) => {
                            self.reject_name(failure, self.call.span);
                            return None;
                        }
                    };
                    let canonical = format!(
                        "dynamic-handle:{anchor}:{}",
                        handle_projection_name(projection)
                    );
                    let (declaration, name_symbol) = match render_handle_projection(
                        self.identifiers,
                        handle,
                        projection,
                        self.source.path(),
                    ) {
                        Ok(rendered) => rendered,
                        Err(failure) => {
                            self.reject_name(failure, self.call.span);
                            return None;
                        }
                    };
                    if let Some(symbol) = name_symbol {
                        self.written_name_symbols.push(symbol);
                    }
                    let contribution = LiteralContribution {
                        canonical,
                        utilities: vec![CompiledUtility {
                            required_properties: Vec::new(),
                            variants: Vec::new(),
                            variant_applications: Vec::new(),
                            declaration,
                            selector: None,
                            container: None,
                            nested: Vec::new(),
                            order: UtilityOrder::Authored(None),
                            candidate: String::new(),
                        }],
                        markers: Vec::new(),
                        external: Vec::new(),
                        plain: Vec::new(),
                    };
                    let class = match self.units.leaf(contribution, guard) {
                        Ok(class) => class,
                        Err(failure) => {
                            self.reject_name(failure, self.call.span);
                            return None;
                        }
                    };
                    let Emitted::Leaf { names } = class else {
                        return None;
                    };
                    let class = js_string(&names.join(" "));
                    table.push(format!("{}:{class}", js_string(&marker)));
                }
                let table = table.join(",");
                let expression = ValueExpr::Call {
                    helper: RuntimeHelper::HandleClass,
                    arguments: vec![
                        self.source_expression(*value)?,
                        format!("{{{table}}}").into(),
                    ],
                    span: *value,
                };
                Some(Emitted::Expression(expression))
            }
            ClassExpression::AnimationUse { animation, options } => {
                self.motion_assets.insert(animation.identity.clone());
                let plan = StyleVariantPlan::resolve(
                    self.identifiers,
                    wrappers,
                    self.theme,
                    self.diagnostics,
                )?;
                let mut utility = resolve_animation_use(
                    self.identifiers,
                    animation,
                    options,
                    self.theme,
                    self.diagnostics,
                );
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
                match self.units.leaf(contribution, guard) {
                    Ok(leaf) => Some(leaf),
                    Err(failure) => {
                        self.reject_name(failure, self.call.span);
                        None
                    }
                }
            }
            ClassExpression::DynamicAnimationUse {
                value,
                animations,
                options,
            } => {
                let plan = StyleVariantPlan::resolve(
                    self.identifiers,
                    wrappers,
                    self.theme,
                    self.diagnostics,
                )?;
                let mut table = Vec::new();
                for animation in animations {
                    self.runtime_assets.insert(animation.identity.clone());
                    let identity = match self.record_motion_name(
                        &animation.identity,
                        AssetKind::Animation,
                        animation.span,
                    ) {
                        Ok(identity) => identity,
                        Err(failure) => {
                            self.reject_name(failure, self.call.span);
                            return None;
                        }
                    };
                    let mut utility = resolve_animation_use(
                        self.identifiers,
                        animation,
                        options,
                        self.theme,
                        self.diagnostics,
                    );
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
                    let class = match self.units.leaf(contribution, guard) {
                        Ok(class) => class,
                        Err(failure) => {
                            self.reject_name(failure, self.call.span);
                            return None;
                        }
                    };
                    let Emitted::Leaf { names } = class else {
                        return None;
                    };
                    let class = js_string(&names.join(" "));
                    table.push(format!("\"{identity}\":{class}"));
                }
                let table = table.join(",");
                let expression = ValueExpr::Call {
                    helper: RuntimeHelper::AssetClass,
                    arguments: vec![
                        self.source_expression(*value)?,
                        format!("{{{table}}}").into(),
                    ],
                    span: *value,
                };
                Some(Emitted::Expression(expression))
            }
        }
    }

    /// Lowers a whole branch of a conditional expression.
    ///
    /// Visits every child with base folding disabled under the current guard/wrappers.
    /// Returns `None` for no children or one omitted child, the child for a singleton,
    /// and an array retaining empty slots otherwise. Later evaluations are never
    /// skipped because an earlier child was omitted; the pipeline owns diagnostics.
    fn emit_sequence(
        &mut self,
        expressions: &[ClassExpression],
        guard: &mut Vec<GuardStep>,
        wrappers: &mut Vec<StyleVariant>,
    ) -> Option<Emitted> {
        let mut items = Vec::new();
        for expression in expressions {
            items.push(self.emit_expression(expression, guard, wrappers, false));
        }
        match items.len() {
            0 => None,
            1 => items.pop().flatten(),
            _ => Some(Emitted::Array(items)),
        }
    }

    /// Captures the decoded custom-property name, original value text/span. Returns
    /// `None` if the source value cannot be sliced. CSS-value conversion and JavaScript
    /// quoting are deferred until a representation has been selected.
    fn render_custom_property(&self, property: &CustomPropertyEntry) -> Option<Property> {
        Some(Property {
            name: property.name.value.clone(),
            value: self.slice(property.value)?,
            span: property.value,
        })
    }

    /// Captures an ordinary property and its original value occurrence without conversion.
    /// An authored `style` stays an ordinary property. Returns `None` for an unreadable
    /// value span, without appending a diagnostic here.
    fn render_attribute(&self, attribute: &AttributeEntry) -> Option<Property> {
        Some(Property {
            name: attribute.name.value.clone(),
            value: self.slice(attribute.value)?,
            span: attribute.value,
        })
    }

    /// Contributes the class that carries a view transition's name and class properties.
    fn emit_view_transition_class(&mut self) {
        self.units.base.canonical.push_str("view-transition");
        self.units.base.canonical.push('\u{0}');
        self.units.base.utilities.push(view_transition_utility());
    }

    /// Copies one original expression with an exact occurrence mapping. Returns `None`
    /// if the span has a foreign file, invalid bounds, or UTF-8 boundaries.
    fn source_expression(&self, span: Span) -> Option<Code> {
        Some(Code::source(self.slice(span)?, span))
    }

    /// Copies a valid source slice verbatim, retaining authored whitespace. Returns
    /// `None` on a slicing error; this helper does not emit diagnostics.
    fn slice(&self, span: Span) -> Option<String> {
        self.source.slice(span).ok().map(str::to_owned)
    }
}

/// Source, analysis, and shared output destinations for lowering one style call.
/// Spans in the call must address `source`; destinations are appended
/// during lowering and are not individually rolled back on failure.
pub(super) struct CallEmitterInputs<'a> {
    /// Original buffer used to slice user expressions.
    pub source: &'a SourceFile,
    /// Compiler configuration for this call.
    pub options: &'a CompilerOptions,
    /// Name allocation shared with the rest of this compilation.
    pub identifiers: &'a IdentifierContext,
    /// Resolved theme and utility registry.
    pub theme: &'a ThemeIndex,
    /// Analyzed style call to lower.
    pub call: &'a StyleCall,
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
}

/// Returns an original-source UTF-8 byte anchor for sorting a call's contributions.
/// Conditionals use their test, variants and groups use their enclosing span, and
/// arrays use their first element or zero when empty. This establishes initial
/// contribution order; folding separately proves whether evaluations may move.
fn expression_position(expression: &ClassExpression) -> u32 {
    match expression {
        ClassExpression::Utilities(value) => value.span.start.0,
        ClassExpression::Array(elements) => elements.first().map_or(0, expression_position),
        ClassExpression::Conditional { test, .. } => test.start.0,
        ClassExpression::Variant { variant, .. } => match variant {
            StyleVariant::Prefix(prefix)
            | StyleVariant::Ref { prefix, .. }
            | StyleVariant::Container { query: prefix, .. } => prefix.span.start.0,
        },
        ClassExpression::Group { span, .. } | ClassExpression::External { span, .. } => {
            span.start.0
        }
        ClassExpression::HandleProjection { handle, .. } => handle.call_span.start.0,
        ClassExpression::AnimationUse { animation, .. } => animation.span.start.0,
        ClassExpression::DynamicHandleProjection { value, .. }
        | ClassExpression::DynamicAnimationUse { value, .. } => value.start.0,
    }
}
