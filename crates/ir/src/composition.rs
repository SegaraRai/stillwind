//! Composition graph validation; this graph never emits dependency CSS.
//!
//! A style call may derive from another call's payload (`style(base)`). The derived
//! call receives the base unit first, and the base must exist, must not carry
//! non-class props, and must use the same framework class property, because merging
//! happens at runtime on one payload.
//!
//! Identity is the declaration site: a call is keyed by ([`AssetKind::Style`], module
//! path, call span), so a re-export or a copy of a call does not inherit another call's
//! identity.
//!
//! This module only reports whether an ordering exists and which one it is. Emitting
//! the dependency's CSS and deciding ownership belong to `compiler` and `project`.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use stillwind_diagnostics::{CompileDiagnostic, CompileError, LintDiagnostic, LintFinding};
use stillwind_semantic::{AssetKind, IdentityId};
use stillwind_source::{ModuleId, Span};

use crate::{ClassExpression, GroupEntry, RuleExpression, ViewTransitionKey, ViewTransitionName};
use crate::{ModuleAnalysis, StyleCall, StyleInput};

/// One `derived` style call that consumes the payload of `base`.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct StyleDependency {
    /// The call that consumes another payload.
    pub derived: IdentityId,
    /// The call whose payload is consumed.
    pub base: IdentityId,
}

/// Every composition edge of one module, deduplicated and sorted by identity.
///
/// Returns an empty vector for a module whose calls are independent.
pub fn composition_dependencies(module: &ModuleAnalysis) -> Vec<StyleDependency> {
    let mut result = BTreeSet::new();
    for call in &module.ir.style_calls {
        let mut dependencies = BTreeSet::new();
        inputs(&call.inputs, &mut dependencies);
        result.extend(dependencies.into_iter().map(|base| StyleDependency {
            derived: identity(module, call),
            base,
        }));
    }
    result.into_iter().collect()
}

fn identity(module: &ModuleAnalysis, call: &StyleCall) -> IdentityId {
    IdentityId::new(
        AssetKind::Style,
        module.module_id.clone(),
        call.span.start.0,
        call.span.end.0,
        "",
    )
}

/// Checks every possible base and both style and producer-module cycles.
///
/// `require_complete` is set when the caller compiles a closed producer set, such as a
/// whole project: an unresolved base is then an error (`CompileDiagnostic::UnavailableStyleBase`). Rendering one
/// module in isolation passes `false`, because bases owned by other modules are
/// expected to arrive later.
///
/// Reports at most one error: the cycle check runs before the per-edge checks so a
/// cyclic graph is not also reported as a hundred missing bases.
pub fn validate_composition(
    modules: &[&ModuleAnalysis],
    at: Span,
    require_complete: bool,
) -> Vec<CompileError> {
    let mut edges = BTreeMap::new();
    let mut props = BTreeSet::new();
    let mut class_properties = BTreeMap::new();
    let mut module_edges: BTreeMap<ModuleId, BTreeSet<ModuleId>> = BTreeMap::new();
    for module in modules {
        for call in &module.ir.style_calls {
            let key = identity(module, call);
            class_properties.insert(key.clone(), module.ir.adapter.spec().class_property);
            let mut dependencies = BTreeSet::new();
            if inputs(&call.inputs, &mut dependencies) {
                props.insert(key.clone());
            }
            for dependency in &dependencies {
                if dependency.module_id != key.module_id {
                    module_edges
                        .entry(key.module_id.clone())
                        .or_default()
                        .insert(dependency.module_id.clone());
                    module_edges
                        .entry(dependency.module_id.clone())
                        .or_default();
                }
            }
            edges.insert(key, dependencies);
        }
    }
    let Some(order) = ordered(&edges) else {
        return vec![CompileError::new(CompileDiagnostic::CompositionCycle, at)];
    };
    if ordered(&module_edges).is_none() {
        return vec![CompileError::new(
            CompileDiagnostic::CompositionModuleCycle,
            at,
        )];
    }
    for key in order {
        for dependency in &edges[&key] {
            if require_complete && !edges.contains_key(dependency) {
                return vec![CompileError::new(
                    CompileDiagnostic::UnavailableStyleBase,
                    at,
                )];
            }
            if props.contains(dependency) {
                return vec![CompileError::new(CompileDiagnostic::StyleBaseWithProps, at)];
            }
            if class_properties
                .get(dependency)
                .is_some_and(|property| *property != class_properties[&key])
            {
                return vec![CompileError::new(
                    CompileDiagnostic::StyleBaseFrameworkMismatch,
                    at,
                )];
            }
        }
    }
    Vec::new()
}

