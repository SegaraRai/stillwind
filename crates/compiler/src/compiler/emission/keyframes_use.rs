//! Animation uses, static and dynamic, and the declarations they contribute.

use stillwind_design_system::{ThemeIndex, UtilityOrder};
use stillwind_diagnostics::CompileError;
use stillwind_ir::{AssetReference, StyleVariant};
use stillwind_semantic::AssetKind;
use stillwind_source::{Span, Spanned};

use super::super::animation_options::{OptionVariants, resolve_options};
use super::super::fragment::Emitted;
use super::super::literal::LiteralContribution;
use super::super::props::GuardStep;
use super::super::style_variants::StyleVariantPlan;
use super::super::symbols::identity_declaration;
use super::super::utilities::{CompiledUtility, UtilitySource};
use super::{CallEmitter, KEYFRAMES_KEY};
use crate::identifiers::IdentifierContext;

impl CallEmitter<'_> {
    /// Lowers one keyframes use whose keyframes is known at compile time.
    ///
    /// A use reached without a guard can fold into the base unit, which is why `fold`
    /// is passed down; the option list resolves into the use's own declarations.
    /// Returns a retained class expression when needed. `None` means the rule joined a
    /// pending region or base, or name allocation failed with a separately recorded
    /// diagnostic.
    pub(super) fn emit_keyframes_use(
        &mut self,
        keyframes: &AssetReference,
        options: &Spanned<String>,
        guard: &[GuardStep],
        wrappers: &[StyleVariant],
        fold: bool,
    ) -> Option<Emitted> {
        self.required_definitions.insert(keyframes.identity.clone());
        let plan =
            StyleVariantPlan::resolve(self.identifiers, wrappers, self.theme, self.diagnostics)?;
        let mut utility = resolve_keyframes_use(
            self.identifiers,
            keyframes,
            options,
            self.theme,
            self.diagnostics,
        );
        plan.apply_rule(&mut utility);
        let canonical = format!(
            "{}\0keyframes\0{}\0{}\0",
            plan.canonical,
            keyframes.identity.binding_name(),
            options.value
        );
        let contribution = LiteralContribution {
            canonical,
            utilities: vec![utility],
            markers: Vec::new(),
            external: Vec::new(),
            plain: Vec::new(),
            view_transitions: Vec::new(),
        };
        self.contribute_rule(contribution, guard, fold)
    }

    /// Lowers `options` for every possible keyframes receiver at `value`.
    /// `guard` contains the enclosing JavaScript conditions, and `wrappers` contains the
    /// CSS variants. The function records definition and runtime dependencies.
    ///
    /// With multiple candidates, the returned expression selects a class using the
    /// receiver's `k` field. With one candidate, the rules may join the current region
    /// or base. `None` means merging or failure. Callers must check diagnostics even
    /// when an expression is returned.
    pub(super) fn emit_dynamic_keyframes_use(
        &mut self,
        value: &Span,
        keyframes: &[AssetReference],
        options: &Spanned<String>,
        guard: &[GuardStep],
        wrappers: &[StyleVariant],
    ) -> Option<Emitted> {
        let plan =
            StyleVariantPlan::resolve(self.identifiers, wrappers, self.theme, self.diagnostics)?;
        let mut candidates = Vec::new();
        for keyframes in keyframes {
            self.runtime_assets.insert(keyframes.identity.clone());
            self.required_definitions.insert(keyframes.identity.clone());
            let identity = match self.record_definition_name(keyframes, AssetKind::Keyframes) {
                Ok(identity) => identity,
                Err(failure) => {
                    self.reject_name(failure, self.call.span);
                    return None;
                }
            };
            let mut utility = resolve_keyframes_use(
                self.identifiers,
                keyframes,
                options,
                self.theme,
                self.diagnostics,
            );
            plan.apply_rule(&mut utility);
            let canonical = format!(
                "{}\0dynamic-keyframes\0{identity}\0{}\0",
                plan.canonical, options.value
            );
            let contribution = LiteralContribution {
                canonical,
                utilities: vec![utility],
                markers: Vec::new(),
                external: Vec::new(),
                plain: Vec::new(),
                view_transitions: Vec::new(),
            };
            candidates.push((identity, contribution));
        }
        self.emit_candidate_units(value, KEYFRAMES_KEY, candidates, guard)
    }
}

/// Returns the resolved utility containing the declarations one keyframes use
/// contributes. Invalid options append diagnostics to `diagnostics`.
///
/// The `animation-name` always names this declaration's keyframes, and the options
/// follow in their written order.
fn resolve_keyframes_use(
    identifiers: &IdentifierContext,
    keyframes: &AssetReference,
    options: &Spanned<String>,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
) -> CompiledUtility {
    let mut declaration = format!(
        "animation-name:{};",
        match identifiers.keyframes_name(&identity_declaration(keyframes)) {
            Ok(name) => name.custom_ident(),
            Err(failure) => {
                diagnostics.push(CompileError::new(failure, options.span));
                String::new()
            }
        }
    );
    declaration.push_str(
        &resolve_options(options, OptionVariants::Rejected, theme, diagnostics).declaration,
    );
    CompiledUtility {
        source: UtilitySource::default(),
        required_properties: Vec::new(),
        variants: Vec::new(),
        variant_applications: Vec::new(),
        declaration,
        selector: None,
        container: None,
        nested: Vec::new(),
        order: UtilityOrder::Authored(None),
        candidate: String::new(),
    }
}
