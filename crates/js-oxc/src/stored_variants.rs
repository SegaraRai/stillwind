//! `style.when()` declarations and the expansion of stored variants at their uses.
//!
//! A declaration stores one variant sequence. A use in a computed key, in a utility
//! template, or in the argument of another declaration expands the sequence into
//! nested [`StyleVariant`] wrappers, so the use means the same as the sequence written
//! directly at that position.
//!
//! A local declaration keeps the spans of its argument, so the compiler reports a
//! problem in the sequence at the declaration. A declaration from another module
//! arrives as a [`StoredVariantRecord`] without spans, so its variants use the span of
//! the expression at the use.

use lean_string::{LeanStr, ToLeanStr};
use oxc::ast::ast::{Argument, CallExpression, Expression, TemplateLiteral};
use oxc::span::{GetSpan, Span as OxcSpan};

use stillwind_diagnostics::{LintDiagnostic, LintFinding};
use stillwind_ir::{RefCondition, RefDefinition, StoredVariantDefinition, StyleVariant};
use stillwind_semantic::{
    AssetKind, IdentityId, StoredRefCondition, StoredVariantRecord, StoredVariantSegment,
};
use stillwind_source::{Span, Spanned};

use crate::local_span::LocalSpan;
use crate::resolution::ResolutionFailure;
use crate::state::MacroExtractor;
use crate::templates::TemplateToken;

/// The result of reading an expression as a stored variant.
pub(crate) enum StoredVariantUse {
    /// The expression is not a stored variant. The caller reports its own diagnostic.
    NotStored,
    /// The use expands to these wrappers, from the outermost to the innermost.
    Variants(Vec<StyleVariant>),
    /// The use contributes nothing, and a diagnostic was already reported for it or
    /// for its declaration.
    Omitted,
}

impl<'a> MacroExtractor<'a> {
    /// Reads one `style.when(...variants)` call that initializes `binding`.
    ///
    /// Each argument is one variant sequence. Several arguments become one
    /// [`StyleVariant::Alternatives`] wrapper, which holds when any sequence holds.
    /// The call is erased with its arguments, so the main visit does not report the ref
    /// variants inside it as unconsumed uses. An invalid argument adds a lint, and the
    /// definition then has no variants.
    pub(crate) fn collect_stored_variant(
        &mut self,
        binding: Spanned<LeanStr>,
        call: &CallExpression<'a>,
    ) -> StoredVariantDefinition {
        self.omit_input(call.span);
        self.consumed_use_spans
            .insert(LocalSpan::from(call.callee.span()));
        let variants = if call.arguments.is_empty() {
            self.stored_variant_error("the call has no arguments", call.span)
        } else {
            // Every argument is read, so each one reports its own problems.
            let sequences =
                call.arguments
                    .iter()
                    .map(|argument| match argument {
                        Argument::SpreadElement(spread) => self
                            .stored_variant_error("spread arguments are unsupported", spread.span),
                        _ => argument
                            .as_expression()
                            .and_then(|expression| self.variant_sequence(expression)),
                    })
                    .collect::<Vec<_>>();
            sequences
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .map(|mut sequences| {
                    if sequences.len() == 1 {
                        sequences.swap_remove(0)
                    } else {
                        vec![StyleVariant::Alternatives(sequences)]
                    }
                })
        };
        let key = self.declaration_key(AssetKind::StoredVariant, binding.span, &binding.value);
        StoredVariantDefinition {
            binding,
            key,
            call_span: self.source_span(call.span),
            variants,
        }
    }

    /// Reads the argument of a declaration as a variant sequence.
    ///
    /// Returns `None` after reporting a diagnostic when the argument is not a sequence.
    fn variant_sequence(&mut self, expression: &Expression<'a>) -> Option<Vec<StyleVariant>> {
        match expression.get_inner_expression() {
            Expression::StringLiteral(literal) => {
                let text = self.string_literal(literal);
                self.variant_text(text, literal.span)
            }
            Expression::TemplateLiteral(template) if template.expressions.is_empty() => {
                let quasi = template.quasis.first()?;
                let text = self.unicode_string(quasi.value.cooked?, template.span)?;
                let span = self.source_span(template.span);
                let content = Span::new(span.file, span.start.0 + 1, span.end.0.saturating_sub(1));
                self.variant_text(Spanned::new(text.to_lean_str(), content), template.span)
            }
            Expression::TemplateLiteral(template) => self.variant_template(template),
            Expression::CallExpression(_) | Expression::TaggedTemplateExpression(_) => {
                self.ref_variant(expression).map(|variant| vec![variant])
            }
            _ => match self.stored_variant_use(expression) {
                StoredVariantUse::Variants(variants) => Some(variants),
                StoredVariantUse::Omitted => None,
                StoredVariantUse::NotStored => self.stored_variant_error(
                    "the argument is not a variant string, a ref variant, or a stored variant",
                    expression.span(),
                ),
            },
        }
    }

