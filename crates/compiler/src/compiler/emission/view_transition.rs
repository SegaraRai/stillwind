//! View-transition use lowering for participant classes and names.
//!
//! The `CallEmitter` methods here resolve static and finite runtime transition values
//! under JavaScript guards and CSS variants. They produce class and name contributions;
//! the fragment renderer chooses framework props, while definition emission owns the
//! transition's pseudo-element rules.

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
use crate::style_analysis::DeclarationValue;

impl CallEmitter<'_> {
    /// Lowers one transition use with the conditions in `guard` and variants in
    /// `wrappers`. It records definition and runtime dependencies, participation, and
    /// name supplies. `fold` allows its rules to join the base when no region is pending.
    ///
    /// Returns the class expression together with any dynamic name. The name is written
    /// to inline style unless CSS wrappers require a custom property. The result can be
    /// empty after merging. Resolution and source errors add diagnostics and return
    /// `None`. Class-unit errors can leave a name in the result, so callers must check
    /// diagnostics even when an expression is returned.
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
        let contribution = self.transition_contribution(
            token,
            static_name,
            name_property.clone(),
            wrappers,
            transition.span.start.0,
        )?;
        let analysis_name = contribution
            .view_transitions
            .first()
            .and_then(|use_| use_.name.clone())
            .map(DeclarationValue::Css)
            .or_else(|| runtime_name(name, name_property.as_deref()));
        self.record_transition(
            &transition.identity,
            transition.span,
            guard,
            &contribution,
            analysis_name.as_ref(),
            None,
        );
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

    /// Lowers all possible receivers for the expression at `value`, with the conditions
    /// in `guard` and variants in `wrappers`. It records definition and runtime
    /// dependencies and mutually exclusive participation and name supplies.
    ///
    /// The returned expression uses the receiver's `v` field to select a class and
    /// keeps any dynamic name alongside it. With one candidate, the rules may join the
    /// current region or base. A dynamic name uses inline style when there are no CSS
    /// wrappers.
    ///
    /// Failures add diagnostics and may return `None`. An error while allocating a
    /// class unit can still leave an expression, so callers must always check
    /// diagnostics.
    pub(super) fn emit_dynamic_view_transition_expression(
        &mut self,
        value: &Span,
        transitions: &[AssetReference],
        name: &ViewTransitionName,
        guard: &[GuardStep],
        wrappers: &[StyleVariant],
    ) -> Option<Emitted> {
        let name_value = self.checked_view_transition_name_value(name, None, *value)?;
        let name_property = if wrappers.is_empty() || name_value.is_none() {
            None
        } else {
            Some(self.view_transition_name_property(value.start.0)?)
        };
        let mut candidates = Vec::new();
        for (index, transition) in transitions.iter().enumerate() {
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
                value.start.0,
            )?;
            let analysis_name = contribution
                .view_transitions
                .first()
                .and_then(|use_| use_.name.clone())
                .map(DeclarationValue::Css)
                .or_else(|| runtime_name(name, name_property.as_deref()));
            self.record_transition(
                &transition.identity,
                *value,
                guard,
                &contribution,
                analysis_name.as_ref(),
                Some((index, transitions.len())),
            );
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

    /// Keeps a transition token and name for later allocation to a class unit.
    ///
    /// `token` is the canonical token, and `name` is an escaped static CSS identifier.
    /// `dynamic_name_property` names a custom property whose runtime value must come
    /// from the same use. It requires at least one CSS variant in `wrappers`.
    /// `source_offset` is the original receiver's inclusive UTF-8 byte start in this
    /// module; isolated previews use it to replay slots from the complete call.
    ///
    /// Returns the contribution without allocating names or classes. If variant
    /// resolution fails, it adds a diagnostic and returns `None`.
    fn transition_contribution(
        &mut self,
        token: String,
        name: Option<String>,
        dynamic_name_property: Option<String>,
        wrappers: &[StyleVariant],
        source_offset: u32,
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
                source_offset,
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

    /// Returns the deferred runtime name expression for a dynamic key. `Ok(None)` means
    /// no runtime name is required.
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
            ViewTransitionName::Key(ViewTransitionKey::Dynamic(key)) => *key,
            ViewTransitionName::None | ViewTransitionName::Key(ViewTransitionKey::Literal(_)) => {
                return Ok(None);
            }
        };
        let receiver = match token {
            Some(token) => format!("\"{token}\"").into(),
            _ => self.source_expression_result(value)?,
        };
        let arguments = vec![receiver, self.source_expression_result(key)?];
        Ok(Some(ValueExpr::Call {
            helper: RuntimeHelper::ViewTransitionName,
            arguments,
            span: value,
        }))
    }

    /// Builds the expression that will compute a dynamic transition name.
    ///
    /// When provided, `token` supplies the canonical identity. Otherwise, the
    /// expression reads the receiver at `value`.
    ///
    /// Returns `Some(None)` when no dynamic name is needed, or `Some(Some(value))` with
    /// the runtime expression. A source extraction failure adds a diagnostic and
    /// returns outer `None`.
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
}

/// The static name one view-transition use declares for a class token, if it declares
/// one.
///
/// A literal key is folded here; an expression key is written as a style property
/// instead.
fn static_view_transition_name(name: &ViewTransitionName, token: &str) -> Option<String> {
    match name {
        ViewTransitionName::Key(ViewTransitionKey::Literal(key)) => {
            Some(view_transition_name(token, key))
        }
        ViewTransitionName::None | ViewTransitionName::Key(ViewTransitionKey::Dynamic(_)) => None,
    }
}

/// Returns the recorded value for a dynamic transition key, or `None` for other names.
/// `slot` is an escaped custom-property name when a CSS wrapper controls the use;
/// its value is recorded as `var(slot)`. Without a slot, the original key span
/// identifies the inline value produced at runtime.
fn runtime_name(name: &ViewTransitionName, slot: Option<&str>) -> Option<DeclarationValue> {
    match name {
        ViewTransitionName::Key(ViewTransitionKey::Dynamic(expression)) => {
            Some(slot.map_or(DeclarationValue::Runtime(*expression), |slot| {
                DeclarationValue::Css(format!("var({slot})"))
            }))
        }
        _ => None,
    }
}
