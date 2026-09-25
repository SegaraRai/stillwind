//! The walk over one call's style inputs.
//!
//! This file owns the entry point, the input dispatch, and the helpers every arm
//! shares: source slicing, diagnostics, deferred runtime parts, and the attribute and
//! custom-property captures. Class expressions and per-kind arms live in the sibling
//! files.

use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_ir::{
    AssetReference, AttributeEntry, ClassExpression, ConditionalBranches, CustomPropertyEntry,
    GroupEntry, StyleInput,
};
use stillwind_semantic::AssetKind;
use stillwind_source::Span;

use super::super::class_units::ClassUnits;
use super::super::fragment::{Emitted, Property, ValueExpr};
use super::super::props::{GuardStep, StyleEmission, StyleEmissionPart};
use super::super::symbols::{definition_name_symbol, ref_identity, ref_marker_name};
use super::CallEmitter;
use super::CallEmitterInputs;
use super::class_value::expression_position;
use super::ref_projection::ref_class_read;
use super::view_transition::TransitionNamePaths;
use crate::generated::Code;

impl<'a> CallEmitter<'a> {
    /// Lowers one analyzed call into finalized class units and ordered contributions.
    ///
    /// Input spans must address `source`. Shared CSS, symbol, asset, and diagnostic
    /// destinations may be extended during traversal. `None` means an input could not
    /// be lowered; `Some` is not proof of diagnostic-free compilation. The pipeline
    /// must inspect diagnostics and discard failed output transactionally.
    #[allow(clippy::needless_pass_by_value)] // The struct owns the borrows this call consumes.
    pub(in crate::compiler) fn run(inputs: CallEmitterInputs<'a>) -> Option<StyleEmission> {
        let emitter = Self {
            source: inputs.source,
            options: inputs.options,
            identifiers: inputs.identifiers,
            theme: inputs.theme,
            call: inputs.call,
            diagnostics: inputs.diagnostics,
            lint_findings: inputs.lint_findings,
            runtime_assets: inputs.runtime_assets,
            required_definitions: inputs.required_definitions,
            parts: Vec::new(),
            pending_region: None,
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
            written_name_symbols: Vec::new(),
            view_transition_classes: false,
            view_transition_name_paths: TransitionNamePaths::default(),
        };
        emitter.emit()
    }

    /// Traverses top-level inputs and publishes accumulated class units on completion.
    /// Stops on an input returning `None`; shared output destinations are not rolled back.
    fn emit(mut self) -> Option<StyleEmission> {
        for input in self.call.inputs.clone() {
            self.emit_input(&input, &mut Vec::new())?;
        }
        self.report_view_transition_names();
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
                self.parts.push(StyleEmissionPart::Conditional {
                    test: self.source_expression(*test)?,
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
                self.emit_item(expression, position, guard);
            }
            StyleInput::Composition { value, .. } => {
                let expression = self.source_expression(*value)?;
                self.parts.push(StyleEmissionPart::Base {
                    expression,
                    position: value.start.0,
                });
            }
            StyleInput::RefAttachment(definition) => {
                let marker = match ref_marker_name(self.identifiers, definition) {
                    Ok(marker) => marker,
                    Err(failure) => {
                        self.reject_name(failure, self.call.span);
                        return None;
                    }
                };
                let marker = marker.as_str().to_owned();
                if !guard.is_empty() {
                    self.parts.push(StyleEmissionPart::Class {
                        emitted: Emitted::Leaf {
                            names: vec![marker],
                        },
                        position: definition.call_span.start.0,
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
                self.units.base.canonical.push('\u{0}');
            }
            StyleInput::DynamicRefAttachment { value, refs } => {
                self.runtime_assets.extend(refs.iter().map(ref_identity));
                let expression = ref_class_read(self.source_expression(*value)?);
                self.push_expression(expression, value.start.0);
            }
            StyleInput::ViewTransitionUse { transition, name } => {
                self.emit_view_transition_use(transition, name, guard)?;
            }
            StyleInput::DynamicViewTransitionUse {
                value,
                transitions,
                name,
            } => {
                self.emit_dynamic_view_transition_use(value, transitions, name, guard)?;
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
    /// A definition name is written by every use of its declaration, so the report keeps
    /// one record per declaration and the pipeline drops later repeats.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure for the asset's family.
    pub(super) fn record_definition_name(
        &mut self,
        reference: &AssetReference,
        kind: AssetKind,
    ) -> Result<String, CompileErrorKind> {
        let symbol = definition_name_symbol(self.identifiers, reference, kind)?;
        let name = symbol.name.clone();
        self.written_name_symbols.push(symbol);
        Ok(name)
    }

    /// Converts a generator failure into a fatal diagnostic at `span`.
    ///
    /// The failed call is omitted from the output, and the pipeline clears every
    /// generated artifact once a diagnostic exists, so a rejected name never reaches
    /// generated code.
    pub(super) fn reject_name(&mut self, failure: CompileErrorKind, span: Span) {
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
        &self,
        property: &CustomPropertyEntry,
    ) -> Option<Property> {
        Some(Property {
            static_value: property.static_value.clone(),
            computed_value: None,
            name: property.name.value.clone(),
            value: self.slice(property.value)?,
            span: property.value,
        })
    }

    /// Captures an ordinary property and its original value occurrence without
    /// conversion. An authored `style` stays an ordinary property. Returns `None` for
    /// an unreadable value span, without appending a diagnostic here.
    pub(super) fn render_attribute(&self, attribute: &AttributeEntry) -> Option<Property> {
        Some(Property {
            static_value: None,
            computed_value: None,
            name: attribute.name.value.clone(),
            value: self.slice(attribute.value)?,
            span: attribute.value,
        })
    }

    /// Copies one original expression with an exact occurrence mapping. Returns `None`
    /// if the span has a foreign file, invalid bounds, or UTF-8 boundaries.
    pub(super) fn source_expression(&self, span: Span) -> Option<Code> {
        Some(Code::source(self.slice(span)?, span))
    }

    /// Copies a valid source slice verbatim, retaining authored whitespace. Returns
    /// `None` on a slicing error; this helper does not emit diagnostics.
    fn slice(&self, span: Span) -> Option<String> {
        self.source.slice(span).ok().map(str::to_owned)
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
