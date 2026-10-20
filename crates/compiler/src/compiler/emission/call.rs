//! The walk over one call's style inputs.
//!
//! This file owns the entry point, the input dispatch, and the helpers every arm
//! shares: source slicing, diagnostics, deferred runtime parts, and the custom-property
//! captures. Class expressions and per-kind arms live in the sibling files.

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use std::collections::{BTreeMap, BTreeSet};

use stillwind_design_system::ThemeIndex;
use stillwind_diagnostics::{
    CompileDiagnostic, CompileError, LintFinding, LintSettings, SourceFailureContext,
};
use stillwind_ir::{
    AssetReference, ClassExpression, ConditionalBranches, CustomPropertyEntry, GroupEntry,
    StyleCall, StyleInput,
};
use stillwind_semantic::{AssetKind, IdentityId};
use stillwind_source::{ModuleId, SourceError, SourceFile, Span};

use super::super::CssContribution;
use super::super::class_units::{ClassUnits, PreviewSlots};
use super::super::fragment::{Emitted, Property, ValueExpr};
use super::super::literal::LiteralContribution;
use super::super::props::{GuardStep, StyleEmission, StyleEmissionPart};
use super::super::symbols::GeneratedSymbol;
use super::super::symbols::{definition_name_symbol, ref_identity, ref_marker_name};
use super::class_value::expression_position;
use super::ref_projection::ref_class_read;
use super::static_custom_properties;
use crate::generated::Code;
use crate::style_analysis::{
    DeclarationValue, RuleContext, RuleSubject, StyleEffect, StyleEffects, StyleSupply, SupplyKind,
    UseId,
};
use crate::{CompilerOptions, IdentifierContext};

/// One `style()` call being lowered.
///
/// The emitter accumulates state rather than building a tree in one pass, because the
/// base unit spans argument positions while each conditional region owns a generated
/// rule class and guard path.
pub(in crate::compiler) struct CallEmitter<'a> {
    /// The buffer the module's original text is sliced from.
    pub(super) source: &'a SourceFile,
    /// Module identity used for generated names.
    pub(super) module_id: &'a ModuleId,
    /// Name allocation shared with the rest of this compilation.
    pub(super) identifiers: &'a IdentifierContext,
    /// Resolved theme and utility registry for this compilation.
    pub(super) theme: &'a ThemeIndex,
    /// Effective lint settings of this compilation.
    pub(super) lint_settings: LintSettings,
    /// Whether runtime conditions lower to native CSS `if()` instead of fallback
    /// helpers.
    pub(super) use_css_if: bool,
    /// Analyzed style call being lowered.
    pub(super) call: &'a StyleCall,
    /// Compilation errors appended while lowering the call.
    pub(super) diagnostics: &'a mut Vec<CompileError>,
    /// Nonfatal findings appended in encounter order.
    pub(super) lint_findings: &'a mut Vec<LintFinding>,
    /// Identities referenced at runtime. The pipeline retains compiled identity
    /// records for these declarations and erases declarations without runtime uses.
    pub(super) runtime_assets: &'a mut BTreeSet<IdentityId>,
    /// Asset definitions whose CSS this module must emit. Static and dynamic uses
    /// record their possible definitions here, including `.for(...)` inputs.
    pub(super) required_definitions: &'a mut BTreeSet<IdentityId>,
    /// Runtime contributions accumulated for source-position ordering during rendering.
    pub(super) parts: Vec<StyleEmissionPart>,
    /// Rule-bearing contributions inside the current JavaScript condition region.
    pub(super) pending_region: Option<PendingRegion>,
    /// Owns class allocation and base-before-conditional CSS publication.
    pub(super) units: ClassUnits<'a>,
    /// Anchor and container names this call wrote, reported with the class symbols.
    pub(super) written_name_symbols: Vec<GeneratedSymbol>,
    /// Direct contributions with their conditions and base references.
    pub(super) style_effects: Vec<StyleEffect>,
    /// Unconditional inline literals with no possible competing write in this call.
    pub(super) static_custom_properties: BTreeMap<LeanStr, CustomPropertyEntry>,
}

