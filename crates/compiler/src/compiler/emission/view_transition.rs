//! View-transition uses: the class an element carries, the name it states, and the
//! conditional class a guarded use adds.

use std::collections::BTreeMap;

use stillwind_diagnostics::{LintFinding, LintKind};
use stillwind_ir::{
    AssetReference, ConditionalKind, StyleGuard, ViewTransitionKey, ViewTransitionName,
};
use stillwind_semantic::AssetKind;
use stillwind_source::Span;

use super::super::fragment::{Emitted, ValueExpr};
use super::super::literal::LiteralContribution;
use super::super::props::{StyleEmissionPart, guard_steps};
use super::{CallEmitter, VIEW_TRANSITION_CLASS_KEY};
use crate::generated::js_string;
use crate::identifiers::view_transition_name;
use crate::runtime::RuntimeHelper;

/// Name supplies grouped by shared conditional paths, so exclusive arms count once.
#[derive(Default)]
pub(super) struct TransitionNamePaths {
    count: usize,
    branches: BTreeMap<(u32, u32), [Self; 2]>,
}

impl TransitionNamePaths {
    fn insert(&mut self, guard: &[StyleGuard]) {
        let mut node = self;
        for step in guard {
            node = &mut node
                .branches
                .entry((step.test.start.0, step.test.end.0))
                .or_default()[usize::from(step.when_true)];
        }
        node.count += 1;
    }

    fn maximum(&self) -> usize {
        self.count
            + self
                .branches
                .values()
                .map(|arms| arms[0].maximum().max(arms[1].maximum()))
                .sum::<usize>()
    }
}

