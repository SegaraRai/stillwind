//! A validated analysis view shared by macro detection and expression comparison.
//!
//! Converters (Vue, Svelte, host transformers) emit a shadow module whose text differs
//! from what the author wrote: type scaffolding, inserted reference lists, helper
//! consts. This module asks the framework adapter which of those constructs may be
//! discarded, removes only the ones the projection map says were never derived from
//! user text, and keeps an ordered segment list that maps every surviving byte back to
//! the generated source.
//!
//! Two invariants matter. The view is analysis-only: output expressions come from
//! validated original spans, never from reprinting this intermediate source. And a span
//! that does not correspond exactly to original text fails the whole conversion instead
//! of producing a plausible edit.

use oxc::allocator::Allocator;
use oxc::ast::ast::{
    ArrayExpressionElement, BindingPattern, Expression, ObjectProperty, PropertyKey, Statement,
    StaticMemberExpression, TSTypeAliasDeclaration,
};
use oxc::ast_visit::{Visit, walk};
use oxc::parser::Parser;
use oxc::span::GetSpan;

use stillwind_ir::{
    AdapterKind, FrameworkSpec, ModuleIr, ProjectionDecision, ProjectionExpression, ScriptLanguage,
};
use stillwind_source::{
    FileId, ProjectionError, ProjectionIndex, ProjectionMap, SourceFile, SourceProvenance, Span,
};

use crate::entry::source_type;

/// One retained run of text, in both coordinate systems.
struct Segment {
    /// Half-open UTF-8 span of the retained run in the analysis source.
    normalized: Span,
    /// Half-open UTF-8 span of the same text in the projected source; equal in byte
    /// length to normalized.
    projected: Span,
}

/// Normalization is only an analysis view. Output expressions come from validated
/// original spans, never from reprinting this intermediate source.
///
/// The struct owns the analysis `SourceFile` and borrows both real sources through the
/// projection index, so it must not outlive them.
pub struct NormalizedProjection<'a> {
    /// The spliced analysis source, tagged with the caller's `analysis_id`.
    pub source: SourceFile,
    /// Loaded map between the generated and original sources.
    index: ProjectionIndex<'a>,
    /// Retained text runs, in normalized source order.
    segments: Vec<Segment>,
}

impl NormalizedProjection<'_> {
    /// Maps a span of the analysis source back to the original source.
    ///
    /// Characters the conversion removed are skipped, so a span that crosses an omitted
    /// region is verified as several fragments; the returned provenance covers the same
    /// text in the original.
    ///
    /// # Errors
    ///
    /// Rejects spans outside the analysis view or without exact original correspondence.
    pub fn original_span(&self, span: Span) -> Result<SourceProvenance, ProjectionError> {
        self.source.slice(span)?;
        let mut fragments = Vec::new();
        for segment in &self.segments {
            let start = span.start.0.max(segment.normalized.start.0);
            let end = span.end.0.min(segment.normalized.end.0);
            if start < end {
                let shift = segment.projected.start.0 - segment.normalized.start.0;
                fragments.push(Span::new(
                    segment.projected.file,
                    start + shift,
                    end + shift,
                ));
            }
        }
        self.index.verify_fragments(&fragments)
    }

    /// Rewrites every IR span to the original source, returning the provenance of each
    /// span it mapped.
    ///
    /// The whole IR is mapped or the call fails; a partially mapped IR is never
    /// returned, because a stale span would produce an edit in the wrong place.
    ///
    /// # Errors
    ///
    /// Fails atomically if any expression or declaration cannot be mapped.
    pub fn original_ir(
        &self,
        ir: &ModuleIr,
    ) -> Result<(ModuleIr, Vec<SourceProvenance>), ProjectionError> {
        let mut provenance = Vec::new();
        // Converter-only reads (for example generated type queries) may have no
        // original coordinate. Keep that uncertainty rather than inventing a location
        // or allowing an unsafe import deletion.
        let mut editable = ir.clone();
        for import in &mut editable.imports.macros {
            for specifier in &mut import.specifiers {
                specifier.references.clear();
            }
        }
        let mut original = editable.try_map_spans(|span| {
            let mapped = self.original_span(span)?;
            provenance.push(mapped);
            Ok::<_, ProjectionError>(mapped.original)
        })?;
        for (target, source) in original.imports.macros.iter_mut().zip(&ir.imports.macros) {
            for (target, source) in target.specifiers.iter_mut().zip(&source.specifiers) {
                for reference in &source.references {
                    if let Ok(mapped) = self.original_span(*reference) {
                        target.references.push(mapped.original);
                        provenance.push(mapped);
                    } else {
                        target.has_unmapped_references = true;
                    }
                }
            }
        }
        Ok((original, provenance))
    }
}

