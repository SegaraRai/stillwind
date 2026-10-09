//! Small bridges between Oxc spans and editable-source spans.
//!
//! Oxc spans are tagged with the file id of the buffer being analysed. That buffer
//! may be a normalized projection; the projection pipeline subsequently maps its
//! local spans to the editable original. Re-tagging alone does not map coordinates.

use oxc::ast::ast::{Expression, PropertyKey, StringLiteral};

use stillwind_ir::AssetReference;
use stillwind_semantic::{AssetKind, IdentityId};
use stillwind_source::{Span, Spanned};

use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    /// Returns a string literal's decoded value with the span of its *contents*.
    ///
    /// The quotes are excluded, so the span can be sliced back out of the source and
    /// compared with the value. Decoded escapes may make the value shorter than the
    /// span; producers that need byte-exact text keep the raw source instead.
    pub(crate) fn string_literal(&self, literal: &StringLiteral<'a>) -> Spanned<String> {
        let literal_span = self.source_span(literal.span);
        let content_span = Span::new(
            literal_span.file,
            literal_span.start.0 + 1,
            literal_span.end.0.saturating_sub(1),
        );
        Spanned::new(literal.value.to_string(), content_span)
    }

    /// Re-tags an Oxc span with the file id of the analysed source.
    pub(crate) fn source_span(&self, span: oxc::span::Span) -> Span {
        Span::new(self.source.id(), span.start, span.end)
    }

    /// Pairs an external declaration identity with a genuine local use-site span.
    pub(crate) fn asset_reference(
        &self,
        identity: &IdentityId,
        span: oxc::span::Span,
    ) -> AssetReference {
        let span = self.source_span(span);
        AssetReference {
            identity: identity.clone(),
            span,
            label: self.declaration_label(identity, span),
        }
    }

    /// The authored label of one declaration identity, as this module can read it.
    ///
    /// A declaration in this module carries its own label; a declaration in another
    /// module carries the label the host published in that producer's asset metadata.
    /// Only keyframes and view transitions spell a label into a generated name, so
    /// other kinds yield none.
    pub(crate) fn declaration_label(
        &self,
        identity: &IdentityId,
        span: Span,
    ) -> Option<Spanned<String>> {
        let declared = match identity.kind() {
            AssetKind::Keyframes => self
                .keyframes_declarations
                .iter()
                .find(|declaration| {
                    self.identity(AssetKind::Keyframes, &declaration.binding) == *identity
                })
                .and_then(|declaration| declaration.label.clone()),
            AssetKind::ViewTransition => self
                .view_transition_declarations
                .iter()
                .find(|declaration| {
                    self.identity(AssetKind::ViewTransition, &declaration.binding) == *identity
                })
                .and_then(|declaration| declaration.label.clone()),
            _ => None,
        };
        declared.or_else(|| {
            self.imported_assets
                .get(identity)
                .and_then(|metadata| metadata.label.clone())
                .map(|label| Spanned::new(label, span))
        })
    }

    /// Returns a property key as name plus span, for identifier and string keys.
    ///
    /// Computed and numeric keys return `None`; callers decide whether that is an error
    /// or simply an entry they do not understand.
    pub(crate) fn property_name(&self, key: &PropertyKey<'a>) -> Option<Spanned<String>> {
        match key {
            PropertyKey::StaticIdentifier(identifier) => Some(Spanned::new(
                identifier.name.to_string(),
                self.source_span(identifier.span),
            )),
            PropertyKey::StringLiteral(literal) => Some(self.string_literal(literal)),
            _ => None,
        }
    }

    /// Resolves literal object keys, including expression-free template keys.
    pub(crate) fn static_property_name(&self, key: &PropertyKey<'a>) -> Option<Spanned<String>> {
        if let Some(name) = self.property_name(key) {
            return Some(name);
        }
        let expression = key.as_expression()?.get_inner_expression();
        match expression {
            Expression::StringLiteral(literal) => Some(self.string_literal(literal)),
            Expression::TemplateLiteral(template) if template.expressions.is_empty() => {
                let value = template.quasis.first()?.value.cooked.as_ref()?.to_string();
                let span = self.source_span(template.span);
                Some(Spanned::new(
                    value,
                    Span::new(span.file, span.start.0 + 1, span.end.0 - 1),
                ))
            }
            _ => None,
        }
    }
}
