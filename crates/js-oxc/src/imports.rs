//! Import edits use binding identity, never a generated-text name search.
//!
//! [`collect`] turns one program into [`ModuleImports`]: the macro import declarations
//! with their specifiers and reference spans, the `module-css` literals of imports,
//! re-exports and dynamic imports, and every local name the module binds or reads.
//! The reserved name set exists so a host can rename without colliding.
//!
//! References are attributed through Oxc symbol ids rather than by matching text, so a
//! shadowed or aliased binding cannot be mistaken for an import. Spans point at the
//! original source; reference lists are filled only for specifiers whose
//! [`MacroImportSpecifier::is_macro`] flag is set.

use std::collections::{BTreeSet, HashMap, HashSet};

use lean_string::{LeanStr, ToLeanStr};
use oxc::ast::ast::{
    BindingIdentifier, CallExpression, ChainElement, Comment, ExportAllDeclaration,
    ExportFromDeclaration, Expression, IdentifierReference, ImportDeclaration,
    ImportDeclarationSpecifier, ImportExpression, ModuleExportName, Program, StringLiteral,
};
use oxc::ast_visit::{Visit, walk};
use oxc::semantic::{Scoping, SymbolId};
use oxc::span::{GetSpan, Span as OxcSpan};

use stillwind_ir::{
    LeadingRead, MODULE_CSS_IMPORT, MacroImport, MacroImportSpecifier, ModuleImports,
    from_macro_import,
};
use stillwind_source::{FileId, Span};

/// Collects the module's macro imports, module-css literals and reserved names.
///
/// The returned [`ModuleImports::reserved_names`] include every binding and identifier
/// reference in the module, so a generated name that is not in the set is free to use.
pub(crate) fn collect(program: &Program<'_>, scoping: &Scoping, file: FileId) -> ModuleImports {
    let mut collector = Collector {
        scoping,
        file,
        text: program.source_text,
        comments: &program.comments,
        names: BTreeSet::new(),
        imports: Vec::new(),
        module_css: Vec::new(),
        references: HashMap::new(),
        symbols: Vec::new(),
        leading_reads: Vec::new(),
    };
    collector.visit_program(program);
    let mut macro_symbols = HashSet::new();
    for (import, symbols) in collector.imports.iter_mut().zip(collector.symbols) {
        for (specifier, symbol) in import.specifiers.iter_mut().zip(symbols) {
            if specifier.is_macro
                && let Some(symbol) = symbol
            {
                specifier.references = collector.references.remove(&symbol).unwrap_or_default();
                macro_symbols.insert(symbol);
            }
        }
    }
    let leading_reads = collector
        .leading_reads
        .into_iter()
        .filter_map(|(read, symbol)| macro_symbols.contains(&symbol).then_some(read))
        .collect();
    ModuleImports {
        consumed_uses: Vec::new(),
        leading_reads,
        reserved_names: collector.names.into_iter().collect(),
        macros: collector.imports,
        module_css: collector.module_css,
    }
}

/// Visitor state for one `collect` call.
struct Collector<'s> {
    /// Semantic scoping table used to resolve reference ids to symbols.
    scoping: &'s Scoping,
    /// File id stamped onto every produced span.
    file: FileId,
    /// Every local binding and referenced name seen in the module.
    names: BTreeSet<LeanStr>,
    /// Macro import declarations in source order.
    imports: Vec<MacroImport>,
    /// Source literal spans of value imports, re-exports and dynamic imports of
    /// `stillwind/module-css`.
    module_css: Vec<Span>,
    /// Symbol of each import specifier, parallel to [`Self::imports`].
    symbols: Vec<Vec<Option<SymbolId>>>,
    /// Reference spans per symbol, used to fill macro specifier references.
    references: HashMap<SymbolId, Vec<Span>>,
    /// Source text of the program, which Oxc spans address.
    text: &'s str,
    /// Comments of the program in source order, used to skip their text between
    /// import specifiers.
    comments: &'s [Comment],
    /// Calls whose first token reads a resolved binding, with the symbol of that
    /// binding. [`collect`] keeps the reads of macro bindings.
    leading_reads: Vec<(LeadingRead, SymbolId)>,
}

