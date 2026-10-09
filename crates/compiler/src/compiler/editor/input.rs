//! Selects one source contribution while retaining its enclosing IR conditions.

use std::collections::BTreeSet;

use stillwind_ir::{
    ClassExpression, ConditionalBranches, GroupEntry, KeyframesBody, RuleExpression, StyleInput,
    StyleVariant,
};
use stillwind_semantic::IdentityId;
use stillwind_source::Span;

use super::EditorPlan;
use crate::CandidateContext;

/// Original occurrence span, optional literal context, and the isolated input that
/// retains its enclosing conditions. A missing context identifies an expression use.
type InputPlan = (Span, Option<CandidateContext>, StyleInput);

/// Original occurrence span, optional literal context, and the isolated class
/// expression.
type ClassPlan = (Span, Option<CandidateContext>, ClassExpression);

/// Original occurrence span, optional literal context, and the isolated rule
/// expression.
type RulePlan = (Span, Option<CandidateContext>, RuleExpression);

/// Selects each recognized literal or use in traversal order, retaining enclosing
/// variants and JavaScript conditions. Spans address the original `input`; unsupported
/// inputs contribute no plans. The original IR remains unchanged.
pub(super) fn input_plans(input: &StyleInput) -> Vec<InputPlan> {
    match input {
        StyleInput::Class(class) => class_plans(class)
            .into_iter()
            .map(|(span, context, class)| (span, context, StyleInput::Class(class)))
            .collect(),
        StyleInput::Array(inputs) => inputs.iter().flat_map(input_plans).collect(),
        StyleInput::Conditional { test, branches } => branch_plans(branches, input_plans)
            .into_iter()
            .map(|(span, context, branches)| {
                (
                    span,
                    context,
                    StyleInput::Conditional {
                        test: *test,
                        branches,
                    },
                )
            })
            .collect(),
        StyleInput::ViewTransitionUse { transition, .. } => {
            vec![(transition.span, None, input.clone())]
        }
        StyleInput::DynamicViewTransitionUse { value, .. }
        | StyleInput::Composition { value, .. } => vec![(*value, None, input.clone())],
        _ => Vec::new(),
    }
}

/// Adds every finite style composition candidate to `dependencies`, including both
/// conditional branches. Existing entries remain present; the set deduplicates
/// identities.
pub(super) fn collect_bases(input: &StyleInput, dependencies: &mut BTreeSet<IdentityId>) {
    match input {
        StyleInput::Composition { candidates, .. } => {
            dependencies.extend(candidates.iter().cloned());
        }
        StyleInput::Array(inputs) => {
            for input in inputs {
                collect_bases(input, dependencies);
            }
        }
        StyleInput::Conditional { branches, .. } => {
            for input in branches
                .consequent()
                .iter()
                .chain(branches.alternate().into_iter().flatten())
            {
                collect_bases(input, dependencies);
            }
        }
        _ => {}
    }
}

/// Selects rule-bearing occurrences inside `class` in traversal order. Conditional
/// plans retain the test and branch shape; class values and group text have no plans.
fn class_plans(class: &ClassExpression) -> Vec<ClassPlan> {
    match class {
        ClassExpression::Rule(rule) => rule_plans(rule)
            .into_iter()
            .map(|(span, context, rule)| (span, context, ClassExpression::Rule(Box::new(rule))))
            .collect(),
        ClassExpression::Array(classes) => classes.iter().flat_map(class_plans).collect(),
        ClassExpression::Conditional { test, branches } => branch_plans(branches, class_plans)
            .into_iter()
            .map(|(span, context, branches)| {
                (
                    span,
                    context,
                    ClassExpression::Conditional {
                        test: *test,
                        branches,
                    },
                )
            })
            .collect(),
        ClassExpression::Group { entries, .. } => entries
            .iter()
            .flat_map(|entry| match entry {
                GroupEntry::Class(class) => class_plans(class),
                _ => Vec::new(),
            })
            .collect(),
        ClassExpression::ClassValue { .. } => Vec::new(),
    }
}

