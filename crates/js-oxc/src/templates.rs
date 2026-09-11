//! Direct utility templates retain each ref substitution as an IR variant.

use oxc::ast::ast::TemplateLiteral;

use stillwind_ir::{StyleInput, StyleVariant};
use stillwind_source::{CompileError, Span, Spanned};

use crate::state::MacroExtractor;

impl<'a> MacroExtractor<'a> {
    pub(crate) fn collect_utility_template(
        &mut self,
        template: &TemplateLiteral<'a>,
        inputs: &mut Vec<StyleInput>,
    ) {
        let mut token = TemplateToken::default();
        let mut collected = Vec::new();
        for (index, quasi) in template.quasis.iter().enumerate() {
            let Some(text) = quasi.value.cooked.as_ref() else {
                self.diagnostics
                    .push(template_error(self.source_span(quasi.span)));
                return;
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
                    return;
                }
                start = offset + character.len_utf8();
            }
            if let Err(error) = token.text(&text[start..], subspan(span, start, text.len())) {
                self.diagnostics.push(error);
                return;
            }
            if let Some(expression) = template.expressions.get(index) {
                let Some(variant) = self.ref_variant(expression) else {
                    self.diagnostics
                        .push(template_error(self.source_span(template.span)));
                    return;
                };
                if let Err(error) = token.reference(variant, span) {
                    self.diagnostics.push(error);
                    return;
                }
            }
        }
        if let Err(error) = token.finish(&mut collected, self.source_span(template.span)) {
            self.diagnostics.push(error);
            return;
        }
        inputs.extend(collected);
    }
}

#[derive(Default)]
struct TemplateToken {
    variants: Vec<StyleVariant>,
    tail: Option<Spanned<String>>,
    needs_separator: bool,
}

impl TemplateToken {
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

    fn finish(&mut self, inputs: &mut Vec<StyleInput>, span: Span) -> Result<(), CompileError> {
        if self.needs_separator || (!self.variants.is_empty() && self.tail.is_none()) {
            return Err(template_error(span));
        }
        if let Some(value) = self.tail.take() {
            let mut input = StyleInput::UtilityString(value);
            for variant in self.variants.drain(..).rev() {
                input = StyleInput::Variant {
                    variant,
                    inputs: vec![input],
                };
            }
            inputs.push(input);
        }
        Ok(())
    }
}

fn subspan(span: Span, start: usize, end: usize) -> Span {
    // Cooked text is no longer than the original spelling. Keep diagnostics
    // inside the literal even when JavaScript escape sequences were decoded.
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

fn template_error(span: Span) -> CompileError {
    CompileError::new(
        "SHJS005",
        "utility templates require direct ref variants in complete `variant:utility` segments",
        span,
    )
}
