//! Validates and groups the rule inputs supplied by object keys.

use oxc::ast::ast::Expression;
use oxc::span::GetSpan;

use stillwind_diagnostics::{LintDiagnostic, LintFinding, VariantInputErrorKind};
use stillwind_ir::{
    GroupEntry, RuleExpression, RuleExpressionError, RuleExpressionErrorKind, StyleInput,
    StyleVariant,
};
use stillwind_source::Span;

use crate::state::MacroExtractor;

/// A failed variant input conversion with the first incompatible input's kind and span.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct VariantInputError {
    reason: VariantInputErrorKind,
    span: Span,
}

impl From<RuleExpressionError> for VariantInputError {
    fn from(failure: RuleExpressionError) -> Self {
        let reason = match failure.reason {
            RuleExpressionErrorKind::Attribute => VariantInputErrorKind::Attribute,
            RuleExpressionErrorKind::CustomProperty => VariantInputErrorKind::CustomProperty,
            RuleExpressionErrorKind::ExternalClass => VariantInputErrorKind::ExternalClass,
        };
        Self {
            reason,
            span: failure.span,
        }
    }
}

impl<'a> MacroExtractor<'a> {
    /// Returns a rule entry when every descendant supports CSS variants. `variant`
    /// wraps the rules when present; `None` retains only the enclosing variants.
    /// Returns `None` for an empty value or after recording a lint for the first
    /// incompatible descendant, omitting the whole object property.
    pub(super) fn rule_entry(
        &mut self,
        variant: Option<StyleVariant>,
        value: &Expression<'a>,
    ) -> Option<GroupEntry> {
        let mut inputs = Vec::new();
        self.collect_style_expression(value, &mut inputs);
        if inputs.is_empty() {
            return None;
        }
        let inputs = match inputs
            .iter()
            .map(rule_class_input)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(inputs) => inputs,
            Err(failure) => {
                self.omit_input(value.span());
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::InvalidVariantInput {
                        reason: failure.reason,
                    },
                    failure.span,
                ));
                return None;
            }
        };
        let rule = match variant {
            Some(variant) => RuleExpression::Variant { variant, inputs },
            None => RuleExpression::Array(inputs),
        };
        Some(GroupEntry::Class(Box::new(rule.into())))
    }
}

/// Returns a rule subtree for a root input when every nested leaf supports CSS
/// variants. An incompatible descendant carries its own source range.
fn rule_class_input(input: &StyleInput) -> Result<RuleExpression, VariantInputError> {
    match input {
        StyleInput::Class(class) => RuleExpression::try_from(class.clone()).map_err(Into::into),
        StyleInput::Array(inputs) => Ok(RuleExpression::Array(
            inputs
                .iter()
                .map(rule_class_input)
                .collect::<Result<_, _>>()?,
        )),
        StyleInput::Conditional { test, branches } => Ok(RuleExpression::Conditional {
            test: *test,
            branches: branches.as_ref().try_map(|values| {
                values
                    .iter()
                    .map(rule_class_input)
                    .collect::<Result<_, _>>()
            })?,
        }),
        StyleInput::ViewTransitionUse { transition, name } => {
            Ok(RuleExpression::ViewTransitionUse {
                transition: transition.clone(),
                name: name.clone(),
            })
        }
        StyleInput::DynamicViewTransitionUse {
            value,
            transitions,
            name,
        } => Ok(RuleExpression::DynamicViewTransitionUse {
            value: *value,
            transitions: transitions.clone(),
            name: name.clone(),
        }),
        StyleInput::Composition { value, .. } => Err(VariantInputError {
            reason: VariantInputErrorKind::Composition,
            span: *value,
        }),
        StyleInput::RefAttachment { value, .. } => Err(VariantInputError {
            reason: VariantInputErrorKind::RefAttachment,
            span: *value,
        }),
        StyleInput::DynamicRefAttachment { value, .. } => Err(VariantInputError {
            reason: VariantInputErrorKind::DynamicRefAttachment,
            span: *value,
        }),
    }
}
