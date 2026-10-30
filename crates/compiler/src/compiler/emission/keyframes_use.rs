//! Animation uses, static and dynamic, and the declarations they contribute.

use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_design_system::{ThemeIndex, UtilityOrder, VariantContext};
use stillwind_diagnostics::{CompileError, LintFinding};
use stillwind_ir::{AssetReference, StyleVariant};
use stillwind_semantic::AssetKind;
use stillwind_source::{Span, Spanned};

use super::super::animation_options::resolve_options;
use super::super::fragment::Emitted;
use super::super::literal::LiteralContribution;
use super::super::props::GuardStep;
use super::super::style_variants::StyleVariantPlan;
use super::super::symbols::identity_declaration;
use super::super::utilities::{CompiledUtility, CompiledUtilityDiagnosticData};
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
        options: &Spanned<LeanStr>,
        guard: &[GuardStep],
        wrappers: &[StyleVariant],
        fold: bool,
    ) -> Option<Emitted> {
        self.required_definitions.insert(keyframes.identity.clone());
        let plan = StyleVariantPlan::resolve(
            self.identifiers,
            wrappers,
            self.theme,
            self.diagnostics,
            self.lint_findings,
        )?;
        let mut utility = resolve_keyframes_use(
            self.identifiers,
            keyframes,
            options,
            self.theme,
            self.diagnostics,
            self.lint_findings,
        );
        plan.apply_rule(&mut utility);
        let canonical = format_args!(
            "{}\0keyframes\0{}\0{}\0",
            plan.canonical,
            keyframes.identity.binding_name(),
            options.value
        )
        .to_lean_string();
        let contribution = LiteralContribution {
            canonical: (canonical),
            utilities: vec![utility],
            markers: Vec::new(),
            plain: Vec::new(),
            view_transitions: Vec::new(),
        };
        self.contribute_rule(contribution, guard, fold)
    }

    /// Lowers `options` for every possible keyframes receiver at `value`.
    /// `guard` contains the enclosing JavaScript conditions, and `wrappers` contains the
    /// CSS variants. The function records definition dependencies, and runtime
    /// dependencies when the output reads the receiver.
    ///
    /// With multiple candidates, the returned expression selects a class using the
    /// receiver's `k` field. With one candidate, the rules may join the current region
    /// or base. `None` means merging or failure. Callers must check diagnostics even
    /// when an expression is returned.
    pub(super) fn emit_dynamic_keyframes_use(
        &mut self,
        value: &Span,
        keyframes: &[AssetReference],
        options: &Spanned<LeanStr>,
        guard: &[GuardStep],
        wrappers: &[StyleVariant],
    ) -> Option<Emitted> {
        let plan = StyleVariantPlan::resolve(
            self.identifiers,
            wrappers,
            self.theme,
            self.diagnostics,
            self.lint_findings,
        )?;
        let mut candidates = Vec::new();
        // One candidate lowers without reading the receiver.
        let reads_receiver = keyframes.len() > 1;
        for keyframes in keyframes {
            if reads_receiver {
                self.runtime_assets.insert(keyframes.identity.clone());
            }
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
                self.lint_findings,
            );
            plan.apply_rule(&mut utility);
            let canonical = format_args!(
                "{}\0dynamic-keyframes\0{identity}\0{}\0",
                plan.canonical, options.value
            )
            .to_lean_string();
            let contribution = LiteralContribution {
                canonical: (canonical),
                utilities: vec![utility],
                markers: Vec::new(),
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
    options: &Spanned<LeanStr>,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
    lint_findings: &mut Vec<LintFinding>,
) -> CompiledUtility {
    let mut declaration = format_args!(
        "animation-name:{};",
        match identifiers.keyframes_name(&identity_declaration(keyframes)) {
            Ok(name) => name.custom_ident(),
            Err(failure) => {
                diagnostics.push(CompileError::new(failure, options.span));
                LeanString::new()
            }
        }
    )
    .to_lean_string();
    let (resolved, banners) = resolve_options(options, theme, diagnostics, lint_findings);
    declaration.push_str(&resolved);
    CompiledUtility {
        banners,
        diagnostic_data: CompiledUtilityDiagnosticData::default(),
        context: VariantContext::Element,
        required_properties: Vec::new(),
        variants: Vec::new(),
        variant_applications: Vec::new(),
        declaration: (declaration).into(),
        selector: None,
        container: None,
        nested: Vec::new(),
        order: UtilityOrder::Authored(None),
        candidate: (LeanString::new()).into(),
    }
}
