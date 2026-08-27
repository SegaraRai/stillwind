//! Oxc-backed JS/TS/JSX parsing and the first import-bound `style()` extractor.

use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use oxc::{
    allocator::Allocator,
    ast::ast::{
        Argument, ArrayExpressionElement, BindingPattern, CallExpression, Expression,
        ImportDeclaration, ImportDeclarationSpecifier, ModuleExportName, ObjectExpression,
        ObjectPropertyKind, PropertyKey, PropertyKind, StringLiteral, VariableDeclarator,
    },
    ast_visit::{Visit, walk},
    parser::Parser,
    semantic::{Scoping, SemanticBuilder, SymbolId},
    span::{GetSpan, SourceType},
    syntax::operator::LogicalOperator,
};
use shorth_ir::{
    AnimationDefinition, AnimationFrame, AttributeEntry, HandleProjection, ModuleIr, StyleCall,
    StyleInput, UnsafeDeclaration, ViewTransitionDefinition,
};
use shorth_source::{Diagnostic, SourceFile, Span, Spanned};

#[derive(Debug)]
pub struct ParsedJavaScript {
    pub diagnostics: Vec<Diagnostic>,
    pub module: ModuleIr,
}

pub fn parse(source: &SourceFile) -> ParsedJavaScript {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(Path::new(source.path())).unwrap_or_default();
    let result = Parser::new(&allocator, source.text(), source_type).parse();

    let mut diagnostics: Vec<_> = result
        .diagnostics
        .into_iter()
        .map(|diagnostic| Diagnostic::error("SHJS001", diagnostic.to_string(), source.full_span()))
        .collect();

    let semantic = SemanticBuilder::new().build(&result.program).semantic;
    let mut extractor = MacroExtractor::new(source, semantic.scoping());
    extractor.visit_program(&result.program);
    extractor.validate_handles();
    diagnostics.extend(extractor.diagnostics);

    ParsedJavaScript {
        diagnostics,
        module: ModuleIr {
            adapter: shorth_ir::AdapterKind::React,
            style_calls: extractor.style_calls,
            handle_declarations: extractor.handle_declarations,
            animation_declarations: extractor.animation_declarations,
            view_transition_declarations: extractor.view_transition_declarations,
        },
    }
}

