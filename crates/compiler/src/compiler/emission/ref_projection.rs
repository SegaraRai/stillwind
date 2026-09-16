//! Ref projections, static and dynamic, and the declarations they render.

use stillwind_design_system::UtilityOrder;
use stillwind_diagnostics::CompileErrorKind;
use stillwind_ir::{RefDefinition, RefProjection};
use stillwind_source::Span;

use super::super::fragment::{Emitted, ValueExpr};
use super::super::literal::LiteralContribution;
use super::super::props::GuardStep;
use super::super::symbols::{
    anchor_name, identity_input_parts, projection_symbol, ref_context, ref_identity,
    ref_marker_name, ref_projection_name,
};
use super::super::utilities::resolve::CompiledUtility;
use super::{CLASS_KEY, CallEmitter};
use crate::GeneratedSymbol;
use crate::generated::{Code, code, js_string};
use crate::identifiers::{IdentifierContext, IdentifierPool};

impl CallEmitter<'_> {
    /// Lowers one static ref projection onto the element the projection is written on.
    ///
    /// The declaration keeps its runtime value: the emitted lookup reads the canonical token
    /// off the receiver expression the use site wrote, so the projection needs no runtime
    /// helper of its own and an alias or member receiver names the same declaration.
    pub(super) fn emit_ref_projection(
        &mut self,
        definition: &RefDefinition,
        value: &Span,
        projection: &RefProjection,
        guard: &[GuardStep],
    ) -> Option<Emitted> {
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
        let (declaration, name_symbol) = match render_ref_projection(
            self.identifiers,
            definition,
            projection,
            self.source.path(),
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
        let utility = CompiledUtility {
            required_properties: Vec::new(),
            variants: Vec::new(),
            variant_applications: Vec::new(),
            declaration,
            selector: None,
            container: None,
            nested: Vec::new(),
            order: UtilityOrder::Authored(None),
            candidate: String::new(),
        };
        let contribution = LiteralContribution {
            canonical: format!("ref:{}:{name}\u{0}", anchor.as_str()),
            utilities: vec![utility],
            markers: Vec::new(),
            external: Vec::new(),
            plain: Vec::new(),
            view_transition_token: None,
            view_transition_name: None,
        };
        // The projection is read through the token the receiver carries, so the element
        // keeps its marker class and the caller needs no helper.
        let class = match self.units.leaf(contribution, guard) {
            Ok(leaf) => leaf,
            Err(failure) => {
                self.reject_name(failure, self.call.span);
                return None;
            }
        };
        let Emitted::Leaf { mut names } = class else {
            return None;
        };
        names.push(marker.as_str().to_owned());
        let entry = format!(
            "{}:{}",
            marker.javascript_string(),
            js_string(&names.join(" "))
        );
        Some(Emitted::Expression(ValueExpr::Lookup {
            value: Box::new(ValueExpr::Source(self.source_expression(*value)?)),
            key: CLASS_KEY.to_owned(),
            table: format!("{{{entry}}}").into(),
        }))
    }

    /// Lowers one dynamic ref projection by building the table the runtime value selects from.
    ///
    /// Every statically known ref contributes its marker token and the class set of its own
    /// projection, so the runtime reads one entry instead of recomputing declarations.
    pub(super) fn emit_dynamic_ref_projection(
        &mut self,
        value: &Span,
        refs: &[RefDefinition],
        projection: &RefProjection,
        guard: &[GuardStep],
    ) -> Option<Emitted> {
        let mut table = Vec::new();
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
                "dynamic-ref:{}:{}",
                anchor.as_str(),
                ref_projection_name(projection)
            );
            let (declaration, name_symbol) = match render_ref_projection(
                self.identifiers,
                definition,
                projection,
                self.source.path(),
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
            let contribution = LiteralContribution {
                canonical,
                utilities: vec![CompiledUtility {
                    required_properties: Vec::new(),
                    variants: Vec::new(),
                    variant_applications: Vec::new(),
                    declaration,
                    selector: None,
                    container: None,
                    nested: Vec::new(),
                    order: UtilityOrder::Authored(None),
                    candidate: String::new(),
                }],
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
            table.push(format!("{}:{class}", marker.javascript_string()));
        }
        let table = table.join(",");
        let expression = ValueExpr::Lookup {
            value: Box::new(ValueExpr::Source(self.source_expression(*value)?)),
            key: CLASS_KEY.to_owned(),
            table: format!("{{{table}}}").into(),
        };
        Some(Emitted::Expression(expression))
    }
}