/// Creates a parser-neutral normalization plan, validates each omitted prefix against
/// converter mappings, then builds an Oxc analysis source from that plan.
///
/// `analysis_id` becomes the file id of the produced source and must differ from both
/// inputs, so a span can never be attributed to the wrong file by accident.
///
/// # Errors
///
/// Rejects malformed maps, invalid syntax, overlapping plans or source identity collisions.
pub fn normalize_projection<'a>(
    generated: &'a SourceFile,
    original: &'a SourceFile,
    map: &ProjectionMap,
    framework: AdapterKind,
    analysis_id: FileId,
) -> Result<NormalizedProjection<'a>, ProjectionError> {
    normalize_with_language(
        generated,
        original,
        map,
        framework,
        analysis_id,
        ScriptLanguage::from_path(generated.path()),
    )
}

/// Language-explicit form of [`normalize_projection`].
///
/// The language decides how the generated text is parsed, which matters when the
/// generated path does not carry the script language.
pub(crate) fn normalize_with_language<'a>(
    generated: &'a SourceFile,
    original: &'a SourceFile,
    map: &ProjectionMap,
    framework: AdapterKind,
    analysis_id: FileId,
    language: ScriptLanguage,
) -> Result<NormalizedProjection<'a>, ProjectionError> {
    if generated.id() == original.id()
        || analysis_id == generated.id()
        || analysis_id == original.id()
    {
        return Err(ProjectionError::Map(
            "projection sources require distinct file identities".into(),
        ));
    }
    let index = ProjectionIndex::new(generated, original, map)?;
    let allocator = Allocator::default();
    let parsed = Parser::new(
        &allocator,
        generated.text(),
        source_type(generated.path(), language),
    )
    .parse();
    if !parsed.diagnostics.is_empty() {
        return Err(ProjectionError::Map(format!(
            "invalid generated JavaScript: {}",
            parsed.diagnostics[0]
        )));
    }
    let mut visitor = NormalizationVisitor {
        source: generated,
        index: &index,
        framework: framework.spec(),
        omitted: Vec::new(),
    };
    visitor.visit_program(&parsed.program);
    let mut omitted = visitor.omitted;
    omitted.sort_by_key(|span| (span.start.0, std::cmp::Reverse(span.end.0)));
    omitted.dedup();
    let mut non_nested: Vec<Span> = Vec::new();
    for omission in omitted {
        if non_nested.last().is_some_and(|previous| {
            previous.start.0 <= omission.start.0 && previous.end.0 >= omission.end.0
        }) {
            continue;
        }
        non_nested.push(omission);
    }
    let mut output = String::with_capacity(generated.text().len());
    let mut cursor = 0;
    let mut segments = Vec::new();
    for omission in non_nested.into_iter().chain(std::iter::once(Span::new(
        generated.id(),
        generated.full_span().end.0,
        generated.full_span().end.0,
    ))) {
        if omission.start.0 < cursor {
            return Err(ProjectionError::Map(
                "overlapping normalization candidates".into(),
            ));
        }
        let start = u32::try_from(output.len())
            .map_err(|_| ProjectionError::Map("normalized source too large".into()))?;
        output.push_str(generated.slice(Span::new(generated.id(), cursor, omission.start.0))?);
        let end = u32::try_from(output.len())
            .map_err(|_| ProjectionError::Map("normalized source too large".into()))?;
        if start != end {
            segments.push(Segment {
                normalized: Span::new(analysis_id, start, end),
                projected: Span::new(generated.id(), cursor, omission.start.0),
            });
        }
        cursor = omission.end.0;
    }
    Ok(NormalizedProjection {
        source: SourceFile::new(analysis_id, generated.path(), output)?,
        index,
        segments,
    })
}

/// Visitor that asks the framework adapter which scaffolding to omit.
///
/// It only ever records spans it could prove the converter inserted; a construct the
/// map claims came from user text is never omitted.
struct NormalizationVisitor<'a, 'b> {
    /// Generated source being visited.
    source: &'a SourceFile,
    /// Loaded projection map, used to test whether a header was derived.
    index: &'b ProjectionIndex<'a>,
    /// Framework rules deciding what counts as scaffolding.
    framework: &'static FrameworkSpec,
    /// Candidate omission spans, in visit order.
    omitted: Vec<Span>,
}