impl Collector<'_> {
    /// Stamps an Oxc span with this collector's file id.
    fn span(&self, span: OxcSpan) -> Span {
        Span::new(self.file, span.start, span.end)
    }

    /// Records `source` as a module-css literal when it names the module-css entry
    /// point.
    ///
    /// The caller must pass `is_value = false` when the declaration loads no runtime
    /// value, because a type-only declaration is erased and its literal must stay
    /// unchanged.
    fn record_module_css(&mut self, source: &StringLiteral<'_>, is_value: bool) {
        if is_value && source.value == MODULE_CSS_IMPORT {
            self.module_css.push(self.span(source.span));
        }
    }

    /// Returns the `,` that joins two bindings of one import list within `gap`.
    ///
    /// `gap` must lie between two adjacent specifiers, or between the last specifier
    /// and the end of the declaration, so that it contains only punctuation, whitespace,
    /// and comments. Commas inside comments do not count. A `{` means that the gap
    /// leaves the named-specifier list, so the result is `None`. The search stops at
    /// `}`, which closes the list.
    fn list_comma(&self, gap: OxcSpan) -> Option<OxcSpan> {
        let mut comma = None;
        let mut offset = gap.start;
        while offset < gap.end {
            if let Some(comment) = self
                .comments
                .iter()
                .find(|comment| comment.span.start <= offset && offset < comment.span.end)
            {
                offset = comment.span.end;
                continue;
            }
            match self.text.as_bytes()[offset as usize] {
                b',' => comma = Some(OxcSpan::new(offset, offset + 1)),
                b'{' => return None,
                b'}' => return comma,
                _ => {}
            }
            offset += 1;
        }
        comma
    }
}

/// Returns the identifier that is the first token of `expression`, looking through
/// parentheses, member accesses, calls, tagged templates, TypeScript wrappers, and the
/// left operands of operators.
fn leading_identifier<'e, 'a>(
    expression: &'e Expression<'a>,
) -> Option<&'e IdentifierReference<'a>> {
    match expression {
        Expression::Identifier(identifier) => Some(identifier),
        Expression::ParenthesizedExpression(expression) => {
            leading_identifier(&expression.expression)
        }
        Expression::StaticMemberExpression(member) => leading_identifier(&member.object),
        Expression::ComputedMemberExpression(member) => leading_identifier(&member.object),
        Expression::PrivateFieldExpression(member) => leading_identifier(&member.object),
        Expression::CallExpression(call) => leading_identifier(&call.callee),
        Expression::TaggedTemplateExpression(tagged) => leading_identifier(&tagged.tag),
        Expression::ChainExpression(chain) => match &chain.expression {
            ChainElement::CallExpression(call) => leading_identifier(&call.callee),
            ChainElement::TSNonNullExpression(expression) => {
                leading_identifier(&expression.expression)
            }
            ChainElement::StaticMemberExpression(member) => leading_identifier(&member.object),
            ChainElement::ComputedMemberExpression(member) => leading_identifier(&member.object),
            ChainElement::PrivateFieldExpression(member) => leading_identifier(&member.object),
        },
        Expression::TSAsExpression(expression) => leading_identifier(&expression.expression),
        Expression::TSSatisfiesExpression(expression) => leading_identifier(&expression.expression),
        Expression::TSNonNullExpression(expression) => leading_identifier(&expression.expression),
        Expression::TSInstantiationExpression(expression) => {
            leading_identifier(&expression.expression)
        }
        Expression::BinaryExpression(expression) => leading_identifier(&expression.left),
        Expression::LogicalExpression(expression) => leading_identifier(&expression.left),
        Expression::ConditionalExpression(expression) => leading_identifier(&expression.test),
        Expression::SequenceExpression(expression) => {
            expression.expressions.first().and_then(leading_identifier)
        }
        _ => None,
    }
}