struct MacroExtractor<'a> {
    source: &'a SourceFile,
    scoping: &'a Scoping,
    style_symbols: HashSet<SymbolId>,
    handle_symbols: HashMap<SymbolId, Spanned<String>>,
    handle_uses: Vec<(String, &'static str, Span)>,
    handle_declarations: Vec<Span>,
    animation_symbols: HashMap<SymbolId, Spanned<String>>,
    animation_declarations: Vec<AnimationDefinition>,
    view_transition_symbols: HashMap<SymbolId, Spanned<String>>,
    view_transition_declarations: Vec<ViewTransitionDefinition>,
    style_calls: Vec<StyleCall>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> MacroExtractor<'a> {
    fn new(source: &'a SourceFile, scoping: &'a Scoping) -> Self {
        Self {
            source,
            scoping,
            style_symbols: HashSet::new(),
            handle_symbols: HashMap::new(),
            handle_uses: Vec::new(),
            handle_declarations: Vec::new(),
            animation_symbols: HashMap::new(),
            animation_declarations: Vec::new(),
            view_transition_symbols: HashMap::new(),
            view_transition_declarations: Vec::new(),
            style_calls: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn source_span(&self, span: oxc::span::Span) -> Span {
        Span::new(self.source.id(), span.start, span.end)
    }

    fn is_style_import(declaration: &ImportDeclaration<'a>) -> bool {
        declaration.source.value.as_str() == "stillwind/react"
    }

    fn is_style_member(&self, expression: &Expression<'a>, property: &str) -> bool {
        let Expression::StaticMemberExpression(member) = expression else {
            return false;
        };
        let Expression::Identifier(identifier) = &member.object else {
            return false;
        };
        let Some(reference_id) = identifier.reference_id.get() else {
            return false;
        };
        self.scoping
            .get_reference(reference_id)
            .symbol_id()
            .is_some_and(|symbol| self.style_symbols.contains(&symbol))
            && member.property.name.as_str() == property
    }

    fn is_style_handle_call(&self, call: &CallExpression<'a>) -> bool {
        self.is_style_member(&call.callee, "handle") && call.arguments.is_empty()
    }

    fn is_style_animation_call(&self, call: &CallExpression<'a>) -> bool {
        self.is_style_member(&call.callee, "animation")
    }

    fn is_style_view_transition_call(&self, call: &CallExpression<'a>) -> bool {
        self.is_style_member(&call.callee, "viewTransition")
    }

    fn handle_reference(&self, expression: &Expression<'a>) -> Option<Spanned<String>> {
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        let reference_id = identifier.reference_id.get()?;
        let symbol_id = self.scoping.get_reference(reference_id).symbol_id()?;
        self.handle_symbols.get(&symbol_id).cloned()
    }

    fn animation_reference(&self, expression: &Expression<'a>) -> Option<Spanned<String>> {
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        let reference_id = identifier.reference_id.get()?;
        let symbol_id = self.scoping.get_reference(reference_id).symbol_id()?;
        self.animation_symbols.get(&symbol_id).cloned()
    }

    fn view_transition_reference(&self, expression: &Expression<'a>) -> Option<Spanned<String>> {
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        let reference_id = identifier.reference_id.get()?;
        let symbol_id = self.scoping.get_reference(reference_id).symbol_id()?;
        self.view_transition_symbols.get(&symbol_id).cloned()
    }

    fn view_transition_use(&mut self, expression: &Expression<'a>) -> Option<StyleInput> {
        if let Some(transition) = self.view_transition_reference(expression) {
            return Some(StyleInput::ViewTransitionUse {
                transition,
                key: None,
            });
        }
        let Expression::CallExpression(call) = expression else {
            return None;
        };
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return None;
        };
        if member.property.name.as_str() != "for" {
            return None;
        }
        let transition = self.view_transition_reference(&member.object)?;
        if call.arguments.len() != 1 {
            self.diagnostics.push(Diagnostic::error(
                "SHVIEW002",
                "viewTransition.for() requires one stable key expression",
                self.source_span(call.span),
            ));
            return None;
        }
        Some(StyleInput::ViewTransitionUse {
            transition,
            key: Some(self.source_span(call.arguments[0].span())),
        })
    }

    fn animation_use(&mut self, expression: &Expression<'a>) -> Option<StyleInput> {
        let Expression::CallExpression(call) = expression else {
            return None;
        };
        let animation = self.animation_reference(&call.callee)?;
        if call.arguments.len() != 1 {
            self.diagnostics.push(Diagnostic::error(
                "SHANIMATION002",
                "an animation use requires one literal options string",
                self.source_span(call.span),
            ));
            return None;
        }
        let Expression::StringLiteral(options) = call.arguments[0].to_expression() else {
            self.diagnostics.push(Diagnostic::error(
                "SHANIMATION002",
                "an animation use requires one literal options string",
                self.source_span(call.arguments[0].span()),
            ));
            return None;
        };
        Some(StyleInput::AnimationUse {
            animation,
            options: self.string_literal(options),
        })
    }

    fn property_name(&self, key: &PropertyKey<'a>) -> Option<Spanned<String>> {
        match key {
            PropertyKey::StaticIdentifier(identifier) => Some(Spanned::new(
                identifier.name.to_string(),
                self.source_span(identifier.span),
            )),
            PropertyKey::StringLiteral(literal) => Some(self.string_literal(literal)),
            _ => None,
        }
    }

    fn collect_animation_definition(
        &mut self,
        name: Spanned<String>,
        call: &CallExpression<'a>,
    ) -> Option<AnimationDefinition> {
        if call.arguments.len() != 1 {
            self.diagnostics.push(Diagnostic::error(
                "SHANIMATION001",
                "style.animation() requires one object of static keyframe utility strings",
                self.source_span(call.span),
            ));
            return None;
        }
        let Expression::ObjectExpression(object) = call.arguments[0].to_expression() else {
            self.diagnostics.push(Diagnostic::error(
                "SHANIMATION001",
                "style.animation() requires one object of static keyframe utility strings",
                self.source_span(call.arguments[0].span()),
            ));
            return None;
        };
        let mut frames = Vec::new();
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                self.diagnostics.push(Diagnostic::error(
                    "SHANIMATION001",
                    "animation keyframes cannot use object spreads",
                    self.source_span(property.span()),
                ));
                continue;
            };
            if property.computed || property.method || property.kind != PropertyKind::Init {
                self.diagnostics.push(Diagnostic::error(
                    "SHANIMATION001",
                    "animation keyframes must be plain static properties",
                    self.source_span(property.span),
                ));
                continue;
            }
            let Some(selector) = self.property_name(&property.key) else {
                self.diagnostics.push(Diagnostic::error(
                    "SHANIMATION001",
                    "animation frames must be `from`, `to`, or percentage keys",
                    self.source_span(property.key.span()),
                ));
                continue;
            };
            if !is_animation_frame(&selector.value) {
                self.diagnostics.push(Diagnostic::error(
                    "SHANIMATION001",
                    "animation frames must be `from`, `to`, or comma-separated percentages",
                    selector.span,
                ));
                continue;
            }
            let Expression::StringLiteral(utilities) = &property.value else {
                self.diagnostics.push(Diagnostic::error(
                    "SHANIMATION001",
                    "animation frame values must be literal utility strings",
                    self.source_span(property.value.span()),
                ));
                continue;
            };
            frames.push(AnimationFrame {
                selector,
                utilities: self.string_literal(utilities),
            });
        }
        if frames.is_empty() {
            self.diagnostics.push(Diagnostic::error(
                "SHANIMATION001",
                "style.animation() requires at least one valid keyframe",
                self.source_span(call.span),
            ));
            return None;
        }
        Some(AnimationDefinition {
            name,
            call_span: self.source_span(call.span),
            frames,
        })
    }

    fn collect_view_transition_definition(
        &mut self,
        name: Spanned<String>,
        call: &CallExpression<'a>,
    ) -> Option<ViewTransitionDefinition> {
        if call.arguments.len() != 1 {
            self.diagnostics.push(Diagnostic::error(
                "SHVIEW001",
                "style.viewTransition() requires one object of static utility strings",
                self.source_span(call.span),
            ));
            return None;
        }
        let Expression::ObjectExpression(object) = call.arguments[0].to_expression() else {
            self.diagnostics.push(Diagnostic::error(
                "SHVIEW001",
                "style.viewTransition() requires one object of static utility strings",
                self.source_span(call.arguments[0].span()),
            ));
            return None;
        };
        let mut group = None;
        let mut old = None;
        let mut new = None;
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                self.diagnostics.push(Diagnostic::error(
                    "SHVIEW001",
                    "view transition definitions cannot use object spreads",
                    self.source_span(property.span()),
                ));
                continue;
            };
            if property.computed || property.method || property.kind != PropertyKind::Init {
                self.diagnostics.push(Diagnostic::error(
                    "SHVIEW001",
                    "view transition definitions must be plain static properties",
                    self.source_span(property.span),
                ));
                continue;
            }
            let Some(property_name) = self.property_name(&property.key) else {
                self.diagnostics.push(Diagnostic::error(
                    "SHVIEW001",
                    "view transition keys must be `group`, `old`, or `new`",
                    self.source_span(property.key.span()),
                ));
                continue;
            };
            let Expression::StringLiteral(value) = &property.value else {
                self.diagnostics.push(Diagnostic::error(
                    "SHVIEW001",
                    "view transition values must be literal utility strings",
                    self.source_span(property.value.span()),
                ));
                continue;
            };
            let value = self.string_literal(value);
            match property_name.value.as_str() {
                "group" => group = Some(value),
                "old" => old = Some(value),
                "new" => new = Some(value),
                _ => self.diagnostics.push(Diagnostic::error(
                    "SHVIEW001",
                    "view transition keys must be `group`, `old`, or `new`",
                    property_name.span,
                )),
            }
        }
        if group.is_none() && old.is_none() && new.is_none() {
            self.diagnostics.push(Diagnostic::error(
                "SHVIEW001",
                "style.viewTransition() requires at least one of group, old, or new",
                self.source_span(call.span),
            ));
            return None;
        }
        Some(ViewTransitionDefinition {
            name,
            call_span: self.source_span(call.span),
            group,
            old,
            new,
        })
    }

    #[allow(clippy::too_many_lines)] // Validates the complete escape-hatch object in one pass.
    fn collect_unsafe_call(&mut self, call: &CallExpression<'a>) -> Option<StyleCall> {
        if call.arguments.len() != 1 {
            self.diagnostics.push(Diagnostic::error(
                "SHUNSAFE001",
                "style.unsafe() requires one static { selector, css, reason } object",
                self.source_span(call.span),
            ));
            return None;
        }
        let Expression::ObjectExpression(object) = call.arguments[0].to_expression() else {
            self.diagnostics.push(Diagnostic::error(
                "SHUNSAFE001",
                "style.unsafe() requires one static { selector, css, reason } object",
                self.source_span(call.arguments[0].span()),
            ));
            return None;
        };
        let mut selector = None;
        let mut reason = None;
        let mut declarations = Vec::new();
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                self.diagnostics.push(Diagnostic::error(
                    "SHUNSAFE001",
                    "style.unsafe() does not support object spreads",
                    self.source_span(property.span()),
                ));
                continue;
            };
            if property.computed || property.method || property.kind != PropertyKind::Init {
                self.diagnostics.push(Diagnostic::error(
                    "SHUNSAFE001",
                    "style.unsafe() properties must be plain static entries",
                    self.source_span(property.span),
                ));
                continue;
            }
            let Some(name) = self.property_name(&property.key) else {
                self.diagnostics.push(Diagnostic::error(
                    "SHUNSAFE001",
                    "style.unsafe() property names must be static",
                    self.source_span(property.key.span()),
                ));
                continue;
            };
            match name.value.as_str() {
                "selector" => {
                    let Expression::StringLiteral(value) = &property.value else {
                        self.diagnostics.push(Diagnostic::error(
                            "SHUNSAFE001",
                            "unsafe selector must be a literal string",
                            self.source_span(property.value.span()),
                        ));
                        continue;
                    };
                    selector = Some(self.string_literal(value));
                }
                "reason" => {
                    let Expression::StringLiteral(value) = &property.value else {
                        self.diagnostics.push(Diagnostic::error(
                            "SHUNSAFE001",
                            "unsafe reason must be a literal string",
                            self.source_span(property.value.span()),
                        ));
                        continue;
                    };
                    reason = Some(self.string_literal(value));
                }
                "css" => {
                    let Expression::ObjectExpression(css) = &property.value else {
                        self.diagnostics.push(Diagnostic::error(
                            "SHUNSAFE001",
                            "unsafe css must be a static object",
                            self.source_span(property.value.span()),
                        ));
                        continue;
                    };
                    for declaration in &css.properties {
                        let ObjectPropertyKind::ObjectProperty(declaration) = declaration else {
                            self.diagnostics.push(Diagnostic::error(
                                "SHUNSAFE001",
                                "unsafe css does not support object spreads",
                                self.source_span(declaration.span()),
                            ));
                            continue;
                        };
                        if declaration.computed
                            || declaration.method
                            || declaration.kind != PropertyKind::Init
                        {
                            self.diagnostics.push(Diagnostic::error(
                                "SHUNSAFE001",
                                "unsafe css declarations must be plain static entries",
                                self.source_span(declaration.span),
                            ));
                            continue;
                        }
                        let Some(property) = self.property_name(&declaration.key) else {
                            self.diagnostics.push(Diagnostic::error(
                                "SHUNSAFE001",
                                "unsafe css property names must be static",
                                self.source_span(declaration.key.span()),
                            ));
                            continue;
                        };
                        let value = match &declaration.value {
                            Expression::StringLiteral(value) => self.string_literal(value).span,
                            Expression::NumericLiteral(value) => self.source_span(value.span),
                            _ => {
                                self.diagnostics.push(Diagnostic::error(
                                    "SHUNSAFE001",
                                    "unsafe css values must be static strings or numbers",
                                    self.source_span(declaration.value.span()),
                                ));
                                continue;
                            }
                        };
                        declarations.push(UnsafeDeclaration { property, value });
                    }
                }
                _ => self.diagnostics.push(Diagnostic::error(
                    "SHUNSAFE001",
                    "style.unsafe() only accepts selector, css, and reason",
                    name.span,
                )),
            }
        }
        let Some(selector) = selector else {
            self.diagnostics.push(Diagnostic::error(
                "SHUNSAFE001",
                "style.unsafe() requires a selector",
                self.source_span(call.span),
            ));
            return None;
        };
        if !selector.value.starts_with('&') {
            self.diagnostics.push(Diagnostic::error(
                "SHUNSAFE001",
                "unsafe selector must start with `&` so it remains rooted at this element",
                selector.span,
            ));
            return None;
        }
        let Some(reason) = reason.filter(|value| !value.value.trim().is_empty()) else {
            self.diagnostics.push(Diagnostic::error(
                "SHUNSAFE001",
                "style.unsafe() requires a non-empty reason",
                self.source_span(call.span),
            ));
            return None;
        };
        if declarations.is_empty() {
            self.diagnostics.push(Diagnostic::error(
                "SHUNSAFE001",
                "style.unsafe() requires at least one css declaration",
                self.source_span(call.span),
            ));
            return None;
        }
        Some(StyleCall {
            span: self.source_span(call.span),
            inputs: vec![StyleInput::Unsafe {
                selector,
                declarations,
                reason,
            }],
        })
    }

    fn collect_handle_projection(&mut self, expression: &Expression<'a>) -> Option<StyleInput> {
        if let Expression::CallExpression(call) = expression
            && let Expression::StaticMemberExpression(member) = &call.callee
            && member.property.name.as_str() == "at"
        {
            if call.arguments.len() != 2 {
                self.diagnostics.push(Diagnostic::error(
                    "SHHANDLE004",
                    "handle.at() requires a literal query and a literal utility string",
                    self.source_span(call.span),
                ));
                return None;
            }
            let Some(handle) = self.handle_reference(&member.object) else {
                self.diagnostics.push(Diagnostic::error(
                    "SHHANDLE001",
                    "handle projections must use an identity returned by style.handle()",
                    self.source_span(expression.span()),
                ));
                return None;
            };
            let (Expression::StringLiteral(query), Expression::StringLiteral(value)) = (
                call.arguments[0].to_expression(),
                call.arguments[1].to_expression(),
            ) else {
                self.diagnostics.push(Diagnostic::error(
                    "SHHANDLE004",
                    "handle.at() requires a literal query and a literal utility string",
                    self.source_span(call.span),
                ));
                return None;
            };
            return Some(StyleInput::ContainerQuery {
                handle,
                query: self.string_literal(query),
                value: self.string_literal(value),
            });
        }
        let (handle_expression, projection) = match expression {
            Expression::StaticMemberExpression(member) => {
                let projection = match member.property.name.as_str() {
                    "anchor" => HandleProjection::Anchor,
                    "position" => HandleProjection::Position,
                    "scope" => HandleProjection::Scope,
                    _ => return None,
                };
                (&member.object, projection)
            }
            Expression::CallExpression(call) => {
                let Expression::StaticMemberExpression(member) = &call.callee else {
                    return None;
                };
                if member.property.name.as_str() != "container" {
                    return None;
                }
                let Some(argument) = call.arguments.first() else {
                    self.diagnostics.push(Diagnostic::error(
                        "SHHANDLE002",
                        "handle.container() requires a literal container type",
                        self.source_span(call.span),
                    ));
                    return None;
                };
                let Expression::StringLiteral(value) = argument.to_expression() else {
                    self.diagnostics.push(Diagnostic::error(
                        "SHHANDLE002",
                        "handle.container() requires a literal container type",
                        self.source_span(argument.span()),
                    ));
                    return None;
                };
                (
                    &member.object,
                    HandleProjection::Container(self.string_literal(value)),
                )
            }
            _ => return None,
        };
        let Some(handle) = self.handle_reference(handle_expression) else {
            self.diagnostics.push(Diagnostic::error(
                "SHHANDLE001",
                "handle projections must use an identity returned by style.handle()",
                self.source_span(expression.span()),
            ));
            return None;
        };
        let projection_name = match &projection {
            HandleProjection::Anchor => "anchor",
            HandleProjection::Position => "position",
            HandleProjection::Scope => "scope",
            HandleProjection::Container(_) => "container",
        };
        self.handle_uses.push((
            handle.value.clone(),
            projection_name,
            self.source_span(expression.span()),
        ));
        Some(StyleInput::HandleProjection { handle, projection })
    }

    fn string_literal(&self, literal: &StringLiteral<'a>) -> Spanned<String> {
        let literal_span = self.source_span(literal.span);
        let content_span = Span::new(
            literal_span.file,
            literal_span.start.0 + 1,
            literal_span.end.0.saturating_sub(1),
        );
        Spanned::new(literal.value.to_string(), content_span)
    }

    fn handle_tagged_selector(
        &mut self,
        key: &PropertyKey<'a>,
    ) -> Option<(Spanned<String>, Spanned<String>)> {
        let PropertyKey::TaggedTemplateExpression(tagged) = key else {
            return None;
        };
        let Some(handle) = self.handle_reference(&tagged.tag) else {
            self.diagnostics.push(Diagnostic::error(
                "SHHANDLE001",
                "handle selectors must be tagged by an identity returned by style.handle()",
                self.source_span(tagged.tag.span()),
            ));
            return None;
        };
        if !tagged.quasi.expressions.is_empty() || tagged.quasi.quasis.len() != 1 {
            self.diagnostics.push(Diagnostic::error(
                "SHHANDLE005",
                "a handle selector must be a template literal without substitutions",
                self.source_span(tagged.quasi.span),
            ));
            return None;
        }
        let Some(cooked) = tagged.quasi.quasis[0].value.cooked.as_ref() else {
            self.diagnostics.push(Diagnostic::error(
                "SHHANDLE005",
                "a handle selector must be a valid static template literal",
                self.source_span(tagged.quasi.span),
            ));
            return None;
        };
        let template_span = self.source_span(tagged.quasi.span);
        let selector = Spanned::new(
            cooked.to_string(),
            Span::new(
                template_span.file,
                template_span.start.0 + 1,
                template_span.end.0.saturating_sub(1),
            ),
        );
        if !selector.value.starts_with('&') {
            self.diagnostics.push(Diagnostic::error(
                "SHHANDLE005",
                "a handle selector must start with `&`",
                selector.span,
            ));
            return None;
        }
        self.handle_uses.push((
            handle.value.clone(),
            "selector",
            self.source_span(tagged.span),
        ));
        Some((handle, selector))
    }

    fn collect_style_expression(
        &mut self,
        expression: &Expression<'a>,
        inputs: &mut Vec<StyleInput>,
    ) {
        if let Some(projection) = self.collect_handle_projection(expression) {
            inputs.push(projection);
            return;
        }
        if let Some(animation) = self.animation_use(expression) {
            inputs.push(animation);
            return;
        }
        if let Some(transition) = self.view_transition_use(expression) {
            inputs.push(transition);
            return;
        }
        match expression {
            Expression::StringLiteral(literal) => {
                inputs.push(StyleInput::UtilityString(self.string_literal(literal)));
            }
            Expression::LogicalExpression(expression)
                if expression.operator == LogicalOperator::And =>
            {
                let Expression::StringLiteral(literal) = &expression.right else {
                    self.diagnostics.push(Diagnostic::error(
                        "SHJS002",
                        "a style() conditional must have a literal utility string on its right side",
                        self.source_span(expression.right.span()),
                    ));
                    return;
                };
                inputs.push(StyleInput::ConditionalUtilityString {
                    guard: self.source_span(expression.left.span()),
                    when_true: true,
                    value: self.string_literal(literal),
                });
            }
            Expression::ConditionalExpression(expression) => {
                let (Expression::StringLiteral(consequent), Expression::StringLiteral(alternate)) =
                    (&expression.consequent, &expression.alternate)
                else {
                    self.diagnostics.push(Diagnostic::error(
                        "SHJS002",
                        "a style() ternary must contain literal utility strings on both sides",
                        self.source_span(expression.span),
                    ));
                    return;
                };
                let guard = self.source_span(expression.test.span());
                inputs.push(StyleInput::ConditionalUtilityString {
                    guard,
                    when_true: true,
                    value: self.string_literal(consequent),
                });
                inputs.push(StyleInput::ConditionalUtilityString {
                    guard,
                    when_true: false,
                    value: self.string_literal(alternate),
                });
            }
            Expression::ArrayExpression(array) => {
                for element in &array.elements {
                    match element {
                        ArrayExpressionElement::SpreadElement(spread) => self.diagnostics.push(
                            Diagnostic::error(
                                "SHJS002",
                                "style() arrays do not support spread elements",
                                self.source_span(spread.span),
                            ),
                        ),
                        ArrayExpressionElement::Elision(elision) => self.diagnostics.push(
                            Diagnostic::error(
                                "SHJS002",
                                "style() arrays cannot contain empty slots",
                                self.source_span(elision.span),
                            ),
                        ),
                        _ => self.collect_style_expression(element.to_expression(), inputs),
                    }
                }
            }
            Expression::ObjectExpression(object) => self.collect_object(object, inputs),
            _ => self.diagnostics.push(Diagnostic::error(
                "SHJS002",
                "style() accepts literal utility strings, arrays, `condition && \"…\"` branches, attributes, and `&` selectors",
                self.source_span(expression.span()),
            )),
        }
    }

    #[allow(clippy::too_many_lines)] // Keeps the tightly coupled object-key diagnostics together.
    fn collect_object(&mut self, object: &ObjectExpression<'a>, inputs: &mut Vec<StyleInput>) {
        let mut entries = Vec::new();
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                self.diagnostics.push(Diagnostic::error(
                    "SHJS003",
                    "style() attribute objects do not support spread properties",
                    self.source_span(property.span()),
                ));
                continue;
            };
            if property.method || property.kind != PropertyKind::Init {
                self.diagnostics.push(Diagnostic::error(
                    "SHJS003",
                    "style() attributes must be plain key-value properties",
                    self.source_span(property.span),
                ));
                continue;
            }
            if property.computed {
                let Some((handle, selector)) = self.handle_tagged_selector(&property.key) else {
                    self.diagnostics.push(Diagnostic::error(
                        "SHJS003",
                        "computed style() keys must be static selectors tagged by style.handle()",
                        self.source_span(property.key.span()),
                    ));
                    continue;
                };
                match &property.value {
                    Expression::StringLiteral(value) => {
                        inputs.push(StyleInput::HandleSelectorUtilityString {
                            handle,
                            selector,
                            value: self.string_literal(value),
                        });
                    }
                    Expression::LogicalExpression(expression)
                        if expression.operator == LogicalOperator::And =>
                    {
                        let Expression::StringLiteral(value) = &expression.right else {
                            self.diagnostics.push(Diagnostic::error(
                                "SHJS004",
                                "a handle selector conditional must have a literal utility string on its right side",
                                self.source_span(expression.right.span()),
                            ));
                            continue;
                        };
                        inputs.push(StyleInput::ConditionalHandleSelectorUtilityString {
                            handle,
                            selector,
                            guard: self.source_span(expression.left.span()),
                            when_true: true,
                            value: self.string_literal(value),
                        });
                    }
                    _ => self.diagnostics.push(Diagnostic::error(
                        "SHJS004",
                        "a handle selector must map to a literal utility string or `condition && \"…\"`",
                        self.source_span(property.value.span()),
                    )),
                }
                continue;
            }
            let PropertyKey::StringLiteral(name) = &property.key else {
                self.diagnostics.push(Diagnostic::error(
                    "SHJS003",
                    "style() attribute keys must be quoted aria-* or data-* names",
                    self.source_span(property.key.span()),
                ));
                continue;
            };
            let name = self.string_literal(name);
            if name.value.starts_with("aria-") || name.value.starts_with("data-") {
                entries.push(AttributeEntry {
                    name,
                    value: self.source_span(property.value.span()),
                });
                continue;
            }
            if !name.value.starts_with('&') {
                self.diagnostics.push(Diagnostic::error(
                    "SHJS003",
                    "style() object keys must be quoted aria-*/data-* attributes or `&` selectors",
                    name.span,
                ));
                continue;
            }
            match &property.value {
                Expression::CallExpression(_) => {
                    let Some(StyleInput::AnimationUse { animation, options }) =
                        self.animation_use(&property.value)
                    else {
                        self.diagnostics.push(Diagnostic::error(
                            "SHJS004",
                            "a `&` selector must map to a literal utility string, animation use, or `condition && \"…\"`",
                            self.source_span(property.value.span()),
                        ));
                        continue;
                    };
                    inputs.push(StyleInput::SelectorAnimationUse {
                        selector: name,
                        animation,
                        options,
                    });
                }
                Expression::StringLiteral(value) => {
                    inputs.push(StyleInput::SelectorUtilityString {
                        selector: name,
                        value: self.string_literal(value),
                    });
                }
                Expression::LogicalExpression(expression)
                    if expression.operator == LogicalOperator::And =>
                {
                    let Expression::StringLiteral(value) = &expression.right else {
                        self.diagnostics.push(Diagnostic::error(
                            "SHJS004",
                            "a selector conditional must have a literal utility string on its right side",
                            self.source_span(expression.right.span()),
                        ));
                        continue;
                    };
                    inputs.push(StyleInput::ConditionalSelectorUtilityString {
                        selector: name,
                        guard: self.source_span(expression.left.span()),
                        when_true: true,
                        value: self.string_literal(value),
                    });
                }
                _ => self.diagnostics.push(Diagnostic::error(
                    "SHJS004",
                    "a `&` selector must map to a literal utility string or `condition && \"…\"`",
                    self.source_span(property.value.span()),
                )),
            }
        }
        if !entries.is_empty() {
            inputs.push(StyleInput::Attributes(entries));
        }
    }

    fn validate_handles(&mut self) {
        let names = self
            .handle_uses
            .iter()
            .map(|(name, _, _)| name.clone())
            .collect::<HashSet<_>>();
        for name in names {
            let uses = self
                .handle_uses
                .iter()
                .filter(|(candidate, _, _)| candidate == &name)
                .collect::<Vec<_>>();
            let has_anchor = uses
                .iter()
                .any(|(_, projection, _)| *projection == "anchor");
            if !has_anchor {
                for (_, _, span) in uses
                    .iter()
                    .filter(|(_, projection, _)| *projection == "position")
                {
                    self.diagnostics.push(Diagnostic::error(
                        "SHHANDLE003",
                        format!("handle `{name}` uses position without an anchor projection"),
                        *span,
                    ));
                }
            }
        }
    }
}

