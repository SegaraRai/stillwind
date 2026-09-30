//! Ref projections, static and dynamic, and the declarations they render.

use stillwind_design_system::UtilityOrder;
use stillwind_diagnostics::CompileErrorKind;
use stillwind_ir::{AnchorScope, RefDefinition, RefProjection, StyleVariant};
use stillwind_source::{ModuleId, Span};

use super::super::fragment::{Emitted, ValueExpr};
use super::super::literal::LiteralContribution;
use super::super::props::GuardStep;
use super::super::style_variants::StyleVariantPlan;
use super::super::symbols::{
    anchor_name, identity_input_parts, projection_symbol, ref_container_name, ref_context,
    ref_identity, ref_marker_name, ref_projection_name,
};
use super::super::utilities::resolve::CompiledUtility;
use super::CallEmitter;
use crate::GeneratedSymbol;
use crate::generated::Code;
use crate::identifiers::{IdentifierContext, IdentifierPool};

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
        let plan =
            StyleVariantPlan::resolve(self.identifiers, wrappers, self.theme, self.diagnostics)?;
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
                Ok(marker) => Some(format!("&:where({})", marker.selector())),
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
        let gate = if matches!(projection, RefProjection::PositionAnchor) {
            None
        } else {
            ref_marker_name(self.identifiers, definition)
                .ok()
                .map(|marker| marker.as_str().to_owned())
        };
        self.record_property_writes(&contribution, gate.is_some(), span, guard, gate.as_deref());
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
        for definition in refs {
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
            self.record_property_writes(
                &contribution,
                provider,
                *value,
                guard,
                provider.then_some(marker.as_str()),
            );
            candidates.push((marker.as_str().to_owned(), contribution));
        }
        self.emit_candidate_units(value, super::CLASS_KEY, candidates, guard)
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
        let plan =
            StyleVariantPlan::resolve(self.identifiers, wrappers, self.theme, self.diagnostics)?;
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
        self.record_property_writes(&contribution, true, span, guard, None);
        self.contribute_rule(contribution, guard, fold)
    }
}

/// Builds one authored utility from `declaration` and the optional `selector`.
/// The selector, when present, requires the receiver's marker. The function includes
/// both strings and the variant `plan` in the canonical identity, then applies the
/// plan to the utility. It does not allocate names.
fn rule_contribution(
    plan: &StyleVariantPlan,
    declaration: String,
    selector: Option<String>,
) -> LiteralContribution {
    let canonical = format!(
        "{}\0ref-use\0{}\0{}\0",
        plan.canonical,
        selector.as_deref().unwrap_or(""),
        declaration
    );
    let mut utility = CompiledUtility {
        required_properties: Vec::new(),
        variants: Vec::new(),
        variant_applications: Vec::new(),
        declaration,
        selector,
        container: None,
        nested: Vec::new(),
        order: UtilityOrder::Authored(None),
        candidate: String::new(),
    };
    plan.apply_rule(&mut utility);
    LiteralContribution {
        canonical,
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
) -> Result<String, CompileErrorKind> {
    let value = match scope {
        AnchorScope::All => "all".to_owned(),
        AnchorScope::None => "none".to_owned(),
        AnchorScope::Refs(refs) => {
            if refs.is_empty() {
                return Ok(String::new());
            }
            refs.iter()
                .map(|definition| {
                    anchor_name(context, definition, None).map(|name| name.dashed_ident())
                })
                .collect::<Result<Vec<_>, _>>()?
                .join(",")
        }
    };
    Ok(format!("anchor-scope:{value};"))
}

/// Returns an expression that will read the marker token from a compiled ref's `c`
/// field. The caller must supply code that yields a compiled ref record. This
/// function does not evaluate or validate that code.
pub(super) fn ref_class_read(value: Code) -> ValueExpr {
    ValueExpr::RefClassRead { value }
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
) -> Result<(String, Option<GeneratedSymbol>), CompileErrorKind> {
    let name = anchor_name(context, definition, Some(ref_projection_name(projection)))?;
    let symbol = |name: &str, pool: IdentifierPool, kind: &str| {
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
            format!(
                "anchor-name:{};{}",
                name.dashed_ident(),
                scope
                    .as_ref()
                    .map(|scope| render_anchor_scope(context, scope))
                    .transpose()?
                    .unwrap_or_default()
            ),
            symbol(name.as_str(), IdentifierPool::AnchorName, "anchor"),
        ),
        RefProjection::PositionAnchor => {
            (format!("position-anchor:{};", name.dashed_ident()), None)
        }
        RefProjection::AsContainer(kind) => {
            let container = ref_container_name(context, definition)?;
            (
                format!(
                    "container-name:{};{}",
                    container.custom_ident(),
                    kind.as_ref()
                        .map(|kind| format!("container-type:{};", kind.value))
                        .unwrap_or_default()
                ),
                Some(projection_symbol(
                    container.as_str(),
                    identity_input_parts(&ref_context(definition, None)),
                    IdentifierPool::ContainerName,
                    "container",
                    module_id,
                    definition.call_span,
                )),
            )
        }
    })
}

#[cfg(test)]
mod tests {
    use stillwind_ir::BoundaryRef;
    use stillwind_source::{FileId, Span, Spanned};

    use super::*;
    use crate::compiler::symbols::{boundary_marker_name, ref_marker_name};
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
    fn projections_share_identity_and_boundaries_name_their_own_declaration() {
        let binding = Spanned::new("ref".to_owned(), Span::new(FileId(1), 0, 3));
        let mut definition = RefDefinition {
            module_id: ModuleId::from("x"),
            binding: binding.clone(),
            label: None,
            boundary: None,
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
        assert_eq!(position, format!("position-anchor:{reference}"));
        let scope_projection =
            render_anchor_scope(&context, &AnchorScope::Refs(vec![definition.clone()])).unwrap();
        assert_eq!(scope_projection, format!("anchor-scope:{reference}"));
        let (container, container_symbol) = render_ref_projection(
            &context,
            &definition,
            &RefProjection::AsContainer(Some(Spanned::new("size".to_owned(), binding.span))),
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
        definition.boundary = Some(BoundaryRef {
            module_id: ModuleId::from("x"),
            binding: binding.clone(),
            label: Some(binding.clone()),
            call_span: binding.span,
        });
        let boundary =
            boundary_marker_name(&context, definition.boundary.as_ref().unwrap()).unwrap();
        // A boundary is a ref declaration, so the element carrying it wears that
        // declaration's own marker class.
        assert_eq!(
            boundary.as_str(),
            ref_marker_name(
                &context,
                &RefDefinition {
                    module_id: ModuleId::from("x"),
                    binding: binding.clone(),
                    label: Some(binding.clone()),
                    boundary: None,
                    call_span: binding.span,
                }
            )
            .unwrap()
            .as_str()
        );
        definition.label = Some(binding);
        // A label is spelled into the name, so the same declaration wears a different
        // marker once it has one; the identity the name hashes does not move.
        assert_ne!(ref_marker_name(&context, &definition).unwrap(), marker);
    }
}