    /// Converts static variant text into one prefix wrapper with a final `:`.
    fn variant_text(
        &mut self,
        mut text: Spanned<LeanStr>,
        span: OxcSpan,
    ) -> Option<Vec<StyleVariant>> {
        if text.value.is_empty() || text.value == ":" {
            return self.stored_variant_error("the argument contains no variants", span);
        }
        if !text.value.ends_with(':') {
            text.value = format_args!("{}:", text.value).to_lean_str();
        }
        Some(vec![StyleVariant::Prefix(text)])
    }

    /// Reads a template whose substitutions occupy complete variant segments, such as
    /// `` `${card("hover")}:md` ``.
    fn variant_template(&mut self, template: &TemplateLiteral<'a>) -> Option<Vec<StyleVariant>> {
        let mut token = TemplateToken::default();
        let segment_error = "a substitution must be a ref variant or a stored variant that \
                             occupies a complete variant segment";
        for (index, quasi) in template.quasis.iter().enumerate() {
            let text = quasi
                .value
                .cooked
                .and_then(|value| self.unicode_string(value, quasi.span))?;
            if text.chars().any(char::is_whitespace) {
                return self.stored_variant_error(
                    "a variant sequence cannot contain whitespace",
                    quasi.span,
                );
            }
            if token.text(text, self.source_span(quasi.span)).is_err() {
                return self.stored_variant_error(segment_error, quasi.span);
            }
            let Some(expression) = template.expressions.get(index) else {
                continue;
            };
            let variants = match self.template_substitution(expression) {
                StoredVariantUse::Variants(variants) => variants,
                StoredVariantUse::Omitted => {
                    self.probe_remaining_substitutions(template, index + 1);
                    return None;
                }
                StoredVariantUse::NotStored => {
                    self.probe_remaining_substitutions(template, index + 1);
                    return self.stored_variant_error(segment_error, expression.span());
                }
            };
            if token
                .reference(variants, self.source_span(quasi.span))
                .is_err()
            {
                return self.stored_variant_error(segment_error, quasi.span);
            }
        }
        match token.into_variants() {
            Some(variants) => Some(variants),
            None => self.stored_variant_error("the argument contains no variants", template.span),
        }
    }

    /// Reads a template substitution as a ref variant or a stored variant.
    ///
    /// A ref call or tag that cannot be lowered has already reported its diagnostic,
    /// so it returns [`StoredVariantUse::Omitted`].
    pub(crate) fn template_substitution(
        &mut self,
        expression: &Expression<'a>,
    ) -> StoredVariantUse {
        match expression.get_inner_expression() {
            Expression::CallExpression(_) | Expression::TaggedTemplateExpression(_) => {
                let lints = self.lint_findings.len();
                let errors = self.diagnostics.len();
                match self.ref_variant(expression) {
                    Some(variant) => StoredVariantUse::Variants(vec![variant]),
                    None if lints == self.lint_findings.len()
                        && errors == self.diagnostics.len() =>
                    {
                        StoredVariantUse::NotStored
                    }
                    None => StoredVariantUse::Omitted,
                }
            }
            _ => self.stored_variant_use(expression),
        }
    }

    /// Reads the substitutions of `template` from `index` on only to record their
    /// checker probes, and discards the findings of these reads.
    ///
    /// A template reader stops at its first unusable substitution. Before facts are
    /// known, an imported stored variant is unusable, so the substitutions after it
    /// would have no facts in the final extraction. Call this before omitting the
    /// template.
    pub(crate) fn probe_remaining_substitutions(
        &mut self,
        template: &TemplateLiteral<'a>,
        index: usize,
    ) {
        let lints = self.lint_findings.len();
        let errors = self.diagnostics.len();
        for expression in template.expressions.iter().skip(index) {
            self.template_substitution(expression);
        }
        self.lint_findings.truncate(lints);
        self.diagnostics.truncate(errors);
    }

