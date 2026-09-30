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
    /// Builds one receiver's CSS contribution without attaching its marker.
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

    /// Contributes a statically resolved use to its current unit or region.
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

    /// Selects a finite candidate's CSS unit using the receiver's marker token.
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

    /// Emits explicit scope CSS without a receiver gate or marker attachment.
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

/// Serializes an explicit scope union; an empty list emits no declaration.
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

/// Reads the canonical class token off a compiled ref or scope value.
///
/// Every ref value carries its marker under `c`, so attachments emit a direct property
/// read. The returned value is a known class string.
pub(super) fn ref_class_read(value: Code) -> ValueExpr {
    ValueExpr::RefClassRead { value }
}

/// Returns `(CSS declaration, optional generated-name symbol)` for a projection applied
/// to the element itself. The symbol is present only when the projection writes an
/// anchor or container name.
///
/// An anchor projection writes the declaration's `anchor-name`, and a container
/// projection writes its `container-name`, which is also the name `at()` queries; both
/// are written for the ref's identity, so one declaration legitimately appears in two
/// CSS namespaces.
///
/// # Errors
///
/// Returns the host generator's failure for the anchor or container family.
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
