//! Oxc-backed JS/TS/JSX parsing and the first import-bound `style()` extractor.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
};

use oxc::{
    allocator::Allocator,
    ast::ast::{
        Argument, ArrayExpressionElement, BindingPattern, CallExpression, Declaration,
        ExportAllDeclaration, ExportDeclaration, ExportFromDeclaration, ExportNamedDeclaration,
        Expression, ImportDeclaration, ImportDeclarationSpecifier, JSXElement, ModuleExportName,
        ObjectExpression, ObjectPropertyKind, PropertyKey, PropertyKind, StaticMemberExpression,
        StringLiteral, VariableDeclarator,
    },
    ast_visit::{Visit, walk},
    parser::Parser,
    semantic::{Scoping, SemanticBuilder, SymbolId},
    span::{GetSpan, SourceType},
    syntax::operator::LogicalOperator,
};
use stillwind_ir::{
    AnimationDefinition, AnimationFrame, AssetReference, AttributeEntry, HandleDefinition,
    HandleProjection, ModuleIr, ScopeDefinition, StyleCall, StyleInput, ViewTransitionDefinition,
};
use stillwind_semantic::{
    AssetKind, AssetMetadata, AssetRecord, ExportedValue, IdentityId, ImportBinding, ModuleSummary,
    ReExport, RefAlternative, SymbolicValue, TypeFact,
};
use stillwind_source::{CompileError, LintFinding, SourceFile, Span, Spanned};

#[derive(Clone, Debug)]
pub struct ParsedJavaScript {
    pub diagnostics: Vec<CompileError>,
    pub lint_findings: Vec<LintFinding>,
    pub module: ModuleIr,
    pub summary: ModuleSummary,
}

pub fn parse(source: &SourceFile) -> ParsedJavaScript {
    parse_with_type_facts(source, &[], &BTreeMap::new())
}

/// Parses a module using expression facts produced by the TypeScript checker.
pub fn parse_with_type_facts(
    source: &SourceFile,
    type_facts: &[TypeFact],
    assets: &BTreeMap<IdentityId, AssetMetadata>,
) -> ParsedJavaScript {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(Path::new(source.path())).unwrap_or_default();
    let result = Parser::new(&allocator, source.text(), source_type).parse();

    let mut diagnostics: Vec<_> = result
        .diagnostics
        .into_iter()
        .map(|diagnostic| CompileError::new("SHJS001", diagnostic.to_string(), source.full_span()))
        .collect();

    let semantic = SemanticBuilder::new().build(&result.program).semantic;
    let mut extractor = MacroExtractor::new(source, semantic.scoping(), type_facts, assets);
    extractor.visit_program(&result.program);
    extractor.validate_handles();
    let summary = extractor.module_summary();
    let mut lint_findings = Vec::new();
    for diagnostic in extractor.diagnostics {
        if let Some(rule) = lint_rule_for_code(diagnostic.code) {
            let mut finding = LintFinding::new(
                rule,
                diagnostic.message,
                diagnostic
                    .labels
                    .first()
                    .map_or(source.full_span(), |label| label.span),
            );
            finding.labels = diagnostic.labels;
            finding.notes = diagnostic.notes;
            lint_findings.push(finding);
        } else {
            diagnostics.push(diagnostic);
        }
    }

    ParsedJavaScript {
        diagnostics,
        lint_findings,
        summary,
        module: ModuleIr {
            adapter: stillwind_ir::AdapterKind::React,
            style_calls: extractor.style_calls,
            handle_definitions: extractor.handle_definitions,
            scope_definitions: extractor.scope_definitions,
            animation_declarations: extractor.animation_declarations,
            view_transition_declarations: extractor.view_transition_declarations,
        },
    }
}

fn lint_rule_for_code(code: &str) -> Option<&'static str> {
    Some(match code {
        "SHREF003" => "require-anchor-provider",
        "SHREF006" => "require-container-provider",
        "SHREF007" => "valid-ref-ancestry",
        "SHSCOPE002" => "require-scope-provider",
        "SHSCOPE003" => "valid-scope-ancestry",
        "SHSCOPE004" => "scoped-selector-in-scope",
        _ => return None,
    })
}

struct MacroExtractor<'a> {
    source: &'a SourceFile,
    scoping: &'a Scoping,
    style_symbols: HashSet<SymbolId>,
    handle_symbols: HashMap<SymbolId, HandleDefinition>,
    scope_symbols: HashMap<SymbolId, ScopeDefinition>,
    handle_uses: Vec<HandleUse>,
    scope_uses: Vec<ScopeUse>,
    handle_definitions: Vec<HandleDefinition>,
    scope_definitions: Vec<ScopeDefinition>,
    animation_symbols: HashMap<SymbolId, Spanned<String>>,
    animation_declarations: Vec<AnimationDefinition>,
    view_transition_symbols: HashMap<SymbolId, Spanned<String>>,
    view_transition_declarations: Vec<ViewTransitionDefinition>,
    local_values: HashMap<SymbolId, ExportedValue>,
    style_calls: Vec<StyleCall>,
    diagnostics: Vec<CompileError>,
    jsx_path: Vec<usize>,
    next_jsx_element: usize,
    imports: Vec<ImportBinding>,
    re_exports: Vec<ReExport>,
    export_symbols: Vec<(SymbolId, String)>,
    type_facts: HashMap<(u32, u32), ExportedValue>,
    imported_assets: &'a BTreeMap<IdentityId, AssetMetadata>,
}

#[derive(Clone, Debug)]
struct HandleUse {
    name: String,
    projection: &'static str,
    span: Span,
    jsx_path: Option<Vec<usize>>,
}

#[derive(Clone, Debug)]
struct ScopeUse {
    name: String,
    jsx_path: Option<Vec<usize>>,
}

