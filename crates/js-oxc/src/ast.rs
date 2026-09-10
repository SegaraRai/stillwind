use oxc::ast::ast::{PropertyKey, StringLiteral};

use stillwind_ir::AssetReference;
use stillwind_semantic::IdentityId;
use stillwind_source::{Span, Spanned};

use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    pub(crate) fn string_literal(&self, literal: &StringLiteral<'a>) -> Spanned<String> {
        let literal_span = self.source_span(literal.span);
        let content_span = Span::new(
            literal_span.file,
            literal_span.start.0 + 1,
            literal_span.end.0.saturating_sub(1),
        );
        Spanned::new(literal.value.to_string(), content_span)
    }

    pub(crate) fn source_span(&self, span: oxc::span::Span) -> Span {
        Span::new(self.source.id(), span.start, span.end)
    }

    pub(crate) fn asset_reference(&self, identity: &IdentityId) -> AssetReference {
        AssetReference {
            module_path: identity.module_path.clone(),
            name: Spanned::new(
                identity.binding_name.clone(),
                Span::new(self.source.id(), identity.start, identity.end),
            ),
        }
    }

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
