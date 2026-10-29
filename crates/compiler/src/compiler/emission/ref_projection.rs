//! Ref projections, static and dynamic, and the declarations they render.

use lean_string::{LeanString, ToLeanString};

use stillwind_design_system::{UtilityOrder, VariantContext};
use stillwind_diagnostics::CompileDiagnostic;
use stillwind_ir::{AnchorScope, RefDefinition, RefProjection, StyleVariant};
use stillwind_source::{JoinText, ModuleId, Span};

use super::super::fragment::{Emitted, ValueExpr};
use super::super::literal::LiteralContribution;
use super::super::props::GuardStep;
use super::super::style_variants::StyleVariantPlan;
use super::super::symbols::{
    anchor_name, identity_input_parts, projection_symbol, ref_container_name, ref_context,
    ref_identity, ref_marker_name, ref_projection_name,
};
use super::super::utilities::{CompiledUtility, CompiledUtilityDiagnosticData};
use super::{CLASS_KEY, CallEmitter};
use crate::generated::Code;
use crate::style_analysis::SupplyKind;
use crate::{GeneratedSymbol, GeneratedSymbolKind, IdentifierContext, IdentifierPool};

impl CallEmitter<'_> {
    /// Builds the CSS contribution for `projection` on `definition`.
    /// `wrappers` lists the enclosing CSS variants from outermost to innermost.
    /// Provider selectors require the receiver's marker. Consumer selectors do not.
    ///
    /// Records symbols for generated anchor or container names without attaching the
    /// marker. Returns `None` and adds a diagnostic if variant or name resolution fails.
    fn ref_contribution(
        &mut self,
        definition: &RefDefinition,
        projection: &RefProjection,
        wrappers: &[StyleVariant],
    ) -> Option<LiteralContribution> {
        let plan = StyleVariantPlan::resolve(
            self.identifiers,
            wrappers,
            self.theme,
            self.diagnostics,
            self.lint_findings,
        )?;
        let (declaration, symbol) =
            match render_ref_projection(self.identifiers, definition, projection, self.module_id) {
                Ok(value) => value,
                Err(failure) => {
                    self.reject_name(failure, self.call.span);
                    return None;
                }
            };
        if let Some(symbol) = symbol {
            self.written_name_symbols.push(symbol);
        }
        let selector = if matches!(projection, RefProjection::PositionAnchor) {
            None
        } else {
            match ref_marker_name(self.identifiers, definition) {
                Ok(marker) => Some(format_args!("&:where({})", marker.selector()).to_lean_string()),
                Err(failure) => {
                    self.reject_name(failure, self.call.span);
                    return None;
                }
            }
        };
        Some(rule_contribution(&plan, declaration, selector))
    }

    /// Lowers the ref use at `span` with the JavaScript conditions in `guard` and
    /// CSS variants in `wrappers`. It records the properties supplied by the use before
    /// merging its rules. If no region is pending, `fold` allows an unconditional use
    /// to join the base.
    ///
    /// Returns the class expression still needed at runtime. `None` means that the rules
    /// were merged or that resolution or emission failed. Failures add diagnostics.
    pub(super) fn emit_ref_projection(
        &mut self,
        definition: &RefDefinition,
        span: Span,
        projection: &RefProjection,
        guard: &[GuardStep],
        wrappers: &[StyleVariant],
        fold: bool,
    ) -> Option<Emitted> {
        let contribution = self.ref_contribution(definition, projection, wrappers)?;
        let provider = !matches!(projection, RefProjection::PositionAnchor);
        let gate = provider.then(|| ref_identity(definition));
        if provider {
            self.record_supplies(
                span,
                guard,
                vec![SupplyKind::Provider {
                    reference: ref_identity(definition),
                    name: (definition.binding.value.clone()),
                }],
                None,
            );
        }
        self.record_properties(&contribution, provider, span, guard, gate.as_ref(), None);
        self.contribute_rule(contribution, guard, fold)
    }

    /// Lowers each possible receiver in `refs` for the expression at `value`.
    /// `guard` lists the enclosing JavaScript conditions, and `wrappers` lists the CSS
    /// variants. The function records runtime asset dependencies and supplied properties.
    ///
    /// The returned expression selects classes using the receiver's `c` field. With one
    /// candidate, the rules may be merged without a lookup. `None` means merging or
    /// failure. Failures add diagnostics and keep earlier records and allocations.
    pub(super) fn emit_dynamic_ref_projection(
        &mut self,
        value: &Span,
        refs: &[RefDefinition],
        projection: &RefProjection,
        guard: &[GuardStep],
        wrappers: &[StyleVariant],
    ) -> Option<Emitted> {
        let mut candidates = Vec::new();
        for (index, definition) in refs.iter().enumerate() {
            self.runtime_assets.insert(ref_identity(definition));
            let marker = match ref_marker_name(self.identifiers, definition) {
                Ok(marker) => marker,
                Err(failure) => {
                    self.reject_name(failure, self.call.span);
                    return None;
                }
            };
            let contribution = self.ref_contribution(definition, projection, wrappers)?;
            let provider = !matches!(projection, RefProjection::PositionAnchor);
            if provider {
                self.record_supplies(
                    *value,
                    guard,
                    vec![SupplyKind::Provider {
                        reference: ref_identity(definition),
                        name: (definition.binding.value.clone()),
                    }],
                    Some((index, refs.len())),
                );
            }
            self.record_properties(
                &contribution,
                provider,
                *value,
                guard,
                provider.then(|| ref_identity(definition)).as_ref(),
                Some((index, refs.len())),
            );
            candidates.push((marker.as_str().to_lean_string(), contribution));
        }
        self.emit_candidate_units(value, CLASS_KEY, candidates, guard)
    }

    /// Adds CSS for `scope` at `span`, with the conditions in `guard` and variants in
    /// `wrappers`. The selector does not require a ref marker. An empty ref list emits
    /// nothing. Other scopes are recorded as provider writes for conflict checks.
    ///
    /// If no region is pending, `fold` allows the rules to join the base. Returns the
    /// class expression still needed at runtime, or `None` when the rules are empty,
    /// are merged, or fail to lower. Failures add diagnostics.
    pub(super) fn emit_anchor_scope(
        &mut self,
        scope: &AnchorScope,
        span: Span,
        guard: &[GuardStep],
        wrappers: &[StyleVariant],
        fold: bool,
    ) -> Option<Emitted> {
        let plan = StyleVariantPlan::resolve(
            self.identifiers,
            wrappers,
            self.theme,
            self.diagnostics,
            self.lint_findings,
        )?;
        let declaration = match render_anchor_scope(self.identifiers, scope) {
            Ok(value) => value,
            Err(failure) => {
                self.reject_name(failure, self.call.span);
                return None;
            }
        };
        if declaration.is_empty() {
            return None;
        }
        let contribution = rule_contribution(&plan, declaration, None);
        self.record_properties(&contribution, true, span, guard, None, None);
        self.contribute_rule(contribution, guard, fold)
    }
}