impl<'a> CallEmitter<'a> {
    /// Lowers one analyzed call into finalized class units and ordered contributions.
    ///
    /// Input spans must address `source`. Shared CSS, symbol, asset, and diagnostic
    /// destinations may be extended during traversal. `None` means an input could not
    /// be lowered; `Some` is not proof of diagnostic-free compilation. The pipeline
    /// must inspect diagnostics and discard failed output transactionally.
    #[allow(clippy::needless_pass_by_value)] // The struct owns the borrows this call consumes.
    pub(in crate::compiler) fn run(inputs: CallEmitterInputs<'a>) -> Option<StyleEmission> {
        Self::run_with_properties(inputs, false)
    }

    /// Lowers a CVA value while retaining all custom properties in runtime fragments.
    /// The caller must validate that the inputs are within the CVA subset.
    pub(in crate::compiler) fn run_cva(inputs: CallEmitterInputs<'a>) -> Option<StyleEmission> {
        Self::run_with_properties(inputs, true)
    }

    fn run_with_properties(
        inputs: CallEmitterInputs<'a>,
        inline_properties: bool,
    ) -> Option<StyleEmission> {
        let emitter = Self {
            source: inputs.source,
            module_id: inputs.module_id,
            identifiers: inputs.identifiers,
            theme: inputs.theme,
            lint_settings: inputs.options.lint_settings(),
            use_css_if: inputs.options.use_css_if,
            call: inputs.call,
            diagnostics: inputs.diagnostics,
            lint_findings: inputs.lint_findings,
            runtime_assets: inputs.runtime_assets,
            required_definitions: inputs.required_definitions,
            parts: Vec::new(),
            pending_region: None,
            units: ClassUnits::new(
                inputs.module_id,
                inputs.options,
                inputs.identifiers,
                inputs.theme,
                inputs.call.span,
                inputs.preview,
                inputs.utility_css,
                inputs.symbols,
                inputs.emitted_utility_classes,
            ),
            written_name_symbols: Vec::new(),
            style_effects: Vec::new(),
            static_custom_properties: if inline_properties {
                BTreeMap::new()
            } else {
                static_custom_properties(&inputs.call.inputs)
            },
        };
        emitter.emit()
    }

    /// Traverses the call's inputs and finalizes classes, runtime parts, and direct
    /// effects. The final unconditional transition-class list is recorded after class
    /// units allocate slots. Input or class-unit failures return `None` and keep
    /// accumulated diagnostics and shared allocations. The pipeline must discard failed
    /// output.
    fn emit(mut self) -> Option<StyleEmission> {
        for input in self.call.inputs.clone() {
            self.emit_input(&input, &mut Vec::new())?;
        }
        let mut style_effects = StyleEffects {
            lint_settings: self.lint_settings.clone(),
            identity: IdentityId::new(
                AssetKind::Style,
                self.module_id.clone(),
                self.call.span.start.0,
                self.call.span.end.0,
                "",
            ),
            span: self.call.span,
            root: StyleEffect::All(std::mem::take(&mut self.style_effects)),
        };
        let parts = std::mem::take(&mut self.parts);
        let call_span = self.call.span;
        match self.units.finish() {
            Ok((static_names, classes, transition_list)) => {
                if !transition_list.is_empty()
                    && let StyleEffect::All(nodes) = &mut style_effects.root
                {
                    nodes.push(StyleEffect::Supply(StyleSupply {
                        site: UseId {
                            module: self.module_id.clone(),
                            span: call_span,
                        },
                        ordinal: 0,
                        kind: SupplyKind::Declaration {
                            property: "view-transition-class".into(),
                            value: DeclarationValue::Css((transition_list).into()),
                            important: false,
                            context: RuleContext {
                                selector: "&".into(),
                                at_rules: Vec::new(),
                                subject: RuleSubject::Target,
                            },
                            provider: false,
                            gate: None,
                        },
                    }));
                }
                Some(StyleEmission {
                    static_names,
                    style_effects,
                    parts,
                    classes,
                    call_span,
                    written_name_symbols: std::mem::take(&mut self.written_name_symbols),
                })
            }
            Err(failure) => {
                self.diagnostics.push(CompileError::new(failure, call_span));
                None
            }
        }
    }

