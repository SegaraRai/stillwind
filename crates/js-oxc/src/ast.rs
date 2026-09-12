//! Small bridges between Oxc spans and editable-source spans.
//!
//! Oxc spans are tagged with the file id of the buffer being analysed. That buffer
//! may be a normalized projection; the projection pipeline subsequently maps its
//! local spans to the editable original. Re-tagging alone does not map coordinates.

use oxc::ast::ast::{PropertyKey, StringLiteral};

use stillwind_ir::AssetReference;
use stillwind_semantic::IdentityId;
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

    /// Builds an IR asset reference from an identity declared in another module.
    ///
    /// The name carries declaration offsets but is tagged with the current buffer id.
    /// It is identity metadata paired with `module_path`, not a directly sliceable
    /// diagnostic span in the importing module.
    pub(crate) fn asset_reference(&self, identity: &IdentityId) -> AssetReference {
        AssetReference {
            module_path: identity.module_path.clone(),
            name: Spanned::new(
                identity.binding_name.clone(),
                Span::new(self.source.id(), identity.start, identity.end),
            ),
        }
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
}