impl<'a> MacroExtractor<'a> {
    fn new(
        source: &'a SourceFile,
        scoping: &'a Scoping,
        type_facts: &[TypeFact],
        imported_assets: &'a BTreeMap<IdentityId, AssetMetadata>,
    ) -> Self {
        Self {
            source,
            scoping,
            style_symbols: HashSet::new(),
            handle_symbols: HashMap::new(),
            scope_symbols: HashMap::new(),
            handle_uses: Vec::new(),
            scope_uses: Vec::new(),
            handle_definitions: Vec::new(),
            scope_definitions: Vec::new(),
            animation_symbols: HashMap::new(),
            animation_declarations: Vec::new(),
            view_transition_symbols: HashMap::new(),
            view_transition_declarations: Vec::new(),
            local_values: HashMap::new(),
            style_calls: Vec::new(),
            diagnostics: Vec::new(),
            jsx_path: Vec::new(),
            next_jsx_element: 0,
            imports: Vec::new(),
            re_exports: Vec::new(),
            export_symbols: Vec::new(),
            type_facts: type_facts
                .iter()
                .map(|fact| ((fact.start, fact.end), fact.value.clone()))
                .collect(),
            imported_assets,
        }
    }

    fn imported_scope(&self, identity: &IdentityId) -> ScopeDefinition {
        if let Some(scope) = self
            .scope_definitions
            .iter()
            .find(|scope| self.identity(AssetKind::Scope, &scope.name) == *identity)
        {
            return scope.clone();
        }
        let span = Span::new(self.source.id(), identity.start, identity.end);
        let metadata = self.imported_assets.get(identity);
        ScopeDefinition {
            module_path: identity.module_path.clone(),
            name: Spanned::new(identity.binding_name.clone(), span),
            label: metadata
                .and_then(|metadata| metadata.label.clone())
                .map(|label| Spanned::new(label, span)),
            call_span: span,
        }
    }

    fn imported_ref(&self, alternative: &RefAlternative) -> Option<HandleDefinition> {
        let span = Span::new(
            self.source.id(),
            alternative.reference.start,
            alternative.reference.end,
        );
        let metadata = self.imported_assets.get(&alternative.reference);
        let scope = match &alternative.permitted_scopes {
            None => None,
            Some(scopes) => {
                let candidates = scopes.finite_candidates()?;
                if candidates.len() != 1 {
                    return None;
                }
                Some(self.imported_scope(candidates.first()?))
            }
        };
        let mut definition = self
            .handle_definitions
            .iter()
            .find(|handle| self.identity(AssetKind::Ref, &handle.name) == alternative.reference)
            .cloned()
            .unwrap_or_else(|| HandleDefinition {
                module_path: alternative.reference.module_path.clone(),
                name: Spanned::new(alternative.reference.binding_name.clone(), span),
                label: metadata
                    .and_then(|metadata| metadata.label.clone())
                    .map(|label| Spanned::new(label, span)),
                scope: None,
                call_span: span,
            });
        definition.scope = scope;
        Some(definition)
    }

    fn identity(&self, kind: AssetKind, name: &Spanned<String>) -> IdentityId {
        IdentityId::new(
            kind,
            self.source.path(),
            name.span.start.0,
            name.span.end.0,
            name.value.clone(),
        )
    }

    fn module_summary(&self) -> ModuleSummary {
        let mut summary = ModuleSummary::new(self.source.path());
        summary.imports.clone_from(&self.imports);
        summary.re_exports.clone_from(&self.re_exports);

        for definition in &self.scope_definitions {
            summary.assets.push(AssetRecord {
                identity: self.identity(AssetKind::Scope, &definition.name),
                metadata: AssetMetadata {
                    label: definition.label.as_ref().map(|label| label.value.clone()),
                },
                call_start: definition.call_span.start.0,
                call_end: definition.call_span.end.0,
            });
        }
        for definition in &self.handle_definitions {
            summary.assets.push(AssetRecord {
                identity: self.identity(AssetKind::Ref, &definition.name),
                metadata: AssetMetadata {
                    label: definition.label.as_ref().map(|label| label.value.clone()),
                },
                call_start: definition.call_span.start.0,
                call_end: definition.call_span.end.0,
            });
        }
        for definition in &self.animation_declarations {
            summary.assets.push(AssetRecord {
                identity: self.identity(AssetKind::Animation, &definition.name),
                metadata: AssetMetadata { label: None },
                call_start: definition.call_span.start.0,
                call_end: definition.call_span.end.0,
            });
        }
        for definition in &self.view_transition_declarations {
            summary.assets.push(AssetRecord {
                identity: self.identity(AssetKind::ViewTransition, &definition.name),
                metadata: AssetMetadata { label: None },
                call_start: definition.call_span.start.0,
                call_end: definition.call_span.end.0,
            });
        }

        for (symbol, exported) in &self.export_symbols {
            if let Some(value) = self.local_values.get(symbol) {
                summary.exports.insert(exported.clone(), value.clone());
            }
        }
        summary
    }