/// Deterministic topological order within one producer, seeded by source order.
///
/// Returns the source order unchanged when the graph is cyclic; callers are expected to
/// have run [`validate_composition`], which rejects cycles, so this fallback keeps
/// rendering deterministic instead of dropping calls.
pub fn ordered_style_calls(module: &ModuleAnalysis) -> Vec<StyleCall> {
    let edges = module
        .ir
        .style_calls
        .iter()
        .map(|call| {
            let mut dependencies = BTreeSet::new();
            inputs(&call.inputs, &mut dependencies);
            (identity(module, call), dependencies)
        })
        .collect();
    let Some(order) = ordered(&edges) else {
        return module.ir.style_calls.clone();
    };
    let calls: BTreeMap<_, _> = module
        .ir
        .style_calls
        .iter()
        .map(|call| (identity(module, call), call))
        .collect();
    order
        .into_iter()
        .filter_map(|key| calls.get(&key).map(|call| (*call).clone()))
        .collect()
}

/// One element whose view-transition classes come from two calls.
///
/// The class property is one declaration, so the derived call's rules replace the list its base
/// declares: a transition only the base uses stops marking the element. The comparison is on
/// declarations rather than on the tokens they generate, so it is independent of name allocation,
/// and a transition both calls declare loses nothing. Only calls this compilation sees are
/// compared, which is why the finding is advisory: a base from another module stays unseen.
pub fn split_view_transition_classes(module: &ModuleAnalysis) -> Vec<LintFinding> {
    let declared: BTreeMap<IdentityId, BTreeSet<IdentityId>> = module
        .ir
        .style_calls
        .iter()
        .map(|call| (identity(module, call), view_transitions(&call.inputs)))
        .collect();
    let mut findings = Vec::new();
    for call in &module.ir.style_calls {
        let own = &declared[&identity(module, call)];
        if own.is_empty() {
            continue;
        }
        let replaces = composition_bases(&call.inputs).into_iter().any(|base| {
            declared
                .get(&base)
                .is_some_and(|classes| classes.difference(own).next().is_some())
        });
        if replaces {
            findings.push(LintFinding::new(
                LintDiagnostic::SplitViewTransitionClasses,
                call.span,
            ));
        }
    }
    findings
}

/// Every transition one input list uses, including the candidates of a runtime value.
fn view_transitions(values: &[StyleInput]) -> BTreeSet<IdentityId> {
    let mut transitions = BTreeSet::new();
    for input in values {
        match input {
            StyleInput::Array(inputs) => {
                transitions.extend(view_transitions(inputs));
            }
            StyleInput::Conditional { branches, .. } => {
                transitions.extend(view_transitions(branches.consequent()));
                if let Some(alternate) = branches.alternate() {
                    transitions.extend(view_transitions(alternate));
                }
            }
            StyleInput::ViewTransitionUse { transition, .. } => {
                transitions.insert(transition.identity.clone());
            }
            StyleInput::DynamicViewTransitionUse {
                transitions: candidates,
                ..
            } => {
                transitions.extend(
                    candidates
                        .iter()
                        .map(|transition| transition.identity.clone()),
                );
            }
            StyleInput::Class(expression) => {
                class_view_transitions(expression, &mut transitions);
            }
            _ => {}
        }
    }
    transitions
}

/// Adds all statically reachable view-transition identities in a root class
/// expression to `transitions`, including both arms of every ternary.
fn class_view_transitions(expression: &ClassExpression, transitions: &mut BTreeSet<IdentityId>) {
    match expression {
        ClassExpression::Rule(rule) => {
            rule_view_transitions(rule, transitions);
        }
        ClassExpression::Array(elements) => {
            for element in elements {
                class_view_transitions(element, transitions);
            }
        }
        ClassExpression::Conditional { branches, .. } => {
            for element in branches
                .consequent()
                .iter()
                .chain(branches.alternate().into_iter().flatten())
            {
                class_view_transitions(element, transitions);
            }
        }
        ClassExpression::Group { entries, .. } => {
            for entry in entries {
                if let GroupEntry::Class(element) = entry {
                    class_view_transitions(element, transitions);
                }
            }
        }
        ClassExpression::ClassValue { .. } => {}
    }
}

/// Adds all statically reachable view-transition identities in a rule subtree
/// to `transitions`, including finite runtime candidates.
fn rule_view_transitions(expression: &RuleExpression, transitions: &mut BTreeSet<IdentityId>) {
    match expression {
        RuleExpression::ViewTransitionUse { transition, .. } => {
            transitions.insert(transition.identity.clone());
        }
        RuleExpression::DynamicViewTransitionUse {
            transitions: candidates,
            ..
        } => {
            transitions.extend(
                candidates
                    .iter()
                    .map(|candidate| candidate.identity.clone()),
            );
        }
        RuleExpression::Array(elements)
        | RuleExpression::Group {
            inputs: elements, ..
        } => {
            for element in elements {
                rule_view_transitions(element, transitions);
            }
        }
        RuleExpression::Conditional { branches, .. } => {
            for element in branches
                .consequent()
                .iter()
                .chain(branches.alternate().into_iter().flatten())
            {
                rule_view_transitions(element, transitions);
            }
        }
        RuleExpression::Variant { inputs, .. } => {
            for element in inputs {
                rule_view_transitions(element, transitions);
            }
        }
        _ => {}
    }
}

