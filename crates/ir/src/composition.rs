//! Recovery and ordering of dependencies between composed style calls.
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
//! `composition_dependencies` collects declaration edges for host planning.
//! `recover_composition` removes incompatible operands with lint findings.
//! `validate_composition` checks cycles and producer availability.
//! `ordered_style_calls` puts local bases before their consumers during lowering.
//! The compiler resolves declaration supplies for lint checks, while the project layer
//! chooses CSS ownership and requires selected base producers to be available.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use stillwind_diagnostics::{CompileDiagnostic, CompileError, LintDiagnostic, LintFinding};
use stillwind_semantic::{AssetKind, IdentityId};
use stillwind_source::{ModuleId, Span};

use crate::{
    ClassExpression, ConditionalBranches, GroupEntry, ModuleAnalysis, RuleExpression, StyleCall,
    StyleInput, ViewTransitionKey, ViewTransitionName,
};

/// One `derived` style call that consumes the payload of `base`.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

/// Reports cycles between calls or modules. Also reports missing producers when
/// `modules` contains the complete composition graph.
/// Local compilation passes `require_complete: false` because imported producers may
/// arrive later. Recoverable composition inputs are removed by [`recover_composition`].
pub fn validate_composition(
    modules: &[&ModuleAnalysis],
    at: Span,
    require_complete: bool,
) -> Vec<CompileError> {
    let graph = CompositionGraph::new(modules);
    if ordered(&graph.edges).is_none() {
        return vec![CompileError::new(CompileDiagnostic::CompositionCycle, at)];
    }
    if ordered(&graph.modules).is_none() {
        return vec![CompileError::new(
            CompileDiagnostic::CompositionModuleCycle,
            at,
        )];
    }
    if !require_complete {
        return Vec::new();
    }
    let produced: BTreeSet<_> = modules
        .iter()
        .flat_map(|module| {
            module
                .ir
                .style_calls
                .iter()
                .map(|call| identity(module, call))
        })
        .collect();
    if modules
        .iter()
        .flat_map(|module| composition_dependencies(module))
        .any(|dependency| !produced.contains(&dependency.base))
    {
        return vec![CompileError::new(
            CompileDiagnostic::UnavailableStyleBase,
            at,
        )];
    }
    Vec::new()
}

/// Omits incompatible composition operands while keeping their producers.
/// Each omitted operand adds a lint at its source expression. Reporting settings do
/// not affect omission. Missing producers are left for [`validate_composition`].
pub fn recover_composition(modules: &mut [ModuleAnalysis]) {
    let graph = CompositionGraph::new(&modules.iter().collect::<Vec<_>>());
    let mut rejected = BTreeMap::new();
    for (derived, bases) in &graph.edges {
        for base in bases {
            let reason = if graph.props.contains(base) {
                Some(LintDiagnostic::StyleBaseWithProps)
            } else if graph
                .frameworks
                .get(base)
                .is_some_and(|property| *property != graph.frameworks[derived])
            {
                Some(LintDiagnostic::StyleBaseFrameworkMismatch)
            } else {
                None
            };
            if let Some(reason) = reason {
                rejected.insert(
                    StyleDependency {
                        derived: derived.clone(),
                        base: base.clone(),
                    },
                    reason,
                );
            }
        }
    }
    for module in modules.iter_mut() {
        let module_id = module.module_id.clone();
        for call in &mut module.ir.style_calls {
            let derived = IdentityId::new(
                AssetKind::Style,
                module_id.clone(),
                call.span.start.0,
                call.span.end.0,
                "",
            );
            omit_composition(
                &mut call.inputs,
                &derived,
                &rejected,
                &mut module.lint_findings,
            );
        }
    }
}

struct CompositionGraph {
    edges: BTreeMap<IdentityId, BTreeSet<IdentityId>>,
    props: BTreeSet<IdentityId>,
    frameworks: BTreeMap<IdentityId, &'static str>,
    modules: BTreeMap<ModuleId, BTreeSet<ModuleId>>,
}

impl CompositionGraph {
    fn new(modules: &[&ModuleAnalysis]) -> Self {
        let mut graph = Self {
            edges: BTreeMap::new(),
            props: BTreeSet::new(),
            frameworks: BTreeMap::new(),
            modules: BTreeMap::new(),
        };
        for module in modules {
            for call in &module.ir.style_calls {
                let derived = identity(module, call);
                let mut dependencies = BTreeSet::new();
                if inputs(&call.inputs, &mut dependencies) {
                    graph.props.insert(derived.clone());
                }
                graph
                    .frameworks
                    .insert(derived.clone(), module.ir.adapter.spec().class_property);
                for base in &dependencies {
                    if base.module_id() != derived.module_id() {
                        graph
                            .modules
                            .entry(derived.module_id().clone())
                            .or_default()
                            .insert(base.module_id().clone());
                        graph.modules.entry(base.module_id().clone()).or_default();
                    }
                }
                graph.edges.insert(derived, dependencies);
            }
        }
        graph
    }
}

fn omit_composition(
    values: &mut Vec<StyleInput>,
    derived: &IdentityId,
    rejected: &BTreeMap<StyleDependency, LintDiagnostic>,
    findings: &mut Vec<LintFinding>,
) {
    values.retain_mut(|input| {
        match input {
            StyleInput::Array(values) => omit_composition(values, derived, rejected, findings),
            StyleInput::Conditional { branches, .. } => match branches {
                ConditionalBranches::And { consequent } => {
                    omit_composition(consequent, derived, rejected, findings);
                }
                ConditionalBranches::Ternary {
                    consequent,
                    alternate,
                } => {
                    omit_composition(consequent, derived, rejected, findings);
                    omit_composition(alternate, derived, rejected, findings);
                }
            },
            StyleInput::Composition { candidates, value } => {
                if let Some(reason) = candidates.iter().find_map(|base| {
                    rejected.get(&StyleDependency {
                        derived: derived.clone(),
                        base: base.clone(),
                    })
                }) {
                    findings.push(LintFinding::new(reason.clone(), *value));
                    return false;
                }
            }
            _ => {}
        }
        true
    });
}

/// Deterministic topological order within one producer, seeded by source order.
///
/// Callers must run [`recover_composition`] and [`validate_composition`] first.
/// If a cycle remains, this function retains source order for deterministic diagnostics.
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