    fn module_export_name(name: &ModuleExportName<'a>) -> String {
        match name {
            ModuleExportName::IdentifierName(name) => name.name.to_string(),
            ModuleExportName::IdentifierReference(name) => name.name.to_string(),
            ModuleExportName::StringLiteral(name) => name.value.to_string(),
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

    fn is_style_ref_call(&self, call: &CallExpression<'a>) -> bool {
        self.is_style_member(&call.callee, "ref")
    }

    fn is_style_scope_call(&self, call: &CallExpression<'a>) -> bool {
        self.is_style_member(&call.callee, "scope")
    }

    fn is_style_animation_call(&self, call: &CallExpression<'a>) -> bool {
        self.is_style_member(&call.callee, "animation")
    }

    fn is_style_view_transition_call(&self, call: &CallExpression<'a>) -> bool {
        self.is_style_member(&call.callee, "viewTransition")
    }

    fn handle_reference(&self, expression: &Expression<'a>) -> Option<HandleDefinition> {
        let handles = self.handle_references(expression)?;
        (handles.len() == 1).then(|| handles[0].clone())
    }

    fn handle_references(&self, expression: &Expression<'a>) -> Option<Vec<HandleDefinition>> {
        let span = expression.span();
        if let Some(ExportedValue::Refs(alternatives)) =
            self.type_facts.get(&(span.start, span.end))
        {
            return self.handles_from_alternatives(alternatives);
        }
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        let reference_id = identifier.reference_id.get()?;
        let symbol_id = self.scoping.get_reference(reference_id).symbol_id()?;
        if let Some(handle) = self.handle_symbols.get(&symbol_id) {
            return Some(vec![handle.clone()]);
        }
        let ExportedValue::Refs(alternatives) = self.local_values.get(&symbol_id)? else {
            return None;
        };
        self.handles_from_alternatives(alternatives)
    }

    fn handles_from_alternatives(
        &self,
        alternatives: &[RefAlternative],
    ) -> Option<Vec<HandleDefinition>> {
        let mut handles = Vec::new();
        for alternative in alternatives {
            match &alternative.permitted_scopes {
                None => handles.push(self.imported_ref(alternative)?),
                Some(scopes) => {
                    for scope in scopes.finite_candidates()? {
                        let mut alternative = alternative.clone();
                        alternative.permitted_scopes = Some(SymbolicValue::exact(scope.clone()));
                        handles.push(self.imported_ref(&alternative)?);
                    }
                }
            }
        }
        Some(handles)
    }

    fn scope_reference(&self, expression: &Expression<'a>) -> Option<ScopeDefinition> {
        let scopes = self.scope_references(expression)?;
        (scopes.len() == 1).then(|| scopes[0].clone())
    }

    fn scope_references(&self, expression: &Expression<'a>) -> Option<Vec<ScopeDefinition>> {
        let span = expression.span();
        if let Some(ExportedValue::Scopes(value)) = self.type_facts.get(&(span.start, span.end)) {
            return value
                .finite_candidates()?
                .iter()
                .map(|identity| Some(self.imported_scope(identity)))
                .collect();
        }
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        let reference_id = identifier.reference_id.get()?;
        let symbol_id = self.scoping.get_reference(reference_id).symbol_id()?;
        if let Some(scope) = self.scope_symbols.get(&symbol_id) {
            return Some(vec![scope.clone()]);
        }
        let ExportedValue::Scopes(value) = self.local_values.get(&symbol_id)? else {
            return None;
        };
        Some(
            value
                .finite_candidates()?
                .iter()
                .map(|identity| self.imported_scope(identity))
                .collect(),
        )
    }

    fn asset_reference(&self, identity: &IdentityId) -> AssetReference {
        AssetReference {
            module_path: identity.module_path.clone(),
            name: Spanned::new(
                identity.binding_name.clone(),
                Span::new(self.source.id(), identity.start, identity.end),
            ),
        }
    }

    fn animation_references(&self, expression: &Expression<'a>) -> Option<Vec<AssetReference>> {
        let span = expression.span();
        if let Some(ExportedValue::Animations(value)) = self.type_facts.get(&(span.start, span.end))
        {
            return value
                .finite_candidates()?
                .iter()
                .map(|identity| Some(self.asset_reference(identity)))
                .collect();
        }
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        let reference_id = identifier.reference_id.get()?;
        let symbol_id = self.scoping.get_reference(reference_id).symbol_id()?;
        let animation = self.animation_symbols.get(&symbol_id)?;
        Some(vec![AssetReference {
            module_path: self.source.path().to_owned(),
            name: animation.clone(),
        }])
    }

    fn view_transition_references(
        &self,
        expression: &Expression<'a>,
    ) -> Option<Vec<AssetReference>> {
        let span = expression.span();
        if let Some(ExportedValue::ViewTransitions(value)) =
            self.type_facts.get(&(span.start, span.end))
        {
            return value
                .finite_candidates()?
                .iter()
                .map(|identity| Some(self.asset_reference(identity)))
                .collect();
        }
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        let reference_id = identifier.reference_id.get()?;
        let symbol_id = self.scoping.get_reference(reference_id).symbol_id()?;
        let transition = self.view_transition_symbols.get(&symbol_id)?;
        Some(vec![AssetReference {
            module_path: self.source.path().to_owned(),
            name: transition.clone(),
        }])
    }

    fn view_transition_use(&mut self, expression: &Expression<'a>) -> Option<StyleInput> {
        let expression_has_type_fact = self
            .type_facts
            .contains_key(&(expression.span().start, expression.span().end));
        let (value, transitions, key) = match expression {
            Expression::CallExpression(call) => {
                let Expression::StaticMemberExpression(member) = &call.callee else {
                    return None;
                };
                if member.property.name.as_str() != "for" {
                    return None;
                }
                let transitions = self.view_transition_references(&member.object)?;
                if call.arguments.len() != 1 {
                    self.diagnostics.push(CompileError::new(
                        "SHVIEW002",
                        "viewTransition.for() requires one stable key expression",
                        self.source_span(call.span),
                    ));
                    return None;
                }
                (
                    self.source_span(member.object.span()),
                    transitions,
                    Some(self.source_span(call.arguments[0].span())),
                )
            }
            _ => (
                self.source_span(expression.span()),
                self.view_transition_references(expression)?,
                None,
            ),
        };
        if expression_has_type_fact && key.is_none() {
            return (!transitions.is_empty()).then_some(StyleInput::DynamicViewTransitionUse {
                value,
                transitions,
                key: None,
            });
        }
        match transitions.as_slice() {
            [transition] => Some(StyleInput::ViewTransitionUse {
                transition: transition.clone(),
                key,
            }),
            [] => None,
            _ => Some(StyleInput::DynamicViewTransitionUse {
                value,
                transitions,
                key,
            }),
        }
    }

    fn animation_use(&mut self, expression: &Expression<'a>) -> Option<StyleInput> {
        let Expression::CallExpression(call) = expression else {
            return None;
        };
        let animations = self.animation_references(&call.callee)?;
        if call.arguments.len() != 1 {
            self.diagnostics.push(CompileError::new(
                "SHANIMATION002",
                "an animation use requires one literal options string",
                self.source_span(call.span),
            ));
            return None;
        }
        let Expression::StringLiteral(options) = call.arguments[0].to_expression() else {
            self.diagnostics.push(CompileError::new(
                "SHANIMATION002",
                "an animation use requires one literal options string",
                self.source_span(call.arguments[0].span()),
            ));
            return None;
        };
        let options = self.string_literal(options);
        match animations.as_slice() {
            [animation] => Some(StyleInput::AnimationUse {
                animation: animation.clone(),
                options,
            }),
            [] => None,
            _ => Some(StyleInput::DynamicAnimationUse {
                value: self.source_span(call.span),
                animations,
                options,
            }),
        }
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
            self.diagnostics.push(CompileError::new(
                "SHANIMATION001",
                "style.animation() requires one object of static keyframe utility strings",
                self.source_span(call.span),
            ));
            return None;
        }
        let Expression::ObjectExpression(object) = call.arguments[0].to_expression() else {
            self.diagnostics.push(CompileError::new(
                "SHANIMATION001",
                "style.animation() requires one object of static keyframe utility strings",
                self.source_span(call.arguments[0].span()),
            ));
            return None;
        };
        let mut frames = Vec::new();
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                self.diagnostics.push(CompileError::new(
                    "SHANIMATION001",
                    "animation keyframes cannot use object spreads",
                    self.source_span(property.span()),
                ));
                continue;
            };
            if property.computed || property.method || property.kind != PropertyKind::Init {
                self.diagnostics.push(CompileError::new(
                    "SHANIMATION001",
                    "animation keyframes must be plain static properties",
                    self.source_span(property.span),
                ));
                continue;
            }
            let Some(selector) = self.property_name(&property.key) else {
                self.diagnostics.push(CompileError::new(
                    "SHANIMATION001",
                    "animation frames must be `from`, `to`, or percentage keys",
                    self.source_span(property.key.span()),
                ));
                continue;
            };
            if !is_animation_frame(&selector.value) {
                self.diagnostics.push(CompileError::new(
                    "SHANIMATION001",
                    "animation frames must be `from`, `to`, or comma-separated percentages",
                    selector.span,
                ));
                continue;
            }
            let Expression::StringLiteral(utilities) = &property.value else {
                self.diagnostics.push(CompileError::new(
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
            self.diagnostics.push(CompileError::new(
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
            self.diagnostics.push(CompileError::new(
                "SHVIEW001",
                "style.viewTransition() requires one object of static utility strings",
                self.source_span(call.span),
            ));
            return None;
        }
        let Expression::ObjectExpression(object) = call.arguments[0].to_expression() else {
            self.diagnostics.push(CompileError::new(
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
                self.diagnostics.push(CompileError::new(
                    "SHVIEW001",
                    "view transition definitions cannot use object spreads",
                    self.source_span(property.span()),
                ));
                continue;
            };
            if property.computed || property.method || property.kind != PropertyKind::Init {
                self.diagnostics.push(CompileError::new(
                    "SHVIEW001",
                    "view transition definitions must be plain static properties",
                    self.source_span(property.span),
                ));
                continue;
            }
            let Some(property_name) = self.property_name(&property.key) else {
                self.diagnostics.push(CompileError::new(
                    "SHVIEW001",
                    "view transition keys must be `group`, `old`, or `new`",
                    self.source_span(property.key.span()),
                ));
                continue;
            };
            let Expression::StringLiteral(value) = &property.value else {
                self.diagnostics.push(CompileError::new(
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
                _ => self.diagnostics.push(CompileError::new(
                    "SHVIEW001",
                    "view transition keys must be `group`, `old`, or `new`",
                    property_name.span,
                )),
            }
        }
        if group.is_none() && old.is_none() && new.is_none() {
            self.diagnostics.push(CompileError::new(
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

    fn collect_handle_projection(&mut self, expression: &Expression<'a>) -> Option<StyleInput> {
        if let Expression::CallExpression(call) = expression
            && let Expression::StaticMemberExpression(member) = &call.callee
            && member.property.name.as_str() == "at"
        {
            return self.collect_container_query_projection(call, member);
        }
        let (handle_expression, projection) = match expression {
            Expression::StaticMemberExpression(member) => {
                let projection = match member.property.name.as_str() {
                    "anchor" => HandleProjection::Anchor,
                    "position" => HandleProjection::Position,
                    "anchorScope" => HandleProjection::Scope,
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
                    self.diagnostics.push(CompileError::new(
                        "SHREF002",
                        "ref.container() requires a literal container type",
                        self.source_span(call.span),
                    ));
                    return None;
                };
                let Expression::StringLiteral(value) = argument.to_expression() else {
                    self.diagnostics.push(CompileError::new(
                        "SHREF002",
                        "ref.container() requires a literal container type",
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
        let Some(handles) = self.handle_references(handle_expression) else {
            self.diagnostics.push(CompileError::new(
                "SHTS001",
                "non-local or conditional ref projections require TypeScript checker facts",
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
        for handle in &handles {
            self.record_handle_use(handle, projection_name, self.source_span(expression.span()));
        }
        if handles.len() == 1 {
            Some(StyleInput::HandleProjection {
                handle: handles[0].clone(),
                projection,
            })
        } else {
            Some(StyleInput::DynamicHandleProjection {
                value: self.source_span(expression.span()),
                handles,
                projection,
            })
        }
    }

    fn collect_container_query_projection(
        &mut self,
        call: &CallExpression<'a>,
        member: &StaticMemberExpression<'a>,
    ) -> Option<StyleInput> {
        if call.arguments.len() != 2 {
            self.diagnostics.push(CompileError::new(
                "SHREF004",
                "ref.at() requires a literal query and a literal utility string",
                self.source_span(call.span),
            ));
            return None;
        }
        let Some(handle) = self.handle_reference(&member.object) else {
            self.diagnostics.push(CompileError::new(
                "SHREF001",
                "ref projections must use an identity returned by style.ref()",
                self.source_span(call.span),
            ));
            return None;
        };
        let (Expression::StringLiteral(query), Expression::StringLiteral(value)) = (
            call.arguments[0].to_expression(),
            call.arguments[1].to_expression(),
        ) else {
            self.diagnostics.push(CompileError::new(
                "SHREF004",
                "ref.at() requires a literal query and a literal utility string",
                self.source_span(call.span),
            ));
            return None;
        };
        self.record_handle_use(&handle, "container-query", self.source_span(call.span));
        Some(StyleInput::ContainerQuery {
            handle,
            query: self.string_literal(query),
            value: self.string_literal(value),
        })
    }

    fn record_handle_use(
        &mut self,
        handle: &HandleDefinition,
        projection: &'static str,
        span: Span,
    ) {
        self.handle_uses.push(HandleUse {
            name: handle.name.value.clone(),
            projection,
            span,
            jsx_path: (!self.jsx_path.is_empty()).then(|| self.jsx_path.clone()),
        });
    }

    fn record_scope_use(&mut self, scope: &ScopeDefinition) {
        self.scope_uses.push(ScopeUse {
            name: scope.name.value.clone(),
            jsx_path: (!self.jsx_path.is_empty()).then(|| self.jsx_path.clone()),
        });
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

    fn collect_scope_definition(
        &mut self,
        name: Spanned<String>,
        call: &CallExpression<'a>,
    ) -> Option<ScopeDefinition> {
        let label = match call.arguments.as_slice() {
            [] => None,
            [argument] => {
                if let Expression::StringLiteral(label) = argument.to_expression() {
                    Some(self.string_literal(label))
                } else {
                    self.diagnostics.push(CompileError::new(
                        "SHSCOPE001",
                        "style.scope() accepts an optional literal name",
                        self.source_span(argument.span()),
                    ));
                    return None;
                }
            }
            _ => {
                self.diagnostics.push(CompileError::new(
                    "SHSCOPE001",
                    "style.scope() accepts at most one literal name",
                    self.source_span(call.span),
                ));
                return None;
            }
        };
        if label.as_ref().is_some_and(|label| !is_marker_label(label)) {
            self.diagnostics.push(CompileError::new(
                "SHSCOPE001",
                "style.scope() names may contain only ASCII letters, digits, `_`, and `-`",
                label.expect("checked above").span,
            ));
            return None;
        }
        Some(ScopeDefinition {
            module_path: stillwind_source::normalized_module_path(self.source.path()),
            name,
            label,
            call_span: self.source_span(call.span),
        })
    }

    fn collect_ref_definition(
        &mut self,
        name: Spanned<String>,
        call: &CallExpression<'a>,
    ) -> Option<HandleDefinition> {
        let (scope, label) = match call.arguments.as_slice() {
            [] => (None, None),
            [argument] => match argument.to_expression() {
                Expression::StringLiteral(label) => (None, Some(self.string_literal(label))),
                expression => {
                    if let Some(scope) = self.scope_reference(expression) {
                        (Some(scope), None)
                    } else {
                        self.diagnostics.push(CompileError::new(
                            "SHREF001",
                            "style.ref() accepts an optional literal name or a scope identity",
                            self.source_span(argument.span()),
                        ));
                        return None;
                    }
                }
            },
            [scope, label] => {
                let Some(scope) = self.scope_reference(scope.to_expression()) else {
                    self.diagnostics.push(CompileError::new(
                        "SHREF001",
                        "the first style.ref() argument must be an identity returned by style.scope()",
                        self.source_span(scope.span()),
                    ));
                    return None;
                };
                let Expression::StringLiteral(label) = label.to_expression() else {
                    self.diagnostics.push(CompileError::new(
                        "SHREF001",
                        "the second style.ref() argument must be a literal name",
                        self.source_span(label.span()),
                    ));
                    return None;
                };
                (Some(scope), Some(self.string_literal(label)))
            }
            _ => {
                self.diagnostics.push(CompileError::new(
                    "SHREF001",
                    "style.ref() accepts (), (name), (scope), or (scope, name)",
                    self.source_span(call.span),
                ));
                return None;
            }
        };
        if label.as_ref().is_some_and(|label| !is_marker_label(label)) {
            self.diagnostics.push(CompileError::new(
                "SHREF001",
                "style.ref() names may contain only ASCII letters, digits, `_`, and `-`",
                label.expect("checked above").span,
            ));
            return None;
        }
        Some(HandleDefinition {
            module_path: stillwind_source::normalized_module_path(self.source.path()),
            name,
            label,
            scope,
            call_span: self.source_span(call.span),
        })
    }

    fn handle_tagged_selector(
        &mut self,
        key: &PropertyKey<'a>,
    ) -> Option<(Vec<HandleDefinition>, Spanned<String>)> {
        let PropertyKey::TaggedTemplateExpression(tagged) = key else {
            return None;
        };
        let Some(handles) = self.handle_references(&tagged.tag) else {
            self.diagnostics.push(CompileError::new(
                "SHTS001",
                "non-local or conditional ref selectors require TypeScript checker facts",
                self.source_span(tagged.tag.span()),
            ));
            return None;
        };
        if !tagged.quasi.expressions.is_empty() || tagged.quasi.quasis.len() != 1 {
            self.diagnostics.push(CompileError::new(
                "SHREF005",
                "a ref selector must be a template literal without substitutions",
                self.source_span(tagged.quasi.span),
            ));
            return None;
        }
        let Some(cooked) = tagged.quasi.quasis[0].value.cooked.as_ref() else {
            self.diagnostics.push(CompileError::new(
                "SHREF005",
                "a ref selector must be a valid static template literal",
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
            self.diagnostics.push(CompileError::new(
                "SHREF005",
                "a ref selector must start with `&`",
                selector.span,
            ));
            return None;
        }
        for handle in &handles {
            self.record_handle_use(handle, "selector", self.source_span(tagged.span));
        }
        Some((handles, selector))
    }

    fn collect_dynamic_handle_attachment(
        &mut self,
        expression: &Expression<'a>,
        inputs: &mut Vec<StyleInput>,
    ) -> bool {
        if let Some(scopes) = self.scope_references(expression) {
            for scope in &scopes {
                self.record_scope_use(scope);
            }
            if scopes.len() == 1 {
                inputs.push(StyleInput::ScopeAttachment(scopes[0].clone()));
            } else {
                inputs.push(StyleInput::DynamicScopeAttachment {
                    value: self.source_span(expression.span()),
                    scopes,
                });
            }
            return true;
        }
        if let Some(handles) = self.handle_references(expression) {
            for handle in &handles {
                self.record_handle_use(handle, "ref", self.source_span(expression.span()));
            }
            if handles.len() == 1 {
                inputs.push(StyleInput::HandleAttachment(handles[0].clone()));
            } else {
                inputs.push(StyleInput::DynamicHandleAttachment {
                    value: self.source_span(expression.span()),
                    handles,
                });
            }
            return true;
        }
        false
    }

    fn collect_style_expression(
        &mut self,
        expression: &Expression<'a>,
        inputs: &mut Vec<StyleInput>,
    ) {
        if self.collect_dynamic_handle_attachment(expression, inputs) {
            return;
        }
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
                    self.diagnostics.push(CompileError::new(
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
                    self.diagnostics.push(CompileError::new(
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
                            CompileError::new(
                                "SHJS002",
                                "style() arrays do not support spread elements",
                                self.source_span(spread.span),
                            ),
                        ),
                        ArrayExpressionElement::Elision(elision) => self.diagnostics.push(
                            CompileError::new(
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
            Expression::Identifier(_) => self.diagnostics.push(CompileError::new(
                "SHTS001",
                "non-local or conditional handles require TypeScript checker facts",
                self.source_span(expression.span()),
            )),
            _ => self.diagnostics.push(CompileError::new(
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
                self.diagnostics.push(CompileError::new(
                    "SHJS003",
                    "style() attribute objects do not support spread properties",
                    self.source_span(property.span()),
                ));
                continue;
            };
            if property.method || property.kind != PropertyKind::Init {
                self.diagnostics.push(CompileError::new(
                    "SHJS003",
                    "style() attributes must be plain key-value properties",
                    self.source_span(property.span),
                ));
                continue;
            }
            if property.computed {
                let Some((handles, selector)) = self.handle_tagged_selector(&property.key) else {
                    self.diagnostics.push(CompileError::new(
                        "SHJS003",
                        "computed style() keys must be static selectors tagged by style.ref()",
                        self.source_span(property.key.span()),
                    ));
                    continue;
                };
                match &property.value {
                    Expression::StringLiteral(value) => {
                        for handle in handles {
                            inputs.push(StyleInput::HandleSelectorUtilityString {
                                handle,
                                selector: selector.clone(),
                                value: self.string_literal(value),
                            });
                        }
                    }
                    Expression::LogicalExpression(expression)
                        if expression.operator == LogicalOperator::And =>
                    {
                        let Expression::StringLiteral(value) = &expression.right else {
                            self.diagnostics.push(CompileError::new(
                                "SHJS004",
                                "a ref selector conditional must have a literal utility string on its right side",
                                self.source_span(expression.right.span()),
                            ));
                            continue;
                        };
                        for handle in handles {
                            inputs.push(StyleInput::ConditionalHandleSelectorUtilityString {
                                handle,
                                selector: selector.clone(),
                                guard: self.source_span(expression.left.span()),
                                when_true: true,
                                value: self.string_literal(value),
                            });
                        }
                    }
                    _ => self.diagnostics.push(CompileError::new(
                        "SHJS004",
                        "a ref selector must map to a literal utility string or `condition && \"…\"`",
                        self.source_span(property.value.span()),
                    )),
                }
                continue;
            }
            if self
                .property_name(&property.key)
                .is_some_and(|name| name.value == "class")
            {
                inputs.push(StyleInput::ExternalClass(
                    self.source_span(property.value.span()),
                ));
                continue;
            }
            let PropertyKey::StringLiteral(name) = &property.key else {
                self.diagnostics.push(CompileError::new(
                    "SHJS003",
                    "style() object keys must be `class`, quoted aria-*/data-* attributes, or `&` selectors",
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
                self.diagnostics.push(CompileError::new(
                    "SHJS003",
                    "style() object keys must be `class`, quoted aria-*/data-* attributes, or `&` selectors",
                    name.span,
                ));
                continue;
            }
            match &property.value {
                Expression::CallExpression(_) => {
                    let Some(animation) = self.animation_use(&property.value) else {
                        self.diagnostics.push(CompileError::new(
                            "SHJS004",
                            "a `&` selector must map to a literal utility string, animation use, or `condition && \"…\"`",
                            self.source_span(property.value.span()),
                        ));
                        continue;
                    };
                    match animation {
                        StyleInput::AnimationUse { animation, options } => {
                            inputs.push(StyleInput::SelectorAnimationUse {
                                selector: name,
                                animation,
                                options,
                            });
                        }
                        StyleInput::DynamicAnimationUse {
                            value,
                            animations,
                            options,
                        } => inputs.push(StyleInput::DynamicSelectorAnimationUse {
                            selector: name,
                            value,
                            animations,
                            options,
                        }),
                        _ => unreachable!("animation_use returns an animation input"),
                    }
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
                        self.diagnostics.push(CompileError::new(
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
                _ => self.diagnostics.push(CompileError::new(
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
            .map(|handle_use| handle_use.name.clone())
            .collect::<HashSet<_>>();
        for name in names {
            let uses = self
                .handle_uses
                .iter()
                .filter(|handle_use| handle_use.name == name)
                .collect::<Vec<_>>();
            let is_scoped = self
                .handle_definitions
                .iter()
                .any(|definition| definition.name.value == name && definition.scope.is_some());
            let has_anchor = uses
                .iter()
                .any(|handle_use| handle_use.projection == "anchor");
            if !has_anchor {
                for handle_use in uses
                    .iter()
                    .filter(|handle_use| handle_use.projection == "position")
                {
                    self.diagnostics.push(CompileError::new(
                        "SHREF003",
                        format!("ref `{name}` uses position without an anchor projection"),
                        handle_use.span,
                    ));
                }
            }
            let has_container = uses
                .iter()
                .any(|handle_use| handle_use.projection == "container");
            for handle_use in uses
                .iter()
                .filter(|handle_use| handle_use.projection == "container-query" && !has_container)
            {
                self.diagnostics.push(CompileError::new(
                    "SHREF006",
                    format!("ref `{name}` uses a container query without a container projection"),
                    handle_use.span,
                ));
            }
            let known_anchor_paths = uses
                .iter()
                .filter_map(|handle_use| {
                    (handle_use.projection == "anchor" || handle_use.projection == "ref")
                        .then_some(handle_use.jsx_path.as_ref())
                        .flatten()
                })
                .collect::<Vec<_>>();
            for handle_use in uses
                .iter()
                .filter(|handle_use| handle_use.projection == "selector")
            {
                let Some(path) = &handle_use.jsx_path else {
                    continue;
                };
                if !is_scoped
                    && !known_anchor_paths.is_empty()
                    && !known_anchor_paths
                        .iter()
                        .any(|anchor_path| is_strict_ancestor(anchor_path, path))
                {
                    self.diagnostics.push(CompileError::new(
                        "SHREF007",
                        format!("ref `{name}` selector has no known ancestor ref in this JSX tree"),
                        handle_use.span,
                    ));
                }
            }
        }

        self.validate_scoped_refs();
    }

    fn validate_scoped_refs(&mut self) {
        for definition in self
            .handle_definitions
            .iter()
            .filter(|definition| definition.scope.is_some())
        {
            let scope = definition.scope.as_ref().expect("filtered above");
            let providers = self
                .scope_uses
                .iter()
                .filter(|use_| use_.name == scope.name.value)
                .collect::<Vec<_>>();
            if providers.is_empty() {
                self.diagnostics.push(CompileError::new(
                    "SHSCOPE002",
                    format!(
                        "ref `{}` is bound to scope `{}`, but that scope is not attached to a JSX element",
                        definition.name.value, scope.name.value
                    ),
                    definition.call_span,
                ));
                continue;
            }
            for reference_use in self.handle_uses.iter().filter(|use_| {
                use_.name == definition.name.value
                    && !matches!(use_.projection, "selector" | "container-query")
            }) {
                let Some(reference_path) = &reference_use.jsx_path else {
                    continue;
                };
                if !providers.iter().any(|provider| {
                    provider
                        .jsx_path
                        .as_ref()
                        .is_some_and(|path| is_strict_ancestor(path, reference_path))
                }) {
                    self.diagnostics.push(CompileError::new(
                        "SHSCOPE003",
                        format!(
                            "scope `{}` must be attached to an ancestor of ref `{}`",
                            scope.name.value, definition.name.value
                        ),
                        reference_use.span,
                    ));
                }
            }
            for selector_use in self
                .handle_uses
                .iter()
                .filter(|use_| use_.name == definition.name.value && use_.projection == "selector")
            {
                let Some(target_path) = &selector_use.jsx_path else {
                    continue;
                };
                if !providers.iter().any(|provider| {
                    provider
                        .jsx_path
                        .as_ref()
                        .is_some_and(|path| is_strict_ancestor(path, target_path))
                }) {
                    self.diagnostics.push(CompileError::new(
                        "SHSCOPE004",
                        format!(
                            "a selector using scoped ref `{}` must target an element inside scope `{}`",
                            definition.name.value, scope.name.value
                        ),
                        selector_use.span,
                    ));
                }
            }
        }
    }
}

fn is_strict_ancestor(ancestor: &[usize], descendant: &[usize]) -> bool {
    ancestor.len() < descendant.len() && descendant.starts_with(ancestor)
}

fn is_marker_label(label: &Spanned<String>) -> bool {
    !label.value.is_empty()
        && label
            .value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

impl<'a> Visit<'a> for MacroExtractor<'a> {
    fn visit_jsx_element(&mut self, element: &JSXElement<'a>) {
        let element_id = self.next_jsx_element;
        self.next_jsx_element += 1;
        self.jsx_path.push(element_id);
        walk::walk_jsx_element(self, element);
        self.jsx_path.pop();
    }

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
        if !Self::is_style_import(declaration)
            && let Some(specifiers) = &declaration.specifiers
        {
            for specifier in specifiers {
                let (local, imported) = match specifier {
                    ImportDeclarationSpecifier::ImportSpecifier(specifier) => (
                        specifier.local.name.to_string(),
                        Self::module_export_name(&specifier.imported),
                    ),
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                        (specifier.local.name.to_string(), "default".to_owned())
                    }
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                        (specifier.local.name.to_string(), "*".to_owned())
                    }
                };
                self.imports.push(ImportBinding {
                    local: local.clone(),
                    source: declaration.source.value.to_string(),
                    imported,
                });
            }
        }
        walk::walk_import_declaration(self, declaration);
    }

    fn visit_export_declaration(&mut self, declaration: &ExportDeclaration<'a>) {
        if let Declaration::VariableDeclaration(variables) = &declaration.declaration {
            for declarator in &variables.declarations {
                if let BindingPattern::BindingIdentifier(identifier) = &declarator.id
                    && let Some(symbol) = identifier.symbol_id.get()
                {
                    self.export_symbols
                        .push((symbol, identifier.name.to_string()));
                }
            }
        }
        walk::walk_export_declaration(self, declaration);
    }

    fn visit_export_named_declaration(&mut self, declaration: &ExportNamedDeclaration<'a>) {
        for specifier in &declaration.specifiers {
            let ModuleExportName::IdentifierReference(local) = &specifier.local else {
                continue;
            };
            let Some(reference) = local.reference_id.get() else {
                continue;
            };
            let Some(symbol) = self.scoping.get_reference(reference).symbol_id() else {
                continue;
            };
            self.export_symbols
                .push((symbol, Self::module_export_name(&specifier.exported)));
        }
        walk::walk_export_named_declaration(self, declaration);
    }

    fn visit_export_from_declaration(&mut self, declaration: &ExportFromDeclaration<'a>) {
        for specifier in &declaration.specifiers {
            self.re_exports.push(ReExport {
                exported: Self::module_export_name(&specifier.exported),
                source: declaration.source.value.to_string(),
                imported: Self::module_export_name(&specifier.local),
            });
        }
        walk::walk_export_from_declaration(self, declaration);
    }

    fn visit_export_all_declaration(&mut self, declaration: &ExportAllDeclaration<'a>) {
        self.re_exports.push(ReExport {
            exported: declaration
                .exported
                .as_ref()
                .map_or_else(|| "*".to_owned(), Self::module_export_name),
            source: declaration.source.value.to_string(),
            imported: "*".to_owned(),
        });
        walk::walk_export_all_declaration(self, declaration);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
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
                self.diagnostics.push(CompileError::new(
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
            && self.is_style_ref_call(call)
            && let Some(symbol_id) = identifier.symbol_id.get()
        {
            let name = Spanned::new(
                identifier.name.to_string(),
                self.source_span(identifier.span),
            );
            if let Some(definition) = self.collect_ref_definition(name, call) {
                self.handle_symbols.insert(symbol_id, definition.clone());
                let scopes = definition.scope.as_ref().map(|scope| {
                    SymbolicValue::exact(self.identity(AssetKind::Scope, &scope.name))
                });
                self.local_values.insert(
                    symbol_id,
                    ExportedValue::Refs(vec![
                        RefAlternative::new(
                            self.identity(AssetKind::Ref, &definition.name),
                            scopes,
                        )
                        .expect("extractor creates correctly typed ref relations"),
                    ]),
                );
                self.handle_definitions.push(definition);
            }
        }
        if let (
            BindingPattern::BindingIdentifier(identifier),
            Some(Expression::CallExpression(call)),
        ) = (&declarator.id, &declarator.init)
            && self.is_style_scope_call(call)
            && let Some(symbol_id) = identifier.symbol_id.get()
        {
            let name = Spanned::new(
                identifier.name.to_string(),
                self.source_span(identifier.span),
            );
            if let Some(definition) = self.collect_scope_definition(name, call) {
                self.scope_symbols.insert(symbol_id, definition.clone());
                self.local_values.insert(
                    symbol_id,
                    ExportedValue::Scopes(SymbolicValue::exact(
                        self.identity(AssetKind::Scope, &definition.name),
                    )),
                );
                self.scope_definitions.push(definition);
            }
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
                self.local_values.insert(
                    symbol_id,
                    ExportedValue::Animations(SymbolicValue::exact(
                        self.identity(AssetKind::Animation, &definition.name),
                    )),
                );
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
                self.local_values.insert(
                    symbol_id,
                    ExportedValue::ViewTransitions(SymbolicValue::exact(
                        self.identity(AssetKind::ViewTransition, &definition.name),
                    )),
                );
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
    use stillwind_source::FileId;

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
    fn parses_named_and_scoped_ref_declarations() {
        let source = SourceFile::new(
            FileId(1),
            "Menu.tsx",
            r#"import { style } from "stillwind/react";
const anonymousScope = style.scope();
const menu = style.scope("menu");
const anonymous = style.ref();
const named = style.ref("trigger");
const scoped = style.ref(menu);
const scopedNamed = style.ref(menu, "item");
export const Menu = () => <div {...style(menu)}><button {...style(scoped)} /></div>;"#,
        );

        let parsed = parse(&source);

        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        assert_eq!(parsed.module.scope_definitions.len(), 2);
        assert_eq!(parsed.module.scope_definitions[0].label, None);
        assert_eq!(
            parsed.module.scope_definitions[1]
                .label
                .as_ref()
                .map(|label| label.value.as_str()),
            Some("menu")
        );
        assert_eq!(parsed.module.handle_definitions.len(), 4);
        assert_eq!(parsed.module.handle_definitions[0].scope, None);
        assert_eq!(
            parsed.module.handle_definitions[1]
                .label
                .as_ref()
                .map(|label| label.value.as_str()),
            Some("trigger")
        );
        assert_eq!(
            parsed.module.handle_definitions[2]
                .scope
                .as_ref()
                .map(|scope| scope.name.value.as_str()),
            Some("menu")
        );
        assert_eq!(
            parsed.module.handle_definitions[3]
                .label
                .as_ref()
                .map(|label| label.value.as_str()),
            Some("item")
        );
    }

    #[test]
    fn summarizes_exported_assets_and_module_edges() {
        let source = SourceFile::new(
            FileId(1),
            r"src\Parent.tsx",
            r#"import { style } from "stillwind/react";
import { external as importedExternal } from "./external";
export const menu = style.scope("menu");
const trigger = style.ref(menu, "trigger");
export { trigger as menuTrigger };
export { sharedTransition as transition } from "./shared";
export const Parent = () => <div {...style(menu)}><button {...style(trigger)} /></div>;"#,
        );

        let parsed = parse(&source);

        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        assert_eq!(parsed.summary.module_path, "src/Parent.tsx");
        assert_eq!(parsed.summary.imports.len(), 1);
        assert_eq!(parsed.summary.imports[0].local, "importedExternal");
        assert!(matches!(
            parsed.summary.exports.get("menu"),
            Some(ExportedValue::Scopes(_))
        ));
        let Some(ExportedValue::Refs(alternatives)) = parsed.summary.exports.get("menuTrigger")
        else {
            panic!("menuTrigger should be an exported ref");
        };
        assert_eq!(alternatives.len(), 1);
        assert!(alternatives[0].permitted_scopes.is_some());
        assert_eq!(parsed.summary.re_exports[0].exported, "transition");
        assert_eq!(parsed.summary.re_exports[0].imported, "sharedTransition");
    }

    #[test]
    fn leaves_conditional_handle_propagation_to_typescript() {
        let source = SourceFile::new(
            FileId(1),
            "src/choices.tsx",
            r#"import { style } from "stillwind/react";
const first = style.ref("first");
const second = style.ref("second");
const alias = first;
export const selected = enabled ? alias : second;"#,
        );

        let parsed = parse(&source);
        assert!(!parsed.summary.exports.contains_key("selected"));
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
