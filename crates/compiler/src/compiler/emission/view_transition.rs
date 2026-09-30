//! View-transition use lowering for participant classes and names.
//!
//! The `CallEmitter` methods here resolve static and finite runtime transition values
//! under JavaScript guards and CSS variants. They produce class and name contributions;
//! the fragment renderer chooses framework props, while definition emission owns the
//! transition's pseudo-element rules.

use std::collections::BTreeMap;

use stillwind_diagnostics::{LintFinding, LintKind};
use stillwind_ir::{AssetReference, StyleVariant, ViewTransitionKey, ViewTransitionName};
use stillwind_semantic::AssetKind;
use stillwind_source::{SourceError, Span};

use super::super::fragment::{Emitted, EmittedEntry, Property, ValueExpr};
use super::super::literal::{LiteralContribution, TransitionContribution};
use super::super::props::GuardStep;
use super::super::style_variants::StyleVariantPlan;
use super::{CallEmitter, VIEW_TRANSITION_CLASS_KEY};
use crate::identifiers::view_transition_name;
use crate::runtime::RuntimeHelper;

/// Conditional name supplies used to detect competing view-transition names in one
/// style call. Each node represents a shared guard prefix. Test identity comes from
/// source positions, so equivalent expressions at different positions remain
/// independent for this analysis.
#[derive(Default)]
pub(super) struct TransitionNamePaths {
    /// Supplies selected by this node's prefix without any further condition.
    count: usize,
    /// Keys are `(inclusive start byte, exclusive end byte)` of a source test. Each
    /// value has the falsy arm at index 0 and truthy arm at index 1.
    branches: BTreeMap<(u32, u32), [Self; 2]>,
}

impl TransitionNamePaths {
    /// Adds `count` name supplies under an outermost-to-innermost guard path from this call's
    /// source. An empty path adds an unconditional supply. Repeated insertion counts
    /// another supply, even when the path matches an existing one.
    pub(super) fn insert(&mut self, guard: &[GuardStep], count: usize) {
        let mut node = self;
        for step in guard {
            node = &mut node
                .branches
                .entry((step.test.start.0, step.test.end.0))
                .or_default()[usize::from(step.when_true)];
        }
        node.count += count;
    }

    /// Returns a conservative upper bound on simultaneous supplies below this guard
    /// prefix. Opposite arms of a shared test contribute their larger count. Distinct
    /// test ranges contribute additively, without checking correlations between their
    /// runtime values.
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
    /// Resolves a transition use within a class tree, retaining any name value beside
    /// its class under the tree's JavaScript condition path. Returns the class
    /// expression, with a dynamic name payload when required; `None` means CSS variant
    /// resolution or generated identity allocation failed.
    pub(super) fn emit_view_transition_expression(
        &mut self,
        transition: &AssetReference,
        name: &ViewTransitionName,
        guard: &[GuardStep],
        wrappers: &[StyleVariant],
        fold: bool,
    ) -> Option<Emitted> {
        self.required_definitions
            .insert(transition.identity.clone());
        self.runtime_assets.insert(transition.identity.clone());
        self.view_transition_classes = true;
        if !matches!(name, ViewTransitionName::None) {
            self.view_transition_name_paths.insert(guard, 1);
        }
        let token = match self.record_definition_name(transition, AssetKind::ViewTransition) {
            Ok(token) => token,
            Err(failure) => {
                self.reject_name(failure, self.call.span);
                return None;
            }
        };
        let name_value =
            self.checked_view_transition_name_value(name, Some(&token), transition.span)?;
        let static_name = static_view_transition_name(name, &token);
        let name_property = if wrappers.is_empty() || name_value.is_none() {
            None
        } else {
            Some(self.view_transition_name_property(transition.span.start.0)?)
        };
        let contribution =
            self.transition_contribution(token, static_name, name_property.clone(), wrappers)?;
        let class = self
            .contribute_rule(contribution, guard, fold)
            .unwrap_or(Emitted::Leaf { names: Vec::new() });
        Some(Self::view_transition_emitted(
            class,
            name_value,
            name_property,
            transition.span,
        ))
    }