impl<'a> Visit<'a> for MacroExtractor<'a> {
    fn visit_import_declaration(&mut self, declaration: &ImportDeclaration<'a>) {
        if Self::is_style_import(declaration)
            && let Some(specifiers) = &declaration.specifiers
        {
            for specifier in specifiers {
                let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                    continue;
                };
                let ModuleExportName::IdentifierName(imported) = &specifier.imported else {
                    continue;
                };
                if imported.name.as_str() == "style"
                    && let Some(symbol_id) = specifier.local.symbol_id.get()
                {
                    self.style_symbols.insert(symbol_id);
                }
            }
        }
        walk::walk_import_declaration(self, declaration);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if self.is_style_member(&call.callee, "unsafe") {
            if let Some(style_call) = self.collect_unsafe_call(call) {
                self.style_calls.push(style_call);
            }
            walk::walk_call_expression(self, call);
            return;
        }
        let Expression::Identifier(callee) = &call.callee else {
            walk::walk_call_expression(self, call);
            return;
        };
        let Some(reference_id) = callee.reference_id.get() else {
            walk::walk_call_expression(self, call);
            return;
        };
        let Some(symbol_id) = self.scoping.get_reference(reference_id).symbol_id() else {
            walk::walk_call_expression(self, call);
            return;
        };
        if !self.style_symbols.contains(&symbol_id) {
            walk::walk_call_expression(self, call);
            return;
        }

        let mut inputs = Vec::new();
        for argument in &call.arguments {
            if let Argument::SpreadElement(spread) = argument {
                self.diagnostics.push(Diagnostic::error(
                    "SHJS002",
                    "style() does not support spread arguments",
                    self.source_span(spread.span),
                ));
            } else {
                self.collect_style_expression(argument.to_expression(), &mut inputs);
            }
        }
        self.style_calls.push(StyleCall {
            span: self.source_span(call.span),
            inputs,
        });

        walk::walk_call_expression(self, call);
    }

    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if let (
            BindingPattern::BindingIdentifier(identifier),
            Some(Expression::CallExpression(call)),
        ) = (&declarator.id, &declarator.init)
            && self.is_style_handle_call(call)
            && let Some(symbol_id) = identifier.symbol_id.get()
        {
            self.handle_declarations.push(self.source_span(call.span));
            self.handle_symbols.insert(
                symbol_id,
                Spanned::new(
                    identifier.name.to_string(),
                    self.source_span(identifier.span),
                ),
            );
        }
        if let (
            BindingPattern::BindingIdentifier(identifier),
            Some(Expression::CallExpression(call)),
        ) = (&declarator.id, &declarator.init)
            && self.is_style_animation_call(call)
            && let Some(symbol_id) = identifier.symbol_id.get()
        {
            let name = Spanned::new(
                identifier.name.to_string(),
                self.source_span(identifier.span),
            );
            if let Some(definition) = self.collect_animation_definition(name.clone(), call) {
                self.animation_symbols.insert(symbol_id, name);
                self.animation_declarations.push(definition);
            }
        }
        if let (
            BindingPattern::BindingIdentifier(identifier),
            Some(Expression::CallExpression(call)),
        ) = (&declarator.id, &declarator.init)
            && self.is_style_view_transition_call(call)
            && let Some(symbol_id) = identifier.symbol_id.get()
        {
            let name = Spanned::new(
                identifier.name.to_string(),
                self.source_span(identifier.span),
            );
            if let Some(definition) = self.collect_view_transition_definition(name.clone(), call) {
                self.view_transition_symbols.insert(symbol_id, name);
                self.view_transition_declarations.push(definition);
            }
        }
        walk::walk_variable_declarator(self, declarator);
    }
}