/// Selects utility literals, animation options, and rule-bearing uses inside `rule`.
/// Each plan keeps its enclosing variants and conditional branch. Ref and transition
/// uses address their receiver; anchor scopes address the whole call.
fn rule_plans(rule: &RuleExpression) -> Vec<RulePlan> {
    match rule {
        RuleExpression::Utilities(input) => {
            vec![(input.span, Some(CandidateContext::Utility), rule.clone())]
        }
        RuleExpression::Array(inputs) | RuleExpression::Group { inputs, .. } => {
            inputs.iter().flat_map(rule_plans).collect()
        }
        RuleExpression::Conditional { test, branches } => branch_plans(branches, rule_plans)
            .into_iter()
            .map(|(span, context, branches)| {
                (
                    span,
                    context,
                    RuleExpression::Conditional {
                        test: *test,
                        branches,
                    },
                )
            })
            .collect(),
        RuleExpression::Variant { variant, inputs } => inputs
            .iter()
            .flat_map(rule_plans)
            .map(|(span, context, input)| {
                (
                    span,
                    context,
                    RuleExpression::Variant {
                        variant: variant.clone(),
                        inputs: vec![input],
                    },
                )
            })
            .collect(),
        RuleExpression::AnchorScope { span, .. } => vec![(*span, None, rule.clone())],
        RuleExpression::RefProjection { value, .. }
        | RuleExpression::DynamicRefProjection { value, .. }
        | RuleExpression::DynamicViewTransitionUse { value, .. } => {
            vec![(*value, None, rule.clone())]
        }
        RuleExpression::ViewTransitionUse { transition, .. } => {
            vec![(transition.span, None, rule.clone())]
        }
        RuleExpression::KeyframesUse { keyframes, options } => vec![
            (keyframes.span, None, rule.clone()),
            (
                options.span,
                Some(CandidateContext::AnimationOptions),
                rule.clone(),
            ),
        ],
        RuleExpression::DynamicKeyframesUse { value, options, .. } => vec![
            (*value, None, rule.clone()),
            (
                options.span,
                Some(CandidateContext::AnimationOptions),
                rule.clone(),
            ),
        ],
    }
}

/// Wraps each plan returned by `plans` in the original conditional shape, retaining it
/// in its original branch and leaving the other branch empty. Consequent plans precede
/// alternate plans. Neither the original branches nor their values are changed.
fn branch_plans<T: Clone>(
    branches: &ConditionalBranches<Vec<T>>,
    plans: impl Fn(&T) -> Vec<(Span, Option<CandidateContext>, T)>,
) -> Vec<(Span, Option<CandidateContext>, ConditionalBranches<Vec<T>>)> {
    let mut result = Vec::new();
    for (when_true, values) in [
        (true, Some(branches.consequent())),
        (false, branches.alternate()),
    ] {
        for value in values.into_iter().flatten() {
            for (span, context, value) in plans(value) {
                let consequent = if when_true {
                    vec![value.clone()]
                } else {
                    Vec::new()
                };
                let branches = match branches {
                    ConditionalBranches::And { .. } => ConditionalBranches::And { consequent },
                    ConditionalBranches::Ternary { .. } => ConditionalBranches::Ternary {
                        consequent,
                        alternate: if when_true { Vec::new() } else { vec![value] },
                    },
                };
                result.push((span, context, branches));
            }
        }
    }
    result
}

/// Replaces decoded literal text at the exact original `span` inside `plan`.
/// `candidate` may contain a complete list of tokens. Spans and enclosing IR remain
/// unchanged; validation occurs during emission. A missing span leaves the plan
/// unchanged.
pub(super) fn replace_text(plan: &mut EditorPlan, span: Span, candidate: &str) {
    match plan {
        EditorPlan::Keyframes(definition) => {
            replace_frame_text(&mut definition.body, span, candidate);
        }
        EditorPlan::Transition(definition) => {
            for input in [
                &mut definition.group,
                &mut definition.old,
                &mut definition.new,
                &mut definition.image_pair,
            ]
            .into_iter()
            .flatten()
            {
                if input.span == span {
                    candidate.clone_into(&mut input.value);
                }
            }
        }
        EditorPlan::Call(call) => {
            for input in &mut call.inputs {
                replace_input_text(input, span, candidate);
            }
        }
    }
}