impl<'a> Visit<'a> for NormalizationVisitor<'_, '_> {
    /// Drops a synthetic type alias whose header no user text produced.
    fn visit_ts_type_alias_declaration(&mut self, declaration: &TSTypeAliasDeclaration<'a>) {
        let definition = declaration.type_annotation.span();
        let whole = Span::new(
            self.source.id(),
            declaration.span.start,
            declaration.span.end,
        );
        let header = Span::new(self.source.id(), declaration.span.start, definition.start);
        let decision = (self.framework.normalize)(ProjectionExpression::SyntheticTypeAlias {
            name: declaration.id.name.as_str(),
            definition: self
                .source
                .slice(Span::new(
                    self.source.id(),
                    definition.start,
                    definition.end,
                ))
                .unwrap_or_default(),
        });
        if matches!(decision, ProjectionDecision::DiscardScaffolding)
            && self.index.is_unmapped_syntax(header)
        {
            self.omitted.push(whole);
        } else {
            walk::walk_ts_type_alias_declaration(self, declaration);
        }
    }

    /// Drops an inserted reference list spread over two statements.
    ///
    /// Both the array header and the following binding must be unmapped, so a
    /// user-written `[a, b]` array followed by a variable is left alone.
    fn visit_statements(&mut self, statements: &oxc::allocator::Vec<'a, Statement<'a>>) {
        for (index, statement) in statements.iter().enumerate() {
            if let Statement::ExpressionStatement(expression) = statement
                && let Expression::ArrayExpression(array) = &expression.expression
                && !array.elements.is_empty()
                && array
                    .elements
                    .iter()
                    .all(|element| matches!(element, ArrayExpressionElement::Identifier(_)))
                && let Some(Statement::VariableDeclaration(next)) = statements.get(index + 1)
                && let Some(binding) = next.declarations.first()
                && let BindingPattern::BindingIdentifier(identifier) = &binding.id
            {
                let prefix = &self.source.text()[..expression.span.start as usize];
                let preceding_line = prefix
                    .trim_end()
                    .lines()
                    .next_back()
                    .unwrap_or_default()
                    .trim();
                let decision =
                    (self.framework.normalize)(ProjectionExpression::SyntheticReferenceList {
                        preceding_line,
                        next_binding: identifier.name.as_str(),
                    });
                let header = Span::new(
                    self.source.id(),
                    array.span.start,
                    array.elements[0].span().start,
                );
                if matches!(decision, ProjectionDecision::DiscardScaffolding)
                    && self.index.is_unmapped_syntax(header)
                    && self.index.is_unmapped_syntax(Span::new(
                        self.source.id(),
                        identifier.span.start,
                        identifier.span.end,
                    ))
                {
                    self.omitted.push(Span::new(
                        self.source.id(),
                        expression.span.start,
                        expression.span.end,
                    ));
                    continue;
                }
            }
            self.visit_statement(statement);
        }
    }

    /// Omits a collapsed receiver prefix when the adapter keeps the property.
    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        if let Expression::Identifier(receiver) = &member.object {
            let whole = Span::new(self.source.id(), member.span.start, member.span.end);
            let property = Span::new(
                self.source.id(),
                member.property.span.start,
                member.property.span.end,
            );
            let decision = (self.framework.normalize)(ProjectionExpression::Member {
                receiver: receiver.name.as_str(),
                whole,
                property,
                optional: member.optional,
            });
            if let ProjectionDecision::Retain(retained) = decision
                && self.index.is_collapsed_prefix(whole, retained)
            {
                self.omitted
                    .push(Span::new(whole.file, whole.start.0, retained.start.0));
            }
        }
        walk::walk_static_member_expression(self, member);
    }

    /// Omits the parts of an expanded shorthand property that were not retained.
    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        if !property.computed
            && !property.method
            && let PropertyKey::StaticIdentifier(key) = &property.key
            && let Expression::StaticMemberExpression(value) = &property.value
            && !value.optional
            && let Expression::Identifier(receiver) = &value.object
        {
            let whole = Span::new(self.source.id(), property.span.start, property.span.end);
            let key_span = Span::new(self.source.id(), key.span.start, key.span.end);
            let decision = (self.framework.normalize)(ProjectionExpression::ExpandedShorthand {
                receiver: receiver.name.as_str(),
                whole,
                key: key_span,
                key_name: key.name.as_str(),
                value_name: value.property.name.as_str(),
            });
            if let ProjectionDecision::Retain(retained) = decision
                && self.index.is_collapsed_wrapper(whole, retained)
            {
                if whole.start.0 < retained.start.0 {
                    self.omitted
                        .push(Span::new(whole.file, whole.start.0, retained.start.0));
                }
                if retained.end.0 < whole.end.0 {
                    self.omitted
                        .push(Span::new(whole.file, retained.end.0, whole.end.0));
                }
            }
        }
        walk::walk_object_property(self, property);
    }
}