/// Returns an authored utility with `declaration`, an optional selector, and resolved variants.
/// `declaration` contains complete CSS declarations. `selector` is a template with
/// `&` for the generated class, or `None` to use that class directly. The caller must
/// escape any generated identifiers in both strings. The returned contribution includes
/// these strings and `plan` in its canonical identity and allocates no class or symbol.
pub(super) fn rule_contribution(
    plan: &StyleVariantPlan,
    declaration: LeanString,
    selector: Option<LeanString>,
) -> LiteralContribution {
    let canonical = format_args!(
        "{}\0ref-use\0{}\0{}\0",
        plan.canonical,
        selector.as_deref().unwrap_or(""),
        declaration
    )
    .to_lean_string();
    let mut utility = CompiledUtility {
        banners: Vec::new(),
        diagnostic_data: CompiledUtilityDiagnosticData::default(),
        context: VariantContext::Element,
        required_properties: Vec::new(),
        variants: Vec::new(),
        variant_applications: Vec::new(),
        declaration: (declaration).into(),
        selector: (selector).map(Into::into),
        container: None,
        nested: Vec::new(),
        order: UtilityOrder::Authored(None),
        candidate: (LeanString::new()).into(),
    };
    plan.apply_rule(&mut utility);
    LiteralContribution {
        canonical: (canonical),
        utilities: vec![utility],
        ..LiteralContribution::default()
    }
}

/// Returns the CSS declaration for `scope`, or an empty string for an empty ref list.
/// The caller must remove duplicate refs and put them in the intended output order.
/// `context` generates and escapes their anchor names. The receiver is included
/// only when it appears in the list.
///
/// # Errors
///
/// Returns an error if anchor-name generation fails for any listed ref.
fn render_anchor_scope(
    context: &IdentifierContext,
    scope: &AnchorScope,
) -> Result<LeanString, CompileDiagnostic> {
    let value = match scope {
        AnchorScope::All => "all".to_lean_string(),
        AnchorScope::None => "none".to_lean_string(),
        AnchorScope::Refs(refs) => {
            if refs.is_empty() {
                return Ok(LeanString::new());
            }
            refs.iter()
                .map(|definition| {
                    anchor_name(context, definition, None).map(|name| name.dashed_ident())
                })
                .collect::<Result<Vec<_>, _>>()?
                .join_text(",")
        }
    };
    Ok(format_args!("anchor-scope:{value};").to_lean_string())
}

/// Returns an expression that will read the marker token from a compiled ref's `c`
/// field. The caller must supply code that yields a compiled ref record. This
/// function does not evaluate or validate that code.
pub(super) fn ref_class_read(value: Code, span: Span) -> ValueExpr {
    ValueExpr::RefClassRead { value, span }
}