impl CallEmitter<'_> {
    /// Lowers one view-transition use whose transition is known at compile time.
    ///
    /// An empty guard folds the class into the base unit; a guarded use takes its own class
    /// unit so the condition selects the class rather than the declarations.
    pub(super) fn emit_view_transition_use(
        &mut self,
        transition: &AssetReference,
        name: &ViewTransitionName,
        guard: &[StyleGuard],
    ) -> Option<()> {
        self.required_definitions
            .insert(transition.identity.clone());
        self.runtime_assets.insert(transition.identity.clone());
        self.view_transition_classes = true;
        if !matches!(name, ViewTransitionName::None) {
            self.view_transition_name_paths.insert(guard);
        }
        let token = match self.record_definition_name(transition, AssetKind::ViewTransition) {
            Ok(token) => token,
            Err(failure) => {
                self.reject_name(failure, self.call.span);
                return None;
            }
        };
        let static_name = static_view_transition_name(name, &token);
        if guard.is_empty() {
            self.units.fold(LiteralContribution {
                canonical: format!("view-transition:{token}\u{0}"),
                utilities: Vec::new(),
                markers: Vec::new(),
                external: Vec::new(),
                plain: Vec::new(),
                view_transition_token: Some(token.clone()),
                view_transition_name: static_name,
            });
        } else {
            let steps = guard_steps(guard);
            let class =
                match self
                    .units
                    .view_transition_class_unit(&token, static_name.as_deref(), &steps)
                {
                    Ok(class) => class,
                    Err(failure) => {
                        self.reject_name(failure, self.call.span);
                        return None;
                    }
                };
            self.push_guarded_class(guard, class, transition.span.start.0);
        }
        self.emit_view_transition_name(name, Some(&token), transition.span, guard);
        Some(())
    }

    /// Lowers one view-transition use whose transition is only known at runtime.
    ///
    /// The emitted lookup maps each candidate token to its class set; a name the use states
    /// becomes a style property that the same runtime value selects from.
    pub(super) fn emit_dynamic_view_transition_use(
        &mut self,
        value: &Span,
        transitions: &[AssetReference],
        name: &ViewTransitionName,
        guard: &[StyleGuard],
    ) -> Option<()> {
        self.view_transition_classes = true;
        if !matches!(name, ViewTransitionName::None) {
            self.view_transition_name_paths.insert(guard);
        }
        let mut table = Vec::new();
        let steps = guard_steps(guard);
        for transition in transitions {
            self.runtime_assets.insert(transition.identity.clone());
            self.required_definitions
                .insert(transition.identity.clone());
            let token = match self.record_definition_name(transition, AssetKind::ViewTransition) {
                Ok(token) => token,
                Err(failure) => {
                    self.reject_name(failure, self.call.span);
                    return None;
                }
            };
            let static_name = static_view_transition_name(name, &token);
            let class =
                match self
                    .units
                    .view_transition_class_unit(&token, static_name.as_deref(), &steps)
                {
                    Ok(class) => class,
                    Err(failure) => {
                        self.reject_name(failure, self.call.span);
                        return None;
                    }
                };
            table.push(format!("{}:{}", js_string(&token), js_string(&class)));
        }
        let expression = ValueExpr::Lookup {
            value: Box::new(ValueExpr::Source(self.source_expression(*value)?)),
            key: VIEW_TRANSITION_CLASS_KEY.to_owned(),
            table: format!("{{{}}}", table.join(",")).into(),
        };
        self.parts.push(StyleEmissionPart::Class {
            emitted: self.guarded_class(guard, Emitted::Expression(expression)),
            position: value.start.0,
        });
        self.emit_view_transition_name(name, None, *value, guard);
        Some(())
    }

    /// Reports a call whose view-transition name supply is missing or duplicated.
    ///
    /// Participation needs exactly one name per element, and the compiler sees only the
    /// supplies the call itself states: a name another call, a stylesheet, or a user inline
    /// style supplies stays invisible, so the finding is advisory rather than fatal.
    pub(super) fn report_view_transition_names(&mut self) {
        // TODO: report a repeated render or an unstable key. The call alone cannot show it: a
        // literal name inside `items.map((item) => …)` marks every element identically, so the
        // finding needs the loop or callback the frontend sees, which the IR does not carry.
        if !self.view_transition_classes {
            return;
        }
        let kind = match self.view_transition_names + self.view_transition_name_paths.maximum() {
            0 => LintKind::MissingViewTransitionName,
            1 => return,
            count => LintKind::DuplicateViewTransitionName { count },
        };
        self.lint_findings
            .push(LintFinding::new(kind, self.call.span));
    }

    /// Adds the inline name a use writes when its key is not a literal.
    ///
    /// A name the use site states statically becomes a declaration in the class; one built
    /// from an expression, or read from the value the transition travelled in, becomes a
    /// style property instead.
    fn emit_view_transition_name(
        &mut self,
        name: &ViewTransitionName,
        token: Option<&str>,
        value: Span,
        guard: &[StyleGuard],
    ) {
        let key = match name {
            ViewTransitionName::Key(ViewTransitionKey::Dynamic(key)) => Some(*key),
            ViewTransitionName::Value => None,
            ViewTransitionName::None | ViewTransitionName::Key(ViewTransitionKey::Literal(_)) => {
                return;
            }
        };
        let receiver = match token {
            Some(token) if !matches!(name, ViewTransitionName::Value) => {
                format!("\"{token}\"").into()
            }
            _ => match self.source_expression(value) {
                Some(source) => source,
                None => return,
            },
        };
        let mut arguments = vec![receiver];
        if let Some(key) = key {
            match self.source_expression(key) {
                Some(key) => arguments.push(key),
                None => return,
            }
        }
        self.parts.push(StyleEmissionPart::Style {
            expression: self.guarded_value(
                guard,
                ValueExpr::Entry {
                    property: "view-transition-name".to_owned(),
                    value: Box::new(ValueExpr::Call {
                        helper: RuntimeHelper::ViewTransitionName,
                        arguments,
                        span: value,
                    }),
                },
            ),
            position: value.start.0,
        });
    }

    /// Wraps one inline value in the tests that select it, outermost test last.
    ///
    /// A guarded contribution still evaluates to nothing when its test fails, so the value
    /// carries the test rather than the caller dropping the part.
    fn guarded_value(&self, guard: &[StyleGuard], value: ValueExpr) -> ValueExpr {
        let mut value = value;
        for step in guard.iter().rev() {
            let Some(code) = self.source_expression(step.test) else {
                continue;
            };
            value = ValueExpr::Conditional {
                code,
                when_true: step.when_true,
                value: Box::new(value),
            };
        }
        value
    }

    /// Wraps one class value in the tests that select it.
    fn guarded_class(&self, guard: &[StyleGuard], value: Emitted) -> Emitted {
        let mut emitted = value;
        for step in guard.iter().rev() {
            let Some(code) = self.source_expression(step.test) else {
                continue;
            };
            emitted = Emitted::Conditional {
                code,
                kind: if step.when_true {
                    ConditionalKind::LogicalAnd
                } else {
                    ConditionalKind::Ternary
                },
                consequent: if step.when_true {
                    Some(Box::new(emitted.clone()))
                } else {
                    None
                },
                alternate: if step.when_true {
                    None
                } else {
                    Some(Box::new(emitted))
                },
            };
        }
        emitted
    }

    /// Adds the conditional class a guarded view-transition use marks its element with.
    fn push_guarded_class(&mut self, guard: &[StyleGuard], class: String, position: u32) {
        let emitted = self.guarded_class(guard, Emitted::Leaf { names: vec![class] });
        self.parts
            .push(StyleEmissionPart::Class { emitted, position });
    }
}

/// The static name one view-transition use declares for a class token, if it declares one.
///
/// A literal key is folded here; an expression key and a key carried by the value are
/// written as a style property instead.
fn static_view_transition_name(name: &ViewTransitionName, token: &str) -> Option<String> {
    match name {
        ViewTransitionName::Key(ViewTransitionKey::Literal(key)) => {
            Some(view_transition_name(token, key))
        }
        ViewTransitionName::None
        | ViewTransitionName::Key(ViewTransitionKey::Dynamic(_))
        | ViewTransitionName::Value => None,
    }
}
