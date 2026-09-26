//! Ref projections, static and dynamic, and the declarations they render.

use stillwind_design_system::UtilityOrder;
use stillwind_diagnostics::CompileErrorKind;
use stillwind_ir::{RefDefinition, RefProjection, StyleVariant};
use stillwind_source::Span;

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
    /// Lowers one static ref projection onto the element the projection is written on.
    ///
    /// The emitted class read uses the receiver expression the use site wrote, so an
    /// alias or member receiver names the same declaration. Returns the marker class
    /// read, or `None` if variant resolution, source slicing, or generated-name
    /// allocation fails.
    pub(super) fn emit_ref_projection(
        &mut self,
        definition: &RefDefinition,
        value: &Span,
        projection: &RefProjection,
        guard: &[GuardStep],
        wrappers: &[StyleVariant],
    ) -> Option<Emitted> {
        let plan =
            StyleVariantPlan::resolve(self.identifiers, wrappers, self.theme, self.diagnostics)?;
        let name = ref_projection_name(projection);
        // The projection reads the declaration's canonical token, so the
        // declaration has to keep its runtime value.
        self.runtime_assets.insert(ref_identity(definition));
        let anchor = match anchor_name(self.identifiers, definition, Some(name)) {
            Ok(anchor) => anchor,
            Err(failure) => {
                self.reject_name(failure, self.call.span);
                return None;
            }
        };
        let marker = match ref_marker_name(self.identifiers, definition) {
            Ok(marker) => marker,
            Err(failure) => {
                self.reject_name(failure, self.call.span);
                return None;
            }
        };
        let (declaration, name_symbol) =
            match render_ref_projection(self.identifiers, definition, projection, self.module_id) {
                Ok(rendered) => rendered,
                Err(failure) => {
                    self.reject_name(failure, self.call.span);
                    return None;
                }
            };
        if let Some(symbol) = name_symbol {
            self.written_name_symbols.push(symbol);
        }
        let mut utility = CompiledUtility {
            required_properties: Vec::new(),
            variants: Vec::new(),
            variant_applications: Vec::new(),
            declaration,
            selector: Some(format!("&:where({})", marker.selector())),
            container: None,
            nested: Vec::new(),
            order: UtilityOrder::Authored(None),
            candidate: String::new(),
        };
        plan.apply_rule(&mut utility);
        let contribution = LiteralContribution {
            canonical: format!("{}ref:{}:{name}\u{0}", plan.canonical, anchor.as_str()),
            utilities: vec![utility],
            markers: Vec::new(),
            external: Vec::new(),
            plain: Vec::new(),
            view_transition_token: None,
            view_transition_name: None,
        };
        self.contribute_rule(contribution, guard, true);
        Some(Emitted::Expression(ref_class_read(
            self.source_expression(*value)?,
        )))
    }

    /// Lowers one dynamic ref projection using each finite candidate for CSS planning.
    ///
    /// Every statically known ref contributes a marker token. Its projection rule is
    /// gated by that marker within the surrounding class unit. Returns the marker class
    /// read, or `None` if variant resolution, source slicing, or generated-name
    /// allocation fails. An empty finite set emits no projection CSS but still reads
    /// the runtime ref value.
    pub(super) fn emit_dynamic_ref_projection(
        &mut self,
        value: &Span,
        refs: &[RefDefinition],
        projection: &RefProjection,
        guard: &[GuardStep],
        wrappers: &[StyleVariant],
    ) -> Option<Emitted> {
        let plan =
            StyleVariantPlan::resolve(self.identifiers, wrappers, self.theme, self.diagnostics)?;
        for definition in refs {
            self.runtime_assets.insert(ref_identity(definition));
            let marker = match ref_marker_name(self.identifiers, definition) {
                Ok(marker) => marker,
                Err(failure) => {
                    self.reject_name(failure, self.call.span);
                    return None;
                }
            };
            let anchor = match anchor_name(self.identifiers, definition, Some("dynamic")) {
                Ok(anchor) => anchor,
                Err(failure) => {
                    self.reject_name(failure, self.call.span);
                    return None;
                }
            };
            let canonical = format!(
                "{}dynamic-ref:{}:{}",
                plan.canonical,
                anchor.as_str(),
                ref_projection_name(projection)
            );
            let (declaration, name_symbol) = match render_ref_projection(
                self.identifiers,
                definition,
                projection,
                self.module_id,
            ) {
                Ok(rendered) => rendered,
                Err(failure) => {
                    self.reject_name(failure, self.call.span);
                    return None;
                }
            };
            if let Some(symbol) = name_symbol {
                self.written_name_symbols.push(symbol);
            }
            let mut utility = CompiledUtility {
                required_properties: Vec::new(),
                variants: Vec::new(),
                variant_applications: Vec::new(),
                declaration,
                selector: Some(format!("&:where({})", marker.selector())),
                container: None,
                nested: Vec::new(),
                order: UtilityOrder::Authored(None),
                candidate: String::new(),
            };
            plan.apply_rule(&mut utility);
            let contribution = LiteralContribution {
                canonical,
                utilities: vec![utility],
                markers: Vec::new(),
                external: Vec::new(),
                plain: Vec::new(),
                view_transition_token: None,
                view_transition_name: None,
            };
            self.contribute_rule(contribution, guard, true);
        }
        Some(Emitted::Expression(ref_class_read(
            self.source_expression(*value)?,
        )))
    }
}