/// Returns the CSS declaration and optional generated-name symbol for `projection`.
/// `definition` provides the receiver identity. `module_id` identifies the module
/// that reports the symbol.
///
/// Providers return a symbol for their anchor or container name. If scope or type is
/// omitted, its property is omitted too. The caller adds any selector that requires
/// the receiver's marker.
///
/// # Errors
///
/// Returns an error if name generation fails for the receiver or a listed scope ref.
fn render_ref_projection(
    context: &IdentifierContext,
    definition: &RefDefinition,
    projection: &RefProjection,
    module_id: &ModuleId,
) -> Result<(LeanString, Option<GeneratedSymbol>), CompileDiagnostic> {
    let name = anchor_name(context, definition, Some(ref_projection_name(projection)))?;
    let symbol = |name: &str, pool: IdentifierPool, kind: GeneratedSymbolKind| {
        Some(projection_symbol(
            name,
            identity_input_parts(&ref_context(definition, None)),
            pool,
            kind,
            module_id,
            definition.call_span,
        ))
    };
    Ok(match projection {
        RefProjection::AsAnchor(scope) => (
            format_args!(
                "anchor-name:{};{}",
                name.dashed_ident(),
                scope
                    .as_ref()
                    .map(|scope| render_anchor_scope(context, scope))
                    .transpose()?
                    .unwrap_or_default()
            )
            .to_lean_string(),
            symbol(
                name.as_str(),
                IdentifierPool::AnchorName,
                GeneratedSymbolKind::Anchor,
            ),
        ),
        RefProjection::PositionAnchor => (
            format_args!("position-anchor:{};", name.dashed_ident()).to_lean_string(),
            None,
        ),
        RefProjection::AsContainer(kind) => {
            let container = ref_container_name(context, definition)?;
            (
                format_args!(
                    "container-name:{};{}",
                    container.custom_ident(),
                    kind.as_ref()
                        .map(|kind| format_args!("container-type:{};", kind.value).to_lean_string())
                        .unwrap_or_default()
                )
                .to_lean_string(),
                Some(projection_symbol(
                    container.as_str(),
                    identity_input_parts(&ref_context(definition, None)),
                    IdentifierPool::ContainerName,
                    GeneratedSymbolKind::Container,
                    module_id,
                    definition.call_span,
                )),
            )
        }
    })
}

#[cfg(test)]
mod tests {
    use lean_string::ToLeanStr;

    use stillwind_ir::ContainerType;
    use stillwind_source::{FileId, Span, Spanned};

    use super::*;
    use crate::compiler::symbols::ref_marker_name;
    use crate::identifiers::{
        DEFAULT_HASH_LENGTH, GeneratorSet, IdentifierContext, IdentifierMode, IdentifierPool,
    };

    fn identifiers() -> IdentifierContext {
        IdentifierContext::new(
            GeneratorSet::default(),
            IdentifierMode::Release,
            DEFAULT_HASH_LENGTH,
        )
    }

    #[test]
    fn projections_share_their_ref_identity() {
        let binding = Spanned::new("ref".to_lean_str(), Span::new(FileId(1), 0, 3));
        let mut definition = RefDefinition {
            module_id: ModuleId::from("x"),
            binding: binding.clone(),
            label: None,
            call_span: binding.span,
        };
        let context = identifiers();
        let marker = ref_marker_name(&context, &definition).unwrap();
        let (anchor, anchor_symbol) = render_ref_projection(
            &context,
            &definition,
            &RefProjection::AsAnchor(None),
            &ModuleId::from("x"),
        )
        .unwrap();
        let reference = anchor.strip_prefix("anchor-name:").unwrap();
        let anchor_symbol = anchor_symbol.expect("anchor name symbol");
        assert_eq!(anchor_symbol.pool, IdentifierPool::AnchorName);
        assert_eq!(
            anchor_symbol.identity,
            identity_input_parts(&ref_context(&definition, None))
        );
        let (position, _) = render_ref_projection(
            &context,
            &definition,
            &RefProjection::PositionAnchor,
            &ModuleId::from("x"),
        )
        .unwrap();
        assert_eq!(
            position,
            format_args!("position-anchor:{reference}").to_lean_string()
        );
        let scope_projection =
            render_anchor_scope(&context, &AnchorScope::Refs(vec![definition.clone()])).unwrap();
        assert_eq!(
            scope_projection,
            format_args!("anchor-scope:{reference}").to_lean_string()
        );
        let (container, container_symbol) = render_ref_projection(
            &context,
            &definition,
            &RefProjection::AsContainer(Some(Spanned::new(ContainerType::Size, binding.span))),
            &ModuleId::from("x"),
        )
        .unwrap();
        // A container name is its own family, so it is not the anchor's spelling.
        let container_name = container
            .strip_prefix("container-name:")
            .and_then(|rest| rest.strip_suffix("container-type:size;"))
            .expect("container declaration");
        assert!(container_name.starts_with("sw-c-"), "{container_name}");
        assert_ne!(container_name, reference);
        let container_symbol = container_symbol.expect("container name symbol");
        assert_eq!(container_symbol.pool, IdentifierPool::ContainerName);
        assert_eq!(
            container_symbol.identity,
            identity_input_parts(&ref_context(&definition, None))
        );
        definition.label = Some(binding);
        // A label is spelled into the name, so the same declaration wears a different
        // marker once it has one; the identity the name hashes does not move.
        assert_ne!(ref_marker_name(&context, &definition).unwrap(), marker);
    }
}