impl<'a> Visit<'a> for Collector<'_> {
    /// Records a bound name so generated names can avoid it.
    fn visit_binding_identifier(&mut self, identifier: &BindingIdentifier<'a>) {
        self.names.insert(identifier.name.to_lean_str());
    }

    /// Records a read name and attributes the reference to its symbol.
    ///
    /// Unresolved references contribute their name to the reserved set but no span, so
    /// a reference to a global never looks like an import use.
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.names.insert(identifier.name.to_lean_str());
        if let Some(reference) = identifier.reference_id.get()
            && let Some(symbol) = self.scoping.get_reference(reference).symbol_id()
        {
            let span = self.span(identifier.span);
            self.references.entry(symbol).or_default().push(span);
        }
    }

    /// Records module-css literals and macro imports in one pass.
    ///
    /// A `module-css` import counts only when it is a value import with at least one
    /// non-type specifier (or no specifiers at all); a macro import is kept only when
    /// one of its specifiers is the `style` macro, so an unrelated import from the same
    /// entry point is ignored.
    fn visit_import_declaration(&mut self, declaration: &ImportDeclaration<'a>) {
        self.record_module_css(
            &declaration.source,
            !declaration.import_kind.is_type()
                && declaration.specifiers.as_ref().is_none_or(|specifiers| {
                    specifiers.is_empty()
                        || specifiers.iter().any(|specifier| {
                            !matches!(specifier, ImportDeclarationSpecifier::ImportSpecifier(specifier) if specifier.import_kind.is_type())
                        })
                }),
        );
        if let Some(adapter) = declaration
            .source
            .value
            .as_str()
            .and_then(from_macro_import)
            && !declaration.import_kind.is_type()
            && let Some(specifiers) = &declaration.specifiers
        {
            let mut bindings = Vec::new();
            let mut symbols = Vec::new();
            for specifier in specifiers {
                let (local, is_macro) = match specifier {
                    ImportDeclarationSpecifier::ImportSpecifier(specifier) => (
                        &specifier.local,
                        !specifier.import_kind.is_type()
                            && matches!(&specifier.imported, ModuleExportName::IdentifierName(name) if name.name == "style"),
                    ),
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                        (&specifier.local, false)
                    }
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                        (&specifier.local, false)
                    }
                };
                bindings.push(MacroImportSpecifier {
                    span: self.span(specifier.span()),
                    local: local.name.to_lean_str(),
                    is_macro,
                    has_unmapped_references: false,
                    references: Vec::new(),
                    separator: None,
                });
                symbols.push(local.symbol_id.get());
            }
            for index in 0..specifiers.len() {
                let end = specifiers[index].span().end;
                let next = specifiers
                    .get(index + 1)
                    .map_or(declaration.span.end, |next| next.span().start);
                let separator = self.list_comma(OxcSpan::new(end, next)).or_else(|| {
                    let previous = specifiers.get(index.checked_sub(1)?)?.span().end;
                    self.list_comma(OxcSpan::new(previous, specifiers[index].span().start))
                });
                bindings[index].separator = separator.map(|comma| self.span(comma));
            }
            if bindings.iter().any(|binding| binding.is_macro) {
                self.imports.push(MacroImport {
                    adapter: adapter.id,
                    source_span: self.span(declaration.source.span),
                    span: self.span(declaration.span),
                    specifiers: bindings,
                });
                self.symbols.push(symbols);
            }
        }
        walk::walk_import_declaration(self, declaration);
    }

    /// Records a module-css literal in `export { ... } from`.
    ///
    /// The declaration counts under the same rule as an import: it must not be
    /// `export type`, and it must have no specifiers or at least one value specifier.
    fn visit_export_from_declaration(&mut self, declaration: &ExportFromDeclaration<'a>) {
        self.record_module_css(
            &declaration.source,
            !declaration.export_kind.is_type()
                && (declaration.specifiers.is_empty()
                    || declaration
                        .specifiers
                        .iter()
                        .any(|specifier| !specifier.export_kind.is_type())),
        );
        walk::walk_export_from_declaration(self, declaration);
    }

    /// Records a module-css literal in `export * from` and `export * as name from`.
    fn visit_export_all_declaration(&mut self, declaration: &ExportAllDeclaration<'a>) {
        self.record_module_css(&declaration.source, !declaration.export_kind.is_type());
        walk::walk_export_all_declaration(self, declaration);
    }

    /// Records a call whose first token reads a resolved binding.
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let Some(identifier) = leading_identifier(&call.callee)
            && let Some(reference) = identifier.reference_id.get()
            && let Some(symbol) = self.scoping.get_reference(reference).symbol_id()
        {
            self.leading_reads.push((
                LeadingRead {
                    call: self.span(call.span),
                    reference: self.span(identifier.span),
                },
                symbol,
            ));
        }
        walk::walk_call_expression(self, call);
    }

    /// Records a module-css literal in a dynamic `import()` whose source is a string
    /// literal.
    fn visit_import_expression(&mut self, expression: &ImportExpression<'a>) {
        if let Expression::StringLiteral(source) = &expression.source {
            self.record_module_css(source, true);
        }
        walk::walk_import_expression(self, expression);
    }
}