fn is_animation_frame(value: &str) -> bool {
    value == "from"
        || value == "to"
        || value.split(',').all(|part| {
            part.trim()
                .strip_suffix('%')
                .is_some_and(|number| number.trim().parse::<u8>().is_ok())
        })
}

#[cfg(test)]
mod tests {
    use shorth_source::FileId;

    use super::*;

    #[test]
    fn accepts_typescript_jsx() {
        let source = SourceFile::new(
            FileId(1),
            "Button.tsx",
            "const Button = ({ loading }: { loading: boolean }) => <button>{loading}</button>;",
        );

        assert!(parse(&source).diagnostics.is_empty());
    }

    #[test]
    fn extracts_only_an_import_bound_style_call_with_literal_spans() {
        let source = SourceFile::new(
            FileId(1),
            "Button.tsx",
            r#"import { style as s } from "stillwind/react";
const props = s("px-4 text-center", "hover:bg-accent");
function local(s: (value: string) => unknown) { s("not-a-style-call"); }"#,
        );

        let parsed = parse(&source);

        assert!(parsed.diagnostics.is_empty());
        assert_eq!(parsed.module.style_calls.len(), 1);
        let Some(StyleInput::UtilityString(first)) = parsed.module.style_calls[0].inputs.first()
        else {
            panic!("the first input should be a static utility string");
        };
        assert_eq!(first.value, "px-4 text-center");
        assert_eq!(source.slice(first.span).unwrap(), "px-4 text-center");
    }