/// The base calls one input list composes.
fn composition_bases(values: &[StyleInput]) -> BTreeSet<IdentityId> {
    let mut bases = BTreeSet::new();
    for input in values {
        match input {
            StyleInput::Array(inputs) => {
                bases.extend(composition_bases(inputs));
            }
            StyleInput::Composition { candidates, .. } => {
                bases.extend(candidates.iter().cloned());
            }
            StyleInput::Conditional { branches, .. } => {
                bases.extend(composition_bases(branches.consequent()));
                if let Some(alternate) = branches.alternate() {
                    bases.extend(composition_bases(alternate));
                }
            }
            _ => {}
        }
    }
    bases
}

fn inputs(values: &[StyleInput], dependencies: &mut BTreeSet<IdentityId>) -> bool {
    let mut props = false;
    for input in values {
        match input {
            StyleInput::Array(values) => {
                props |= inputs(values, dependencies);
            }
            StyleInput::Conditional { branches, .. } => {
                props |= inputs(branches.consequent(), dependencies);
                if let Some(alternate) = branches.alternate() {
                    props |= inputs(alternate, dependencies);
                }
            }
            StyleInput::Composition { candidates, .. } => {
                dependencies.extend(candidates.iter().cloned());
            }
            StyleInput::ViewTransitionUse { name, .. }
            | StyleInput::DynamicViewTransitionUse { name, .. } => {
                // A transition use marks the element with a class, which composes like any other
                // class; only a name the runtime composes travels as an inline property.
                props |= matches!(name, ViewTransitionName::Key(ViewTransitionKey::Dynamic(_)));
            }
            StyleInput::Class(expression) => props |= class_expression(expression),
            _ => {}
        }
    }
    props
}

/// Returns whether the expression carries non-class props (attributes).
fn class_expression(expression: &ClassExpression) -> bool {
    match expression {
        ClassExpression::Rule(rule) => rule_expression(rule),
        ClassExpression::Group { entries, .. } => entries.iter().any(|entry| match entry {
            GroupEntry::Class(expression) => class_expression(expression),
            GroupEntry::Attribute(_) | GroupEntry::CustomProperty(_) => true,
        }),
        ClassExpression::Array(elements) => elements.iter().any(class_expression),
        ClassExpression::Conditional { branches, .. } => branches
            .consequent()
            .iter()
            .chain(branches.alternate().into_iter().flatten())
            .any(class_expression),
        ClassExpression::ClassValue { .. } => false,
    }
}

/// Returns whether a rule subtree needs a runtime style property for a
/// view-transition name; CSS-only rules and static names return `false`.
fn rule_expression(expression: &RuleExpression) -> bool {
    match expression {
        RuleExpression::ViewTransitionUse { name, .. }
        | RuleExpression::DynamicViewTransitionUse { name, .. } => {
            matches!(name, ViewTransitionName::Key(ViewTransitionKey::Dynamic(_)))
        }
        RuleExpression::Array(elements)
        | RuleExpression::Group {
            inputs: elements, ..
        } => elements.iter().any(rule_expression),
        RuleExpression::Conditional { branches, .. } => branches
            .consequent()
            .iter()
            .chain(branches.alternate().into_iter().flatten())
            .any(rule_expression),
        RuleExpression::Variant { inputs, .. } => inputs.iter().any(rule_expression),
        _ => false,
    }
}

fn ordered<K: Clone + Ord>(edges: &BTreeMap<K, BTreeSet<K>>) -> Option<Vec<K>> {
    fn visit<K: Clone + Ord>(
        key: &K,
        edges: &BTreeMap<K, BTreeSet<K>>,
        pending: &mut BTreeSet<K>,
        done: &mut BTreeSet<K>,
        output: &mut Vec<K>,
    ) -> bool {
        if done.contains(key) {
            return true;
        }
        if !pending.insert(key.clone()) {
            return false;
        }
        for dependency in edges.get(key).into_iter().flatten() {
            if edges.contains_key(dependency) && !visit(dependency, edges, pending, done, output) {
                return false;
            }
        }
        pending.remove(key);
        done.insert(key.clone());
        output.push(key.clone());
        true
    }

    let mut output = Vec::new();
    let mut pending = BTreeSet::new();
    let mut done = BTreeSet::new();
    for key in edges.keys() {
        if !visit(key, edges, &mut pending, &mut done, &mut output) {
            return None;
        }
    }
    Some(output)
}
