//! Collects proven normalization omissions and checker targets from the generated AST.

use lean_string::{LeanStr, ToLeanStr};
use oxc::ast::ast::{
    ArrayExpressionElement, ArrowFunctionBody, BindingPattern, CallExpression, Expression,
    NewExpression, ObjectExpression, ObjectProperty, ObjectPropertyKind, Program, PropertyKey,
    Statement, StaticMemberExpression, TSQualifiedName, TSType, TSTypeAliasDeclaration,
    TSTypeQueryExprName, VariableDeclarator,
};
use oxc::ast_visit::{Visit, walk};
use oxc::span::GetSpan;

use stillwind_ir::{CheckerBinding, FrameworkSpec, ProjectionDecision, ProjectionExpression};
use stillwind_source::{ProjectionIndex, SourceFile, Span};

/// Proven omissions, synthetic reads, and AST targets collected in traversal order.
pub(super) struct NormalizationPlan {
    pub(super) omitted: Vec<Span>,
    pub(super) ignored: Vec<Span>,
    pub(super) bindings: Vec<CheckerBinding>,
    pub(super) expressions: Vec<(Span, bool)>,
}

/// Visits generated syntax and returns only candidates accepted by the framework policy.
pub(super) fn collect_plan<'a>(
    source: &'a SourceFile,
    index: &ProjectionIndex<'a>,
    framework: &'static FrameworkSpec,
    program: &Program<'_>,
) -> NormalizationPlan {
    let mut visitor = NormalizationVisitor {
        source,
        index,
        framework,
        omitted: Vec::new(),
        ignored: Vec::new(),
        bindings: Vec::new(),
        expressions: Vec::new(),
        binding: "",
    };
    visitor.visit_program(program);
    NormalizationPlan {
        omitted: visitor.omitted,
        ignored: visitor.ignored,
        bindings: visitor.bindings,
        expressions: visitor.expressions,
    }
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
                name: reference.name.to_lean_str(),
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
                    name: callee.name.to_lean_str(),
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
                let value_name: LeanStr = fragments
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

pub(super) fn merge_omissions(mut spans: Vec<Span>) -> Vec<Span> {
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
