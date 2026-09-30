//! Import edits use binding identity, never a generated-text name search.
//!
//! [`collect`] turns one program into [`ModuleImports`]: the macro import declarations
//! with their specifiers and reference spans, the `module-css` import literals, and
//! every local name the module binds or reads. The reserved name set exists so a host
//! can rename without colliding.
//!
//! References are attributed through Oxc symbol ids rather than by matching text, so a
//! shadowed or aliased binding cannot be mistaken for an import. Spans point at the
//! original source; reference lists are filled only for specifiers whose
//! [`MacroImportSpecifier::is_macro`] flag is set.

use std::collections::{BTreeSet, HashMap};

use oxc::ast::ast::{
    BindingIdentifier, IdentifierReference, ImportDeclaration, ImportDeclarationSpecifier,
    ModuleExportName, Program,
};
use oxc::ast_visit::{Visit, walk};
use oxc::semantic::{Scoping, SymbolId};
use oxc::span::GetSpan;

use stillwind_ir::{
    MODULE_CSS_IMPORT, MacroImport, MacroImportSpecifier, ModuleImports, from_macro_import,
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
        names: BTreeSet::new(),
        imports: Vec::new(),
        module_css: Vec::new(),
        references: HashMap::new(),
        symbols: Vec::new(),
    };
    collector.visit_program(program);
    for (import, symbols) in collector.imports.iter_mut().zip(collector.symbols) {
        for (specifier, symbol) in import.specifiers.iter_mut().zip(symbols) {
            if specifier.is_macro
                && let Some(symbol) = symbol
            {
                specifier.references = collector.references.remove(&symbol).unwrap_or_default();
            }
        }
    }
    ModuleImports {
        consumed_uses: Vec::new(),
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
    names: BTreeSet<String>,
    /// Macro import declarations in source order.
    imports: Vec<MacroImport>,
    /// Spans of value imports of `stillwind/module-css`.
    module_css: Vec<Span>,
    /// Symbol of each import specifier, parallel to [`Self::imports`].
    symbols: Vec<Vec<Option<SymbolId>>>,
    /// Reference spans per symbol, used to fill macro specifier references.
    references: HashMap<SymbolId, Vec<Span>>,
}

impl Collector<'_> {
    /// Stamps an Oxc span with this collector's file id.
    fn span(&self, span: oxc::span::Span) -> Span {
        Span::new(self.file, span.start, span.end)
    }
}

impl<'a> Visit<'a> for Collector<'_> {
    /// Records a bound name so generated names can avoid it.
    fn visit_binding_identifier(&mut self, identifier: &BindingIdentifier<'a>) {
        self.names.insert(identifier.name.to_string());
    }

    /// Records a read name and attributes the reference to its symbol.
    ///
    /// Unresolved references contribute their name to the reserved set but no span, so
    /// a reference to a global never looks like an import use.
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.names.insert(identifier.name.to_string());
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
        if declaration.source.value == MODULE_CSS_IMPORT && !declaration.import_kind.is_type()
            && declaration.specifiers.as_ref().is_none_or(|specifiers| {
                specifiers.is_empty() || specifiers.iter().any(|specifier| {
                    !matches!(specifier, ImportDeclarationSpecifier::ImportSpecifier(specifier) if specifier.import_kind.is_type())
                })
            })
        {
            self.module_css.push(self.span(declaration.source.span));
        }
        if from_macro_import(declaration.source.value.as_str()).is_some()
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
                    local: local.name.to_string(),
                    is_macro,
                    has_unmapped_references: false,
                    references: Vec::new(),
                });
                symbols.push(local.symbol_id.get());
            }
            if bindings.iter().any(|binding| binding.is_macro) {
                self.imports.push(MacroImport {
                    span: self.span(declaration.span),
                    specifiers: bindings,
                });
                self.symbols.push(symbols);
            }
        }
        walk::walk_import_declaration(self, declaration);
    }
}
