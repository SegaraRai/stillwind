//! Validated correspondence between authored expressions and converter code.
//!
//! Framework policy selects candidates. Mapping validation produces one analysis
//! view and AST-specific checker targets. Generated declarations keep their scopes;
//! only proven synthetic reads are excluded from macro import accounting.

use std::collections::BTreeMap;

use oxc::allocator::Allocator;
use oxc::ast::ast::{
    ArrayExpressionElement, ArrowFunctionBody, BindingPattern, CallExpression, Expression,
    NewExpression, ObjectExpression, ObjectProperty, ObjectPropertyKind, PropertyKey, Statement,
    StaticMemberExpression, TSQualifiedName, TSType, TSTypeAliasDeclaration, TSTypeQueryExprName,
    VariableDeclarator,
};
use oxc::ast_visit::{Visit, walk};
use oxc::parser::Parser;
use oxc::span::GetSpan;

use stillwind_ir::{
    AdapterKind, CheckerBinding, CheckerRole, CheckerSpan, FrameworkSpec, ModuleIr,
    ProjectionDecision, ProjectionExpression, ScriptLanguage, ScriptModuleKind,
};
use stillwind_source::{
    FileId, ProjectionError, ProjectionIndex, ProjectionMap, SourceFile, SourceProvenance, Span,
};

use crate::entry::source_type;

/// A retained run with equal byte lengths in both buffers.
struct Segment {
    normalized: Span,
    projected: Span,
}

/// An analysis view and the validated generated expressions that it represents.
///
/// Output edits use original spans. This view borrows the projection sources and
/// owns the analysis source tagged with the caller's distinct file identity.
pub struct NormalizedProjection<'a> {
    /// Source for macro extraction and expression comparison.
    pub source: SourceFile,
    index: ProjectionIndex<'a>,
    segments: Vec<Segment>,
    expressions: BTreeMap<(u32, u32), Span>,
    producers: BTreeMap<(u32, u32), Span>,
    ignored: Vec<Span>,
    bindings: Vec<CheckerBinding>,
}

