//! Class expression lowering: utilities, arrays, conditionals, variants, and groups.

use stillwind_diagnostics::{LintFinding, LintKind};
use stillwind_ir::{ClassExpression, ConditionalKind, GroupEntry, StyleVariant};

use super::super::fragment::{Emitted, EmittedEntry, ValueExpr};
use super::super::literal::LiteralContribution;
use super::super::props::{GuardStep, StyleEmissionPart};
use super::super::style_variants::{StyleVariantPlan, parse_prefix};
use super::super::variants::document_scoped_selector;
use super::CallEmitter;
use crate::runtime::RuntimeHelper;

impl CallEmitter<'_> {
    /// Reports a wrapper prefix whose variants would apply to the document instead of to the
    /// styled element.
    ///
    /// The check runs once per authored wrapper, so one wrapper covering several literals is
    /// reported once. The wrapper still lowers; the host decides the policy's severity.
    fn report_document_scoped_variant(&mut self, variant: &StyleVariant) {
        let StyleVariant::Prefix(prefix) = variant else {
            return;
        };
        if !self.theme.config.lint_enabled("require-element-scope") {
            return;
        }
        let Some(variants) = parse_prefix(prefix) else {
            return;
        };
        let theme = self.theme;
        if let Some(selector) = variants
            .iter()
            .find_map(|variant| document_scoped_selector(variant, theme))
        {
            self.lint_findings.push(LintFinding::new(
                LintKind::DocumentScopedSelector { selector },
                prefix.span,
            ));
        }
    }

    /// Lowers a class item under its enclosing input guards and an empty variant stack.
    /// Appends retained runtime code at `position`; omitted contributions may instead
    /// have joined the base unit or reported diagnostics during lowering.
    pub(super) fn emit_item(
        &mut self,
        expression: &ClassExpression,
        position: u32,
        inherited: &[GuardStep],
    ) {
        let mut wrappers = Vec::new();
        let mut guard = inherited.to_vec();
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
                let mut name_supplies = 0;
                let contribution = LiteralContribution::resolve(
                    value,
                    &plan,
                    self.options,
                    self.theme,
                    self.diagnostics,
                    self.lint_findings,
                    &mut name_supplies,
                );
                if name_supplies > 0 {
                    self.view_transition_name_paths.insert(guard, name_supplies);
                }
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
                self.report_document_scoped_variant(variant);
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
            ClassExpression::ClassValue { span, arguments } => {
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
            ClassExpression::RefProjection {
                definition,
                value,
                projection,
            } => self.emit_ref_projection(definition, value, projection, guard),
            ClassExpression::DynamicRefProjection {
                value,
                refs,
                projection,
            } => self.emit_dynamic_ref_projection(value, refs, projection, guard),
            ClassExpression::KeyframesUse { keyframes, options } => {
                self.emit_keyframes_use(keyframes, options, guard, wrappers, fold)
            }
            ClassExpression::DynamicKeyframesUse {
                value,
                keyframes,
                options,
            } => self.emit_dynamic_keyframes_use(value, keyframes, options, guard, wrappers),
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
}

/// Returns an original-source UTF-8 byte anchor for sorting a call's contributions.
///
/// Conditionals use their test, variants and groups use their enclosing span, and
/// arrays use their first element or zero when empty. This establishes initial
/// contribution order; folding separately proves whether evaluations may move.
pub(super) fn expression_position(expression: &ClassExpression) -> u32 {
    match expression {
        ClassExpression::Utilities(value) => value.span.start.0,
        ClassExpression::Array(elements) => elements.first().map_or(0, expression_position),
        ClassExpression::Conditional { test, .. } => test.start.0,
        ClassExpression::Variant { variant, .. } => match variant {
            StyleVariant::Prefix(prefix)
            | StyleVariant::Ref { prefix, .. }
            | StyleVariant::Container { query: prefix, .. } => prefix.span.start.0,
        },
        ClassExpression::Group { span, .. } | ClassExpression::ClassValue { span, .. } => {
            span.start.0
        }
        ClassExpression::RefProjection { value, .. }
        | ClassExpression::DynamicRefProjection { value, .. }
        | ClassExpression::DynamicKeyframesUse { value, .. } => value.start.0,
        ClassExpression::KeyframesUse { keyframes, .. } => keyframes.span.start.0,
    }
}
