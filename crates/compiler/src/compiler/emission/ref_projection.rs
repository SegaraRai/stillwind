//! Ref projections, static and dynamic, and the declarations they render.

use stillwind_design_system::UtilityOrder;
use stillwind_ir::{RefDefinition, RefProjection};
use stillwind_source::Span;

use super::super::fragment::{Emitted, ValueExpr};
use super::super::handles::{ref_identity, render_ref_projection};
use super::super::literal::LiteralContribution;
use super::super::props::GuardStep;
use super::super::symbols::{anchor_name, ref_marker_name, ref_projection_name};
use super::super::utilities::resolve::CompiledUtility;
use super::{CLASS_KEY, CallEmitter};
use crate::generated::{Code, code, js_string};

impl CallEmitter<'_> {
    /// Lowers one static ref projection onto the element the projection is written on.
    ///
    /// The declaration keeps its runtime value: the emitted lookup reads the canonical token
    /// off it, so the projection needs no runtime helper of its own.
    pub(super) fn emit_ref_projection(
        &mut self,
        definition: &RefDefinition,
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
        // The projection is read through the declaration's canonical token, so
        // the element keeps its marker class and the caller needs no helper.
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
            value: Box::new(ValueExpr::Source(definition.binding.value.clone().into())),
            key: "c".to_owned(),
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
            key: "c".to_owned(),
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