impl NormalizedProjection<'_> {
    fn fragments(&self, span: Span) -> Vec<Span> {
        self.segments
            .iter()
            .filter_map(|segment| {
                let start = span.start.0.max(segment.normalized.start.0);
                let end = span.end.0.min(segment.normalized.end.0);
                if start < end
                    || (span.start == span.end
                        && segment.normalized.start <= span.start
                        && span.end <= segment.normalized.end)
                {
                    let shift = segment.projected.start.0 - segment.normalized.start.0;
                    Some(Span::new(
                        segment.projected.file,
                        start + shift,
                        end + shift,
                    ))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Maps retained analysis text to the exact authored text.
    ///
    /// # Errors
    ///
    /// Rejects invalid analysis spans and ambiguous or nonmatching correspondence.
    pub fn original_span(&self, span: Span) -> Result<SourceProvenance, ProjectionError> {
        self.source.slice(span)?;
        self.index.verify_fragments(&self.fragments(span))
    }

    /// Returns the generated AST node for a checker operation on an analysis span.
    ///
    /// # Errors
    ///
    /// Rejects spans without exact original correspondence or the required AST role.
    pub fn checker_span(
        &self,
        span: Span,
        role: CheckerRole,
    ) -> Result<CheckerSpan, ProjectionError> {
        let mut provenance = self.original_span(span)?;
        provenance.projected = match role {
            CheckerRole::Expression => self.expressions.get(&(span.start.0, span.end.0)).copied(),
            CheckerRole::Producer => self.producers.get(&(span.start.0, span.end.0)).copied(),
            CheckerRole::ModuleSpecifier => self
                .index
                .verify_exact(provenance.projected)
                .ok()
                .map(|mapped| mapped.projected),
        }
        .ok_or(ProjectionError::Unmapped(span))?;
        Ok(CheckerSpan { provenance, role })
    }

    /// Generated references whose bindings the checker must protect from author scopes.
    pub fn checker_bindings(&self) -> &[CheckerBinding] {
        &self.bindings
    }

    fn record_expressions(&mut self, expressions: Vec<(Span, bool)>) {
        for (generated_span, call) in expressions {
            let retained: Vec<_> = self
                .segments
                .iter()
                .filter_map(|segment| {
                    let start = generated_span.start.0.max(segment.projected.start.0);
                    let end = generated_span.end.0.min(segment.projected.end.0);
                    (start < end).then(|| {
                        (
                            start - segment.projected.start.0 + segment.normalized.start.0,
                            end - segment.projected.start.0 + segment.normalized.start.0,
                        )
                    })
                })
                .collect();
            if let Some((first, last)) = retained.first().zip(retained.last()) {
                let key = (first.0, last.1);
                let span = Span::new(self.source.id(), key.0, key.1);
                if self.original_span(span).is_ok() {
                    // Parents are visited after children, so a validated wrapper wins
                    // over its retained argument when both represent the same text.
                    self.expressions.insert(key, generated_span);
                    if call {
                        self.producers.insert(key, generated_span);
                    }
                }
            }
        }
    }

    fn ignored_reference(&self, span: Span) -> bool {
        let fragments = self.fragments(span);
        fragments
            .first()
            .zip(fragments.last())
            .is_some_and(|(first, last)| {
                self.ignored
                    .iter()
                    .any(|ignored| ignored.start <= first.start && last.end <= ignored.end)
            })
    }

    /// Maps IR atomically and excludes validated synthetic import reads.
    ///
    /// Unmapped reads outside synthetic regions remain import-deletion guards.
    ///
    /// # Errors
    ///
    /// Rejects any expression or declaration without exact original correspondence.
    pub fn original_ir(
        &self,
        ir: &ModuleIr,
    ) -> Result<(ModuleIr, Vec<SourceProvenance>), ProjectionError> {
        let mut provenance = Vec::new();
        let mut editable = ir.clone();
        for import in &mut editable.imports.macros {
            for specifier in &mut import.specifiers {
                specifier.references.clear();
            }
        }
        let mut original = editable.try_map_spans(|span| {
            if span.file != self.source.id() {
                return Ok(span);
            }
            let mapped = self.original_span(span)?;
            provenance.push(mapped);
            Ok::<_, ProjectionError>(mapped.original)
        })?;
        for (target, source) in original.imports.macros.iter_mut().zip(&ir.imports.macros) {
            for (target, source) in target.specifiers.iter_mut().zip(&source.specifiers) {
                for reference in &source.references {
                    if self.ignored_reference(*reference) {
                        continue;
                    }
                    if let Ok(mapped) = self.original_span(*reference) {
                        target.references.push(mapped.original);
                        provenance.push(mapped);
                    } else {
                        target.has_unmapped_references = true;
                    }
                }
                target.references.sort_by_key(|span| (span.start, span.end));
                target.references.dedup();
            }
        }
        Ok((original, provenance))
    }
}

/// Creates a validated analysis view and generated checker correspondences.
///
/// `analysis_id` must differ from both input file identities. The language describes
/// the generated buffer, independently of its file extension.
///
/// # Errors
///
/// Rejects malformed maps or generated syntax and overlapping normalization plans.
pub fn normalize_projection<'a>(
    generated: &'a SourceFile,
    original: &'a SourceFile,
    map: &ProjectionMap,
    framework: AdapterKind,
    analysis_id: FileId,
    language: ScriptLanguage,
    module_kind: ScriptModuleKind,
) -> Result<NormalizedProjection<'a>, ProjectionError> {
    normalize_with_language(
        generated,
        original,
        map,
        framework,
        analysis_id,
        language,
        module_kind,
    )
}

pub(crate) fn normalize_with_language<'a>(
    generated: &'a SourceFile,
    original: &'a SourceFile,
    map: &ProjectionMap,
    framework: AdapterKind,
    analysis_id: FileId,
    language: ScriptLanguage,
    module_kind: ScriptModuleKind,
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
        source_type(language, module_kind),
    )
    .parse();
    if let Some(diagnostic) = parsed.diagnostics.first() {
        return Err(ProjectionError::Map(format!(
            "invalid generated JavaScript: {diagnostic}"
        )));
    }
    let mut visitor = NormalizationVisitor {
        source: generated,
        index: &index,
        framework: framework.spec(),
        omitted: Vec::new(),
        ignored: Vec::new(),
        bindings: Vec::new(),
        expressions: Vec::new(),
        binding: "",
    };
    visitor.visit_program(&parsed.program);
    let NormalizationVisitor {
        omitted,
        ignored,
        bindings,
        expressions,
        ..
    } = visitor;
    let omitted = merge_omissions(omitted);
    let mut output = String::with_capacity(generated.text().len());
    let mut cursor = 0;
    let mut segments = Vec::new();
    for omission in omitted.into_iter().chain(std::iter::once(Span::new(
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
    let mut normalized = NormalizedProjection {
        source: SourceFile::new(analysis_id, None, None, output)?,
        index,
        segments,
        expressions: BTreeMap::new(),
        producers: BTreeMap::new(),
        ignored,
        bindings,
    };
    normalized.record_expressions(expressions);
    Ok(normalized)
}

fn merge_omissions(mut spans: Vec<Span>) -> Vec<Span> {
    spans.sort_by_key(|span| (span.start, span.end));
    let mut merged: Vec<Span> = Vec::new();
    for span in spans {
        if let Some(previous) = merged.last_mut()
            && span.start <= previous.end
        {
            previous.end = previous.end.max(span.end);
        } else {
            merged.push(span);
        }
    }
    merged
}

struct NormalizationVisitor<'a, 'b> {
    source: &'a SourceFile,
    index: &'b ProjectionIndex<'a>,
    framework: &'static FrameworkSpec,
    omitted: Vec<Span>,
    ignored: Vec<Span>,
    bindings: Vec<CheckerBinding>,
    expressions: Vec<(Span, bool)>,
    binding: &'b str,
}

impl NormalizationVisitor<'_, '_> {
    fn span(&self, span: oxc::span::Span) -> Span {
        Span::new(self.source.id(), span.start, span.end)
    }

    fn text(&self, span: oxc::span::Span) -> &str {
        self.source
            .text()
            .get(span.start as usize..span.end as usize)
            .unwrap_or_default()
    }

    fn fragments(&self, span: Span) -> Vec<Span> {
        let mut cursor = span.start.0;
        let mut fragments = Vec::new();
        for omitted in merge_omissions(self.omitted.clone()) {
            let start = omitted.start.0.max(span.start.0);
            let end = omitted.end.0.min(span.end.0);
            if start < end {
                if cursor < start {
                    fragments.push(Span::new(span.file, cursor, start));
                }
                cursor = cursor.max(end);
            }
        }
        if cursor < span.end.0 {
            fragments.push(Span::new(span.file, cursor, span.end.0));
        }
        fragments
    }

    fn keep(&mut self, decision: ProjectionDecision) -> bool {
        let ProjectionDecision::Keep {
            whole,
            retained,
            checker,
        } = decision
        else {
            return false;
        };
        if self.index.verify_fragments(&self.fragments(whole)).is_ok()
            || self
                .index
                .verify_wrapper(whole, &self.fragments(retained))
                .is_err()
        {
            return false;
        }
        if whole.start < retained.start {
            self.omitted
                .push(Span::new(whole.file, whole.start.0, retained.start.0));
        }
        if retained.end < whole.end {
            self.omitted
                .push(Span::new(whole.file, retained.end.0, whole.end.0));
        }
        if let Some(checker) = checker {
            self.expressions.push((checker, false));
        }
        true
    }

    fn ignore_return(
        &mut self,
        property: &ObjectProperty<'_>,
        key: &str,
        object: &ObjectExpression<'_>,
    ) {
        let mirrored_bindings = object.properties.iter().all(|property| {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                return false;
            };
            if property.computed || property.method {
                return false;
            }
            let PropertyKey::StaticIdentifier(key) = &property.key else {
                return false;
            };
            let Expression::TSAsExpression(value) = &property.value else {
                return false;
            };
            let Expression::Identifier(binding) = &value.expression else {
                return false;
            };
            let TSType::TSTypeQuery(query) = &value.type_annotation else {
                return false;
            };
            let TSTypeQueryExprName::IdentifierReference(reference) = &query.expr_name else {
                return false;
            };
            key.name == binding.name && binding.name == reference.name
        });
        let decision = (self.framework.normalize)(ProjectionExpression::SyntheticReturn {
            binding: self.binding,
            method: key,
            mirrored_bindings,
        });
        if matches!(decision, ProjectionDecision::IgnoreReferences)
            && self.index.is_unmapped_syntax(self.span(property.span))
        {
            self.ignored.push(self.span(object.span));
        }
    }
}

impl<'a> Visit<'a> for NormalizationVisitor<'_, 'a> {
    fn visit_expression(&mut self, expression: &Expression<'a>) {
        walk::walk_expression(self, expression);
        if let Expression::Identifier(reference) = expression
            && matches!(
                (self.framework.normalize)(ProjectionExpression::Reference {
                    name: reference.name.as_str(),
                }),
                ProjectionDecision::BindGlobalHelper
            )
            && self.index.is_unmapped_syntax(self.span(reference.span))
        {
            self.bindings.push(CheckerBinding {
                name: reference.name.to_string(),
                span: self.span(reference.span),
            });
        }
        self.expressions.push((
            self.span(expression.span()),
            matches!(expression, Expression::CallExpression(_)),
        ));
    }

    fn visit_ts_type_alias_declaration(&mut self, declaration: &TSTypeAliasDeclaration<'a>) {
        let definition = declaration.type_annotation.span();
        let decision = (self.framework.normalize)(ProjectionExpression::SyntheticTypeAlias {
            name: declaration.id.name.as_str(),
            definition: self.text(definition),
        });
        if matches!(decision, ProjectionDecision::IgnoreReferences)
            && self.index.is_unmapped_syntax(Span::new(
                self.source.id(),
                declaration.span.start,
                definition.start,
            ))
        {
            self.ignored.push(self.span(declaration.span));
        }
        walk::walk_ts_type_alias_declaration(self, declaration);
    }

    fn visit_statements(&mut self, statements: &oxc::allocator::Vec<'a, Statement<'a>>) {
        for statement in statements {
            if let Statement::ExpressionStatement(expression) = statement
                && let Expression::ArrayExpression(array) = &expression.expression
                && !array.elements.is_empty()
                && array
                    .elements
                    .iter()
                    .all(|element| matches!(element, ArrayExpressionElement::Identifier(_)))
            {
                let preceding_line = self.source.text()[..expression.span.start as usize]
                    .trim_end()
                    .lines()
                    .next_back()
                    .unwrap_or_default()
                    .trim();
                let decision =
                    (self.framework.normalize)(ProjectionExpression::SyntheticReferenceList {
                        preceding_line,
                    });
                if matches!(decision, ProjectionDecision::IgnoreReferences)
                    && self.index.is_unmapped_syntax(Span::new(
                        self.source.id(),
                        array.span.start,
                        array.elements[0].span().start,
                    ))
                    && self.index.is_unmapped_syntax(Span::new(
                        self.source.id(),
                        array
                            .elements
                            .last()
                            .map_or(array.span.end, |element| element.span().end),
                        array.span.end,
                    ))
                {
                    self.ignored.push(self.span(array.span));
                }
            }
            self.visit_statement(statement);
        }
    }

    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        walk::walk_static_member_expression(self, member);
        self.keep((self.framework.normalize)(ProjectionExpression::Member {
            receiver: self.text(member.object.span()),
            object: self.span(member.object.span()),
            whole: self.span(member.span),
            property: self.span(member.property.span),
            property_name: member.property.name.as_str(),
            optional: member.optional,
            runtime: true,
        }));
    }

    fn visit_ts_qualified_name(&mut self, name: &TSQualifiedName<'a>) {
        walk::walk_ts_qualified_name(self, name);
        self.keep((self.framework.normalize)(ProjectionExpression::Member {
            receiver: self.text(name.left.span()),
            object: self.span(name.left.span()),
            whole: self.span(name.span),
            property: self.span(name.right.span),
            property_name: name.right.name.as_str(),
            optional: false,
            runtime: false,
        }));
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        walk::walk_call_expression(self, call);
        if !call.optional
            && call.arguments.len() == 2
            && let Expression::Identifier(callee) = &call.callee
        {
            let value = call.arguments[0].span();
            let decision = (self.framework.normalize)(ProjectionExpression::Call {
                callee: callee.name.as_str(),
                whole: self.span(call.span),
                value: self.span(value),
                witness: self.text(oxc::span::Span::new(value.end, call.span.end)),
            });
            let synthetic = self.index.is_unmapped_syntax(self.span(callee.span));
            let ignored = matches!(decision, ProjectionDecision::IgnoreReferences)
                && synthetic
                && self.index.is_unmapped_syntax(Span::new(
                    self.source.id(),
                    value.end,
                    call.span.end,
                ));
            if ignored {
                self.ignored.push(self.span(call.span));
            }
            if ignored || self.keep(decision) {
                self.bindings.push(CheckerBinding {
                    name: callee.name.to_string(),
                    span: self.span(callee.span),
                });
            }
        }
    }

    fn visit_new_expression(&mut self, expression: &NewExpression<'a>) {
        walk::walk_new_expression(self, expression);
        if let Expression::ParenthesizedExpression(parentheses) = &expression.callee {
            self.keep((self.framework.normalize)(
                ProjectionExpression::ConstructorOperand {
                    whole: self.span(parentheses.span),
                    operand: self.span(parentheses.expression.span()),
                },
            ));
        }
    }

    fn visit_variable_declarator(&mut self, declaration: &VariableDeclarator<'a>) {
        let previous = self.binding;
        self.binding = if let BindingPattern::BindingIdentifier(identifier) = &declaration.id {
            identifier.name.as_str()
        } else {
            ""
        };
        walk::walk_variable_declarator(self, declaration);
        self.binding = previous;
    }

    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        walk::walk_object_property(self, property);
        if property.computed {
            return;
        }
        let PropertyKey::StaticIdentifier(key) = &property.key else {
            return;
        };
        if let Expression::ArrowFunctionExpression(function) = &property.value {
            if let ArrowFunctionBody::ParenthesizedExpression(parentheses) = &function.body
                && let Expression::ObjectExpression(object) = &parentheses.expression
            {
                self.ignore_return(property, key.name.as_str(), object);
            }
        } else if property.method
            && let Expression::FunctionExpression(function) = &property.value
        {
            if let Some(body) = &function.body
                && body.statements.len() == 1
                && let Statement::ReturnStatement(returned) = &body.statements[0]
                && let Some(Expression::ObjectExpression(object)) = &returned.argument
            {
                self.ignore_return(property, key.name.as_str(), object);
            }
        } else if !property.method && !property.shorthand {
            let fragments = self.fragments(self.span(property.value.span()));
            if self.index.verify_fragments(&fragments).is_ok() {
                let value_name: String = fragments
                    .iter()
                    .filter_map(|span| self.source.slice(*span).ok())
                    .collect();
                self.keep((self.framework.normalize)(
                    ProjectionExpression::ExpandedShorthand {
                        whole: self.span(property.span),
                        key: self.span(key.span),
                        value: self.span(property.value.span()),
                        key_name: key.name.as_str(),
                        value_name: &value_name,
                    },
                ));
            }
        }
    }
}
