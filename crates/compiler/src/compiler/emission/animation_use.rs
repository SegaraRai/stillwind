//! Animation uses, static and dynamic, and the declarations they contribute.

use stillwind_ir::{AssetReference, StyleVariant};
use stillwind_semantic::AssetKind;
use stillwind_source::{Span, Spanned};

use super::super::fragment::{Emitted, ValueExpr};
use super::super::literal::LiteralContribution;
use super::super::motion::resolve_animation_use;
use super::super::props::GuardStep;
use super::super::style_variants::StyleVariantPlan;
use super::CallEmitter;
use crate::generated::js_string;
use crate::runtime::RuntimeHelper;

impl CallEmitter<'_> {
    /// Lowers one animation use whose animation is known at compile time.
    ///
    /// A use reached without a guard can fold into the base unit, which is why `fold` is
    /// passed down; the option list resolves into the use's own declarations.
    pub(super) fn emit_animation_use(
        &mut self,
        animation: &AssetReference,
        options: &Spanned<String>,
        guard: &[GuardStep],
        wrappers: &[StyleVariant],
        fold: bool,
    ) -> Option<Emitted> {
        self.motion_assets.insert(animation.identity.clone());
        let plan =
            StyleVariantPlan::resolve(self.identifiers, wrappers, self.theme, self.diagnostics)?;
        let mut utility = resolve_animation_use(
            self.identifiers,
            animation,
            options,
            self.theme,
            self.diagnostics,
        );
        plan.apply_animation(&mut utility);
        let canonical = format!(
            "{}{}:{}:{}\u{0}",
            plan.canonical, "animation:", animation.identity.binding_name, options.value
        );
        let contribution = LiteralContribution {
            canonical,
            utilities: vec![utility],
            markers: Vec::new(),
            external: Vec::new(),
            plain: Vec::new(),
            view_transition_token: None,
            view_transition_name: None,
        };
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

    /// Lowers one animation use whose animation is only known at runtime.
    ///
    /// Every candidate contributes its own prebuilt class, so the runtime selects a class
    /// set instead of resolving declarations.
    pub(super) fn emit_dynamic_animation_use(
        &mut self,
        value: &Span,
        animations: &[AssetReference],
        options: &Spanned<String>,
        guard: &[GuardStep],
        wrappers: &[StyleVariant],
    ) -> Option<Emitted> {
        let plan =
            StyleVariantPlan::resolve(self.identifiers, wrappers, self.theme, self.diagnostics)?;
        let mut table = Vec::new();
        for animation in animations {
            self.runtime_assets.insert(animation.identity.clone());
            let identity = match self.record_definition_name(
                &animation.identity,
                AssetKind::Animation,
                animation.span,
            ) {
                Ok(identity) => identity,
                Err(failure) => {
                    self.reject_name(failure, self.call.span);
                    return None;
                }
            };
            let mut utility = resolve_animation_use(
                self.identifiers,
                animation,
                options,
                self.theme,
                self.diagnostics,
            );
            plan.apply_animation(&mut utility);
            let canonical = if plan.canonical.is_empty() {
                format!("dynamic-animation:{identity}:{}", options.value)
            } else {
                format!(
                    "{}\u{0}dynamic-animation:{identity}:{}",
                    plan.canonical, options.value
                )
            };
            let contribution = LiteralContribution {
                canonical,
                utilities: vec![utility],
                markers: Vec::new(),
                external: Vec::new(),
                plain: Vec::new(),
                view_transition_token: None,
                view_transition_name: None,
            };
            let class = match self.units.leaf(contribution, guard) {
                Ok(class) => class,
                Err(failure) => {
                    self.reject_name(failure, self.call.span);
                    return None;
                }
            };
            let Emitted::Leaf { names } = class else {
                return None;
            };
            let class = js_string(&names.join(" "));
            table.push(format!("\"{identity}\":{class}"));
        }
        let table = table.join(",");
        let expression = ValueExpr::Call {
            helper: RuntimeHelper::AssetClass,
            arguments: vec![
                self.source_expression(*value)?,
                format!("{{{table}}}").into(),
            ],
            span: *value,
        };
        Some(Emitted::Expression(expression))
    }
}