    /// Builds every candidate class for a finite transition value within a class tree.
    /// Returns a runtime lookup of those classes with any dynamic name payload; `None`
    /// means resolution, source slicing, or name allocation failed.
    pub(super) fn emit_dynamic_view_transition_expression(
        &mut self,
        value: &Span,
        transitions: &[AssetReference],
        name: &ViewTransitionName,
        guard: &[GuardStep],
        wrappers: &[StyleVariant],
    ) -> Option<Emitted> {
        self.view_transition_classes = true;
        if !matches!(name, ViewTransitionName::None) {
            self.view_transition_name_paths.insert(guard, 1);
        }
        let name_value = self.checked_view_transition_name_value(name, None, *value)?;
        let name_property = if wrappers.is_empty() || name_value.is_none() {
            None
        } else {
            Some(self.view_transition_name_property(value.start.0)?)
        };
        let mut candidates = Vec::new();
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
            let contribution = self.transition_contribution(
                token.clone(),
                static_name,
                name_property.clone(),
                wrappers,
            )?;
            candidates.push((token, contribution));
        }
        let class = self
            .emit_candidate_units(value, VIEW_TRANSITION_CLASS_KEY, candidates, guard)
            .unwrap_or(Emitted::Leaf { names: Vec::new() });
        Some(Self::view_transition_emitted(
            class,
            name_value,
            name_property,
            *value,
        ))
    }

    /// Retains the transition's CSS context until the shared unit allocator merges its region.
    fn transition_contribution(
        &mut self,
        token: String,
        name: Option<String>,
        dynamic_name_property: Option<String>,
        wrappers: &[StyleVariant],
    ) -> Option<LiteralContribution> {
        let plan = if wrappers.is_empty() {
            None
        } else {
            Some(StyleVariantPlan::resolve(
                self.identifiers,
                wrappers,
                self.theme,
                self.diagnostics,
            )?)
        };
        let canonical = format!(
            "{}\0view-transition\0{token}\0{}\0{}\0",
            plan.as_ref().map_or("", |plan| plan.canonical.as_str()),
            name.as_deref().unwrap_or(""),
            dynamic_name_property.as_deref().unwrap_or("")
        );
        Some(LiteralContribution {
            canonical,
            view_transitions: vec![TransitionContribution {
                token,
                name,
                dynamic_name_property,
                plan,
            }],
            ..LiteralContribution::default()
        })
    }

    /// Returns the custom-property name for a dynamic transition name at an original
    /// UTF-8 source offset. Reports allocation failure and returns `None` when the name
    /// cannot be generated.
    fn view_transition_name_property(&mut self, source_offset: u32) -> Option<String> {
        match self.units.view_transition_dynamic_name_slot(source_offset) {
            Ok(name) => Some(name),
            Err(failure) => {
                self.reject_name(failure, self.call.span);
                None
            }
        }
    }

    /// Returns the class alone when no dynamic name is needed. Otherwise it returns a
    /// grouped class and name value; a CSS variant writes the value through
    /// `name_property`, while an unwrapped use writes inline style.
    fn view_transition_emitted(
        class: Emitted,
        name_value: Option<ValueExpr>,
        name_property: Option<String>,
        span: Span,
    ) -> Emitted {
        let Some(name_value) = name_value else {
            return class;
        };
        let extra = if let Some(property) = name_property {
            EmittedEntry::CustomProperty(Property {
                static_value: None,
                computed_value: Some(name_value),
                name: property,
                value: String::new(),
                span,
            })
        } else {
            EmittedEntry::StylePayload(ValueExpr::Entry {
                property: "view-transition-name".to_owned(),
                value: Box::new(name_value),
            })
        };
        Emitted::Group(vec![Some(EmittedEntry::Class(class)), Some(extra)])
    }

    /// Returns the deferred runtime name expression for a dynamic key or a keyed
    /// transition value. `Ok(None)` means no runtime name is required.
    ///
    /// # Errors
    ///
    /// Returns the source cause when a receiver or dynamic key is unreadable.
    fn view_transition_name_value(
        &self,
        name: &ViewTransitionName,
        token: Option<&str>,
        value: Span,
    ) -> Result<Option<ValueExpr>, SourceError> {
        let key = match name {
            ViewTransitionName::Key(ViewTransitionKey::Dynamic(key)) => Some(*key),
            ViewTransitionName::Value => None,
            ViewTransitionName::None | ViewTransitionName::Key(ViewTransitionKey::Literal(_)) => {
                return Ok(None);
            }
        };
        let receiver = match token {
            Some(token) if !matches!(name, ViewTransitionName::Value) => {
                format!("\"{token}\"").into()
            }
            _ => self.source_expression_result(value)?,
        };
        let mut arguments = vec![receiver];
        if let Some(key) = key {
            arguments.push(self.source_expression_result(key)?);
        }
        Ok(Some(ValueExpr::Call {
            helper: RuntimeHelper::ViewTransitionName,
            arguments,
            span: value,
        }))
    }

    #[expect(
        clippy::option_option,
        reason = "outer None records a fatal source error; inner None means no runtime name"
    )]
    fn checked_view_transition_name_value(
        &mut self,
        name: &ViewTransitionName,
        token: Option<&str>,
        value: Span,
    ) -> Option<Option<ValueExpr>> {
        match self.view_transition_name_value(name, token, value) {
            Ok(name_value) => Some(name_value),
            Err(error) => {
                self.report_source_error(&error);
                None
            }
        }
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
        let kind = match self.view_transition_name_paths.maximum() {
            0 => LintKind::MissingViewTransitionName,
            1 => return,
            count => LintKind::DuplicateViewTransitionName { count },
        };
        self.lint_findings
            .push(LintFinding::new(kind, self.call.span));
    }
}

/// The static name one view-transition use declares for a class token, if it declares
/// one.
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