/// Replaces frame utilities or condition prefixes that exactly match `span`,
/// recursively visiting branches. Only their decoded text changes; original spans
/// remain intact.
fn replace_frame_text(body: &mut KeyframesBody, span: Span, candidate: &str) {
    for frame in &mut body.frames {
        if frame.utilities.span == span {
            candidate.clone_into(&mut frame.utilities.value);
        }
    }
    for branch in &mut body.branches {
        if let StyleVariant::Prefix(prefix) = &mut branch.variant
            && prefix.span == span
        {
            candidate.clone_into(&mut prefix.value);
        }
        replace_frame_text(&mut branch.body, span, candidate);
    }
}

/// Replaces matching rule literals inside classes, arrays, and both conditional
/// branches. Uses that contain no editable literal remain unchanged.
fn replace_input_text(input: &mut StyleInput, span: Span, candidate: &str) {
    match input {
        StyleInput::Class(class) => {
            replace_class_text(class, span, candidate);
        }
        StyleInput::Array(inputs) => {
            for input in inputs {
                replace_input_text(input, span, candidate);
            }
        }
        StyleInput::Conditional { branches, .. } => {
            visit_branches(branches, |input| replace_input_text(input, span, candidate));
        }
        _ => {}
    }
}

/// Replaces matching rule literals inside `class`, including conditional and group
/// entries. Plain class values and non-class group entries remain unchanged.
fn replace_class_text(class: &mut ClassExpression, span: Span, candidate: &str) {
    match class {
        ClassExpression::Rule(rule) => {
            replace_rule_text(rule, span, candidate);
        }
        ClassExpression::Array(classes) => {
            for class in classes {
                replace_class_text(class, span, candidate);
            }
        }
        ClassExpression::Conditional { branches, .. } => {
            visit_branches(branches, |class| replace_class_text(class, span, candidate));
        }
        ClassExpression::Group { entries, .. } => {
            for entry in entries {
                if let GroupEntry::Class(class) = entry {
                    replace_class_text(class, span, candidate);
                }
            }
        }
        ClassExpression::ClassValue { .. } => {}
    }
}

/// Replaces decoded utilities or animation options whose span equals `span`.
/// Recursion preserves rule wrappers, tests, identities, and original source ranges.
fn replace_rule_text(rule: &mut RuleExpression, span: Span, candidate: &str) {
    match rule {
        RuleExpression::Utilities(input) => {
            if input.span == span {
                candidate.clone_into(&mut input.value);
            }
        }
        RuleExpression::KeyframesUse { options, .. }
        | RuleExpression::DynamicKeyframesUse { options, .. } => {
            if options.span == span {
                candidate.clone_into(&mut options.value);
            }
        }
        RuleExpression::Array(inputs)
        | RuleExpression::Variant { inputs, .. }
        | RuleExpression::Group { inputs, .. } => {
            for input in inputs {
                replace_rule_text(input, span, candidate);
            }
        }
        RuleExpression::Conditional { branches, .. } => {
            visit_branches(branches, |input| replace_rule_text(input, span, candidate));
        }
        _ => {}
    }
}

/// Applies `visit` to consequent entries followed by alternate entries, without
/// changing the conditional shape. The visitor may mutate each entry in place.
fn visit_branches<T>(branches: &mut ConditionalBranches<Vec<T>>, mut visit: impl FnMut(&mut T)) {
    match branches {
        ConditionalBranches::And { consequent } => {
            for item in consequent {
                visit(item);
            }
        }
        ConditionalBranches::Ternary {
            consequent,
            alternate,
        } => {
            for item in consequent.iter_mut().chain(alternate) {
                visit(item);
            }
        }
    }
}