/// Reads the canonical class token off a compiled ref or scope value.
///
/// Every ref value carries its marker under `c`, so attachments and projections emit a
/// direct property read. The returned value is a known class string.
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
    source_path: &str,
) -> Result<(String, Option<GeneratedSymbol>), CompileErrorKind> {
    let name = anchor_name(context, definition, Some(ref_projection_name(projection)))?;
    let symbol = |name: &str, pool: IdentifierPool, kind: &str| {
        Some(projection_symbol(
            name,
            identity_input_parts(&ref_context(definition, None)),
            pool,
            kind,
            source_path,
            definition.call_span,
        ))
    };
    Ok(match projection {
        RefProjection::Anchor => (
            format!("anchor-name:{};", name.dashed_ident()),
            symbol(name.as_str(), IdentifierPool::AnchorName, "anchor"),
        ),
        RefProjection::PositionAnchor => {
            (format!("position-anchor:{};", name.dashed_ident()), None)
        }
        RefProjection::AnchorScope => (format!("anchor-scope:{};", name.dashed_ident()), None),
        RefProjection::Container(kind) => {
            let container = ref_container_name(context, definition)?;
            (
                format!(
                    "container-name:{};container-type:{};",
                    container.custom_ident(),
                    kind.value
                ),
                Some(projection_symbol(
                    container.as_str(),
                    identity_input_parts(&ref_context(definition, None)),
                    IdentifierPool::ContainerName,
                    "container",
                    source_path,
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
        let binding = Spanned::new("ref".into(), Span::new(FileId(1), 0, 3));
        let mut definition = RefDefinition {
            module_path: "x".into(),
            binding: binding.clone(),
            label: None,
            boundary: None,
            call_span: binding.span,
        };
        let context = identifiers();
        let marker = ref_marker_name(&context, &definition).unwrap();
        let (anchor, anchor_symbol) =
            render_ref_projection(&context, &definition, &RefProjection::Anchor, "x").unwrap();
        let reference = anchor.strip_prefix("anchor-name:").unwrap();
        let anchor_symbol = anchor_symbol.expect("anchor name symbol");
        assert_eq!(anchor_symbol.pool, IdentifierPool::AnchorName);
        assert_eq!(
            anchor_symbol.identity,
            identity_input_parts(&ref_context(&definition, None))
        );
        let (position, _) =
            render_ref_projection(&context, &definition, &RefProjection::PositionAnchor, "x")
                .unwrap();
        assert_eq!(position, format!("position-anchor:{reference}"));
        let (scope_projection, _) =
            render_ref_projection(&context, &definition, &RefProjection::AnchorScope, "x").unwrap();
        assert_eq!(scope_projection, format!("anchor-scope:{reference}"));
        let (container, container_symbol) = render_ref_projection(
            &context,
            &definition,
            &RefProjection::Container(Spanned::new("size".into(), binding.span)),
            "x",
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
            module_path: "x".into(),
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
                    module_path: "x".into(),
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