    /// Reads `expression` as a reference to one `style.when()` declaration.
    ///
    /// Only identifiers and member expressions can refer to a declaration. A value
    /// that the extractor cannot resolve adds an optional checker probe and is not a
    /// stored variant. A value that can be one of several declarations adds
    /// [`LintDiagnostic::AmbiguousStoredVariant`].
    pub(crate) fn stored_variant_use(&mut self, expression: &Expression<'a>) -> StoredVariantUse {
        let expression = expression.get_inner_expression();
        if !matches!(
            expression,
            Expression::Identifier(_)
                | Expression::StaticMemberExpression(_)
                | Expression::ComputedMemberExpression(_)
        ) {
            return StoredVariantUse::NotStored;
        }
        self.probing_use.set(true);
        let candidates = self.asset_candidates(expression, AssetKind::StoredVariant);
        self.probing_use.set(false);
        let identity = match candidates.as_deref() {
            Ok([identity]) => identity.clone(),
            Ok(_) => {
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::AmbiguousStoredVariant,
                    self.source_span(expression.span()),
                ));
                return StoredVariantUse::Omitted;
            }
            Err(ResolutionFailure::Unsupported | ResolutionFailure::MissingFacts) => {
                return StoredVariantUse::NotStored;
            }
        };
        self.consumed_use_spans
            .insert(LocalSpan::from(expression.span()));
        if let Some(definition) = self.stored_variants.iter().find(|definition| {
            self.identity(
                AssetKind::StoredVariant,
                &definition.binding,
                &definition.key,
            ) == identity
        }) {
            return definition
                .variants
                .clone()
                .map_or(StoredVariantUse::Omitted, StoredVariantUse::Variants);
        }
        if identity.module_id() == self.module_id {
            self.stored_variant_error::<()>(
                "a stored variant must be declared before another declaration uses it",
                expression.span(),
            );
            return StoredVariantUse::Omitted;
        }
        let span = self.source_span(expression.span());
        match self
            .imported_assets
            .get(&identity)
            .map(|metadata| metadata.stored_variant.as_ref())
        {
            Some(Some(record)) => StoredVariantUse::Variants(self.expand_record(record, span)),
            Some(None) => StoredVariantUse::Omitted,
            None => StoredVariantUse::NotStored,
        }
    }

    /// Converts the variants of a local declaration into the record that other
    /// modules expand.
    pub(crate) fn stored_variant_record(variants: &[StyleVariant]) -> StoredVariantRecord {
        let refs = |refs: &[RefDefinition]| {
            refs.iter()
                .map(crate::refs::ref_identity)
                .collect::<Vec<_>>()
        };
        let condition = |condition: &RefCondition| StoredRefCondition {
            refs: refs(&condition.refs),
            prefix: condition.prefix.value.clone(),
        };
        StoredVariantRecord {
            segments: variants
                .iter()
                .map(|variant| match variant {
                    StyleVariant::Prefix(prefix) => {
                        StoredVariantSegment::Prefix(prefix.value.clone())
                    }
                    StyleVariant::Container {
                        refs: providers,
                        query,
                    } => StoredVariantSegment::Container {
                        refs: refs(providers),
                        query: query.value.clone(),
                    },
                    StyleVariant::Ref {
                        condition: subject,
                        scopes,
                    } => StoredVariantSegment::Ref {
                        condition: condition(subject),
                        scopes: scopes.iter().map(condition).collect(),
                    },
                    StyleVariant::Alternatives(alternatives) => StoredVariantSegment::Alternatives(
                        alternatives
                            .iter()
                            .map(|sequence| Self::stored_variant_record(sequence))
                            .collect(),
                    ),
                })
                .collect(),
        }
    }

    /// Expands the record of a declaration from another module. Every variant uses
    /// `span`, the expression at the use.
    fn expand_record(&self, record: &StoredVariantRecord, span: Span) -> Vec<StyleVariant> {
        let refs = |identities: &[IdentityId]| {
            identities
                .iter()
                .map(|identity| self.imported_ref(identity))
                .collect::<Vec<_>>()
        };
        let condition = |condition: &StoredRefCondition| RefCondition {
            refs: refs(&condition.refs),
            prefix: Spanned::new(condition.prefix.clone(), span),
        };
        record
            .segments
            .iter()
            .map(|segment| match segment {
                StoredVariantSegment::Prefix(prefix) => {
                    StyleVariant::Prefix(Spanned::new(prefix.clone(), span))
                }
                StoredVariantSegment::Container {
                    refs: providers,
                    query,
                } => StyleVariant::Container {
                    refs: refs(providers),
                    query: Spanned::new(query.clone(), span),
                },
                StoredVariantSegment::Ref {
                    condition: subject,
                    scopes,
                } => StyleVariant::Ref {
                    condition: condition(subject),
                    scopes: scopes.iter().map(condition).collect(),
                },
                StoredVariantSegment::Alternatives(alternatives) => StyleVariant::Alternatives(
                    alternatives
                        .iter()
                        .map(|sequence| self.expand_record(sequence, span))
                        .collect(),
                ),
            })
            .collect()
    }

    /// Adds [`LintDiagnostic::InvalidStoredVariantArgument`] at `span` and returns
    /// `None`.
    fn stored_variant_error<T>(&mut self, reason: &str, span: OxcSpan) -> Option<T> {
        self.lint_findings.push(LintFinding::new(
            LintDiagnostic::InvalidStoredVariantArgument {
                reason: reason.into(),
            },
            self.source_span(span),
        ));
        None
    }
}