/// Reads the canonical class token off a compiled ref or scope value.
///
/// Every ref value carries its token under `c`, so an attachment is a property read and
/// needs no runtime helper.
pub(super) fn ref_class_read(value: Code) -> ValueExpr {
    ValueExpr::Source(code!["(", value, ").", CLASS_KEY])
}

/// The declaration that makes a projection usable on the element itself, and the
/// reportable symbol for the name it writes.
///
/// An anchor projection writes the declaration's `anchor-name`, and a container
/// projection writes its `container-name`; both names are written for the ref's
/// identity, so one declaration can legitimately appear in two CSS namespaces under
/// the same spelling.
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
            format!("anchor-name:{};", name.custom_ident()),
            symbol(name.as_str(), IdentifierPool::AnchorName, "anchor"),
        ),
        RefProjection::Position => (format!("position-anchor:{};", name.custom_ident()), None),
        RefProjection::Scope => (format!("anchor-scope:{};", name.custom_ident()), None),
        RefProjection::Container(kind) => {
            let container = context.container_name(&ref_context(definition, Some("container")))?;
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
    use stillwind_ir::ScopeDefinition;
    use stillwind_source::{FileId, Span, Spanned};

    use super::*;
    use crate::compiler::symbols::{ref_marker_name, scope_marker_name};
    use crate::identifiers::{IdentifierContext, IdentifierPool};

    #[test]
    fn projections_share_identity_and_scoped_selectors_bind_the_parent() {
        let binding = Spanned::new("ref".into(), Span::new(FileId(1), 0, 3));
        let mut definition = RefDefinition {
            module_path: "x".into(),
            binding: binding.clone(),
            label: None,
            scope: None,
            call_span: binding.span,
        };
        let identifiers = IdentifierContext::builtin();
        let marker = ref_marker_name(&identifiers, &definition).unwrap();
        let (anchor, anchor_symbol) =
            render_ref_projection(&identifiers, &definition, &RefProjection::Anchor, "x").unwrap();
        let reference = anchor.strip_prefix("anchor-name:").unwrap();
        let anchor_symbol = anchor_symbol.expect("anchor name symbol");
        assert_eq!(anchor_symbol.pool, IdentifierPool::AnchorName);
        assert_eq!(
            anchor_symbol.identity,
            identity_input_parts(&ref_context(&definition, None))
        );
        let (position, _) =
            render_ref_projection(&identifiers, &definition, &RefProjection::Position, "x")
                .unwrap();
        assert_eq!(position, format!("position-anchor:{reference}"));
        let (scope_projection, _) =
            render_ref_projection(&identifiers, &definition, &RefProjection::Scope, "x").unwrap();
        assert_eq!(scope_projection, format!("anchor-scope:{reference}"));
        let (container, container_symbol) = render_ref_projection(
            &identifiers,
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
        assert!(container_name.starts_with("--sw-s-"), "{container_name}");
        assert_ne!(container_name, reference);
        let container_symbol = container_symbol.expect("container name symbol");
        assert_eq!(container_symbol.pool, IdentifierPool::ContainerName);
        assert_eq!(
            container_symbol.identity,
            identity_input_parts(&ref_context(&definition, None))
        );
        definition.scope = Some(ScopeDefinition {
            module_path: "x".into(),
            binding: binding.clone(),
            label: Some(binding.clone()),
            call_span: binding.span,
        });
        let scope = scope_marker_name(&identifiers, definition.scope.as_ref().unwrap()).unwrap();
        assert!(scope.as_str().starts_with("s_"), "{scope}");
        definition.label = Some(binding);
        // The built-in name hashes the declaration alone, so a label cannot move it:
        // every position the marker is written to resolves the same declaration.
        assert_eq!(ref_marker_name(&identifiers, &definition).unwrap(), marker);
    }
}
