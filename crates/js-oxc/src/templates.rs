//! Direct utility templates retain each ref substitution as an IR variant.
//!
//! A utility template is a whitespace-separated list in which a ref may occupy a
//! complete variant segment, as in `` `md:${trigger`hover:`}:active:p-4` ``. The
//! scanner rebuilds those segments into nested variant wrappers, so each utility
//! keeps its own source span and the substitution stays a ref variant rather
//! than becoming text.
//!
//! The accepted grammar is narrow on purpose. A substitution must be a direct
//! ref call or tag, the text after it must begin with `:`, and any text before it
//! must already be a complete `variant:` prefix; anything else fails with
//! `SHJS005` instead of guessing how the author meant to splice a string.

use oxc::ast::ast::TemplateLiteral;

use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_ir::{ClassExpression, RuleExpression, StyleVariant};
use stillwind_source::{Span, Spanned};

use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    /// Lowers a template into one class expression per `variant:utility` segment.
    ///
    /// Returns `None` after pushing a diagnostic when a substitution is not a ref
    /// variant or a segment is incomplete, so a caller can treat the whole template as
    /// unusable.
    pub(crate) fn utility_template_expressions(
        &mut self,
        template: &TemplateLiteral<'a>,
    ) -> Option<Vec<ClassExpression>> {
        let mut token = TemplateToken::default();
        let mut collected = Vec::new();
        for (index, quasi) in template.quasis.iter().enumerate() {
            let Some(text) = quasi.value.cooked.as_ref() else {
                self.diagnostics
                    .push(template_error(self.source_span(quasi.span)));
                return None;
            };
            let span = self.source_span(quasi.span);
            let mut start = 0;
            for (offset, character) in text.char_indices() {
                if !character.is_whitespace() {
                    continue;
                }
                if let Err(error) = token
                    .text(&text[start..offset], subspan(span, start, offset))
                    .and_then(|()| token.finish(&mut collected, span))
                {
                    self.diagnostics.push(error);
                    return None;
                }
                start = offset + character.len_utf8();
            }
            if let Err(error) = token.text(&text[start..], subspan(span, start, text.len())) {
                self.diagnostics.push(error);
                return None;
            }
            if let Some(expression) = template.expressions.get(index) {
                let Some(variant) = self.ref_variant(expression) else {
                    self.diagnostics
                        .push(template_error(self.source_span(template.span)));
                    return None;
                };
                if let Err(error) = token.reference(variant, span) {
                    self.diagnostics.push(error);
                    return None;
                }
            }
        }
        if let Err(error) = token.finish(&mut collected, self.source_span(template.span)) {
            self.diagnostics.push(error);
            return None;
        }
        Some(collected)
    }
}

/// Accumulator for the segment currently being read.
///
/// [`Self::needs_separator`] is set after a ref substitution; the next text must then
/// start with `:`, which is what keeps `${ref}` from silently gluing onto the
/// surrounding utility text.
#[derive(Default)]
struct TemplateToken {
    /// Variants collected for the current segment, outermost first.
    variants: Vec<StyleVariant>,
    /// Literal text still to be assigned to the segment's utility.
    tail: Option<Spanned<String>>,
    /// True while the next text must begin with `:`.
    needs_separator: bool,
}

impl TemplateToken {
    /// Feeds literal text into the current segment.
    ///
    /// Text arrives already split on whitespace. When a separator is required, exactly
    /// one leading `:` is consumed and the span is advanced so it still points at the
    /// text the value came from.
    fn text(&mut self, text: &str, mut span: Span) -> Result<(), CompileError> {
        if text.is_empty() {
            return Ok(());
        }
        let text = if self.needs_separator {
            let Some(text) = text.strip_prefix(':') else {
                return Err(template_error(span));
            };
            self.needs_separator = false;
            span.start.0 += 1;
            text
        } else {
            text
        };
        if !text.is_empty() {
            self.tail = Some(Spanned::new(text.to_owned(), span));
        }
        Ok(())
    }

    /// Ends the current text run and starts a substitution.
    ///
    /// The text before a substitution is a variant prefix only when it ends with `:`,
    /// so `` `${trigger`hover:`}x` `` is rejected rather than read as a prefix.
    fn reference(&mut self, variant: StyleVariant, span: Span) -> Result<(), CompileError> {
        if self.needs_separator {
            return Err(template_error(span));
        }
        if let Some(prefix) = self.tail.take() {
            if !prefix.value.ends_with(':') {
                return Err(template_error(prefix.span));
            }
            self.variants.push(StyleVariant::Prefix(prefix));
        }
        self.variants.push(variant);
        self.needs_separator = true;
        Ok(())
    }

    /// Closes the current segment and appends the wrapped expression.
    ///
    /// A segment with variants but no utility, or an unterminated separator, is a
    /// template error; wrappers are applied innermost-first so the emitted expression
    /// nests in source order.
    fn finish(
        &mut self,
        expressions: &mut Vec<ClassExpression>,
        span: Span,
    ) -> Result<(), CompileError> {
        if self.needs_separator || (!self.variants.is_empty() && self.tail.is_none()) {
            return Err(template_error(span));
        }
        if let Some(value) = self.tail.take() {
            let mut expression = RuleExpression::Utilities(value);
            for variant in self.variants.drain(..).rev() {
                expression = RuleExpression::Variant {
                    variant,
                    inputs: vec![expression],
                };
            }
            expressions.push(expression.into());
        }
        Ok(())
    }
}

/// Re-bases a byte interval inside a quasi onto the quasi's source span.
///
/// Cooked text can be shorter than the original spelling, so offsets are clamped into
/// the literal: a diagnostic may be imprecise but never points outside the string it
/// is about.
fn subspan(span: Span, start: usize, end: usize) -> Span {
    // Cooked text is no longer than the original spelling. Keep diagnostics inside the
    // literal even when JavaScript escape sequences were decoded.
    Span::new(
        span.file,
        span.start
            .0
            .saturating_add(u32::try_from(start).unwrap_or(u32::MAX))
            .min(span.end.0),
        span.start
            .0
            .saturating_add(u32::try_from(end).unwrap_or(u32::MAX))
            .min(span.end.0),
    )
}

/// The single diagnostic for every rejection path in this module.
fn template_error(span: Span) -> CompileError {
    CompileError::new(CompileErrorKind::InvalidUtilityTemplate, span)
}