    /// Dispatches one style input.
    ///
    /// Static utility contributions join class allocation; retained runtime values
    /// become parts with original-source ordering anchors and mappings. `None`
    /// propagates a required lowering/slicing failure; diagnostics and accumulated
    /// state remain owned by the pipeline, including failures represented by omitted
    /// nested contributions.
    #[allow(clippy::too_many_lines)] // One exhaustive match over the style inputs.
    fn emit_input(&mut self, input: &StyleInput, guard: &mut Vec<GuardStep>) -> Option<()> {
        match input {
            StyleInput::Array(inputs) => {
                if let Some(expression) = class_input(input) {
                    self.emit_item(&expression, expression_position(&expression), guard);
                } else {
                    for input in inputs {
                        self.emit_input(input, guard)?;
                    }
                }
            }
            StyleInput::Conditional { test, branches } => {
                if let Some(expression) = class_input(input) {
                    self.emit_item(&expression, test.start.0, guard);
                    return Some(());
                }
                let in_region = self.pending_region.is_some();
                if let Some(emitted) = self.end_region(guard) {
                    self.parts.push(StyleEmissionPart::Class {
                        emitted,
                        position: test.start.0,
                    });
                }
                let branches = match branches {
                    ConditionalBranches::And { consequent } => ConditionalBranches::And {
                        consequent: self.emit_branch(consequent, *test, true, guard)?,
                    },
                    ConditionalBranches::Ternary {
                        consequent,
                        alternate,
                    } => ConditionalBranches::Ternary {
                        consequent: self.emit_branch(consequent, *test, true, guard)?,
                        alternate: self.emit_branch(alternate, *test, false, guard)?,
                    },
                };
                if in_region {
                    self.begin_region();
                }
                let condition = self.source_expression(*test)?;
                self.parts.push(StyleEmissionPart::Conditional {
                    test: condition,
                    branches,
                    position: test.start.0,
                });
            }
            StyleInput::Class(ClassExpression::Group { entries, .. }) => {
                for entry in entries {
                    match entry {
                        GroupEntry::Class(expression) => {
                            let position = expression_position(expression);
                            self.emit_item(expression, position, guard);
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
                self.emit_item(expression, position, guard);
            }
            StyleInput::Composition { value, candidates } => {
                let bases = candidates
                    .iter()
                    .cloned()
                    .map(StyleEffect::Base)
                    .collect::<Vec<_>>();
                let effect = if bases.is_empty() {
                    StyleEffect::All(Vec::new())
                } else if bases.len() == 1 {
                    bases.into_iter().next()?
                } else {
                    StyleEffect::Choice {
                        site: UseId {
                            module: self.module_id.clone(),
                            span: *value,
                        },
                        alternatives: bases,
                    }
                };
                self.record_effect(effect, guard);
                let expression = self.source_expression(*value)?;
                self.parts.push(StyleEmissionPart::Base {
                    expression,
                    position: value.start.0,
                });
            }
            StyleInput::RefAttachment { definition, value } => {
                let marker = match ref_marker_name(self.identifiers, definition) {
                    Ok(marker) => marker,
                    Err(failure) => {
                        self.reject_name(failure, self.call.span);
                        return None;
                    }
                };
                let marker = marker.as_str().to_lean_str();
                self.record_supplies(
                    *value,
                    guard,
                    vec![SupplyKind::Marker(ref_identity(definition))],
                    None,
                );
                if !guard.is_empty() {
                    self.parts.push(StyleEmissionPart::Class {
                        emitted: Emitted::Leaf {
                            names: vec![(marker)],
                        },
                        position: value.start.0,
                    });
                    return Some(());
                }
                if !self.units.base.names.contains(&marker) {
                    self.units.base.names.push(marker);
                }
                self.units.base.canonical.push_str("ref:");
                self.units
                    .base
                    .canonical
                    .push_str(&definition.binding.value);
                self.units.base.canonical.push('\0');
            }
            StyleInput::DynamicRefAttachment { value, refs } => {
                self.runtime_assets.extend(refs.iter().map(ref_identity));
                for (index, definition) in refs.iter().enumerate() {
                    self.record_supplies(
                        *value,
                        guard,
                        vec![SupplyKind::Marker(ref_identity(definition))],
                        Some((index, refs.len())),
                    );
                }
                let expression = ref_class_read(self.source_expression(*value)?, *value);
                self.push_expression(expression, value.start.0);
            }
            StyleInput::ViewTransitionUse { transition, name } => {
                if let Some(emitted) =
                    self.emit_view_transition_expression(transition, name, guard, &[], true)
                {
                    self.parts.push(StyleEmissionPart::Class {
                        emitted,
                        position: transition.span.start.0,
                    });
                }
            }
            StyleInput::DynamicViewTransitionUse {
                value,
                transitions,
                name,
            } => {
                if let Some(emitted) = self.emit_dynamic_view_transition_expression(
                    value,
                    transitions,
                    name,
                    guard,
                    &[],
                ) {
                    self.parts.push(StyleEmissionPart::Class {
                        emitted,
                        position: value.start.0,
                    });
                }
            }
        }
        Some(())
    }

    /// Collects one branch's contributions with its inherited CSS and diagnostic guard
    /// path. Returns the branch-local parts in source order, or `None` if any required
    /// input fails to lower. Restores the caller's contribution and guard stacks in
    /// either case.
    fn emit_branch(
        &mut self,
        inputs: &[StyleInput],
        test: Span,
        when_true: bool,
        guard: &mut Vec<GuardStep>,
    ) -> Option<Vec<StyleEmissionPart>> {
        let outer = std::mem::take(&mut self.parts);
        guard.push(GuardStep { test, when_true });
        let outer_region = self.begin_region();
        let result = inputs
            .iter()
            .try_for_each(|input| self.emit_input(input, guard));
        let class = self.end_region(guard);
        self.pending_region = outer_region;
        guard.pop();
        let mut parts = std::mem::replace(&mut self.parts, outer);
        if let Some(emitted) = class {
            parts.insert(
                0,
                StyleEmissionPart::Class {
                    emitted,
                    position: test.start.0,
                },
            );
        }
        result.map(|()| parts)
    }

    /// Generates one definition's name and records it for pool validation.
    ///
    /// A definition name is written by every use of its declaration, so the report
    /// keeps one record per declaration and the pipeline drops later repeats.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure for the asset's family.
    pub(super) fn record_definition_name(
        &mut self,
        reference: &AssetReference,
        kind: AssetKind,
    ) -> Result<LeanString, CompileDiagnostic> {
        let symbol = definition_name_symbol(self.identifiers, reference, kind)?;
        let name = symbol.name.clone();
        self.written_name_symbols.push(symbol);
        Ok(name.to_lean_string())
    }

    /// Converts a generator failure into a fatal diagnostic at `span`.
    ///
    /// The failed call is omitted from the output, and the pipeline clears every
    /// generated artifact once a diagnostic exists, so a rejected name never reaches
    /// generated code.
    pub(super) fn reject_name(&mut self, failure: CompileDiagnostic, span: Span) {
        self.diagnostics.push(CompileError::new(failure, span));
    }

    /// Appends a deferred class value at an original-source UTF-8 ordering position.
    /// Records the expression without rendering it or registering its runtime helper.
    fn push_expression(&mut self, expression: ValueExpr, position: u32) {
        self.parts.push(StyleEmissionPart::Class {
            emitted: Emitted::Expression(expression),
            position,
        });
    }

    /// Captures the decoded custom-property name, original value text/span. Returns
    /// `None` if the source value cannot be sliced. CSS-value conversion and JavaScript
    /// quoting are deferred until a representation has been selected.
    pub(super) fn render_custom_property(
        &mut self,
        property: &CustomPropertyEntry,
    ) -> Option<Property> {
        Some(Property {
            static_value: (property.static_value.clone()),
            computed_value: None,
            name: (property.name.value.clone()),
            value: (self.slice(property.value)?).into(),
            span: property.value,
        })
    }

    /// Copies one original expression with an exact occurrence mapping. Invalid spans
    /// produce a fatal diagnostic at the containing call.
    pub(super) fn source_expression(&mut self, span: Span) -> Option<Code> {
        match self.source_expression_result(span) {
            Ok(code) => Some(code),
            Err(error) => {
                self.report_source_error(&error);
                None
            }
        }
    }

    /// Copies a source expression while retaining the slicing cause for callers that
    /// distinguish an optional expression from an unreadable one.
    pub(super) fn source_expression_result(&self, span: Span) -> Result<Code, SourceError> {
        Ok(Code::source(
            self.source.slice(span)?.to_lean_string(),
            span,
        ))
    }

    pub(super) fn report_source_error(&mut self, error: &SourceError) {
        self.diagnostics.push(CompileError::new(
            CompileDiagnostic::source_failure(error, SourceFailureContext::Emission),
            self.call.span,
        ));
    }

    /// Copies a valid source slice verbatim, retaining authored whitespace. Returns
    /// `None` on a slicing error after recording its typed cause at the call site.
    fn slice(&mut self, span: Span) -> Option<LeanString> {
        match self.source.slice(span) {
            Ok(value) => Some(value.to_lean_string()),
            Err(error) => {
                self.report_source_error(&error);
                None
            }
        }
    }
}

/// Returns a class subtree for a root input whose every descendant uses only the class
/// channel. Returns `None` when any descendant needs another channel.
fn class_input(input: &StyleInput) -> Option<ClassExpression> {
    match input {
        StyleInput::Class(expression) => Some(expression.clone()),
        StyleInput::Array(inputs) => Some(ClassExpression::Array(
            inputs.iter().map(class_input).collect::<Option<_>>()?,
        )),
        StyleInput::Conditional { test, branches } => Some(ClassExpression::Conditional {
            test: *test,
            branches: branches
                .as_ref()
                .try_map(|values| {
                    values
                        .iter()
                        .map(class_input)
                        .collect::<Option<_>>()
                        .ok_or(())
                })
                .ok()?,
        }),
        _ => None,
    }
}

/// CSS rules gathered between JavaScript condition boundaries before one class identity
/// is allocated for the region.
pub(super) struct PendingRegion {
    /// Source traversal order reserved before visiting the region's children.
    pub(super) order: u32,
    /// Rule-bearing contributions accumulated in encounter order.
    pub(super) rules: LiteralContribution,
}

/// Source, analysis, and shared output destinations for lowering one style call. Spans
/// in the call must address `source`; destinations are appended during lowering and are
/// not individually rolled back on failure.
pub(in crate::compiler) struct CallEmitterInputs<'a> {
    /// A present value replaces generated element selectors with `&` and captures or
    /// replays transition slots. Ref markers use the names supplied by `identifiers`.
    pub(in crate::compiler) preview: Option<&'a mut PreviewSlots>,
    /// Original buffer used to slice user expressions.
    pub(in crate::compiler) source: &'a SourceFile,
    /// Compiler identity of the source module.
    pub(in crate::compiler) module_id: &'a ModuleId,
    /// Compiler configuration for this call.
    pub(in crate::compiler) options: &'a CompilerOptions,
    /// Name allocation shared with the rest of this compilation.
    pub(in crate::compiler) identifiers: &'a IdentifierContext,
    /// Resolved theme and utility registry.
    pub(in crate::compiler) theme: &'a ThemeIndex,
    /// Analyzed style call to lower.
    pub(in crate::compiler) call: &'a StyleCall,
    /// Shared destination for compilation errors.
    pub(in crate::compiler) diagnostics: &'a mut Vec<CompileError>,
    /// Shared destination for lint findings, appended in encounter order.
    pub(in crate::compiler) lint_findings: &'a mut Vec<LintFinding>,
    /// Shared destination for module CSS contributions.
    pub(in crate::compiler) utility_css: &'a mut Vec<CssContribution>,
    /// Shared destination for generated symbol records.
    pub(in crate::compiler) symbols: &'a mut Vec<GeneratedSymbol>,
    /// Previously emitted (class name, canonical input) pairs used to suppress
    /// identical units.
    pub(in crate::compiler) emitted_utility_classes: &'a mut BTreeSet<(LeanStr, LeanStr)>,
    /// Referenced runtime identities used to retain declarations that need runtime
    /// values.
    pub(in crate::compiler) runtime_assets: &'a mut BTreeSet<IdentityId>,
    /// Definition identities whose CSS the module must emit, recorded by each use.
    pub(in crate::compiler) required_definitions: &'a mut BTreeSet<IdentityId>,
}