    #[test]
    fn extracts_aria_and_data_attributes_with_value_expression_spans() {
        let source = SourceFile::new(
            FileId(1),
            "Button.tsx",
            r#"import { style } from "stillwind/react";
const props = style({ "aria-busy": loading, "data-state": state });"#,
        );

        let parsed = parse(&source);

        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let Some(StyleInput::Attributes(entries)) = parsed.module.style_calls[0].inputs.first()
        else {
            panic!("the first input should be an attribute object");
        };
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name.value, "aria-busy");
        assert_eq!(source.slice(entries[0].value).unwrap(), "loading");
        assert_eq!(entries[1].name.value, "data-state");
        assert_eq!(source.slice(entries[1].value).unwrap(), "state");
    }

    #[test]
    fn flattens_arrays_and_retains_element_local_selectors() {
        let source = SourceFile::new(
            FileId(1),
            "Button.tsx",
            r#"import { style } from "stillwind/react";
declare const loading: boolean;
const props = style(["px-4", loading && "opacity-60"], { "&:hover": "bg-accent" });"#,
        );

        let parsed = parse(&source);

        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        assert!(matches!(
            parsed.module.style_calls[0].inputs[0],
            StyleInput::UtilityString(_)
        ));
        assert!(matches!(
            parsed.module.style_calls[0].inputs[1],
            StyleInput::ConditionalUtilityString { .. }
        ));
        let Some(StyleInput::SelectorUtilityString { selector, value }) =
            parsed.module.style_calls[0].inputs.get(2)
        else {
            panic!("the selector should be preserved in the style IR");
        };
        assert_eq!(selector.value, "&:hover");
        assert_eq!(value.value, "bg-accent");
    }
}
