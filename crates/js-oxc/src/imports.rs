//! Import edits use binding identity, never a generated-text name search.
use std::collections::{BTreeSet, HashMap};

use oxc::ast::ast::{
    BindingIdentifier, IdentifierReference, ImportDeclaration, ImportDeclarationSpecifier,
    ModuleExportName, Program,
};
use oxc::ast_visit::{Visit, walk};
use oxc::semantic::{Scoping, SymbolId};
use oxc::span::GetSpan;

use stillwind_ir::{MacroImport, MacroImportSpecifier, ModuleImports};
use stillwind_source::{FileId, Span};

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
        reserved_names: collector.names.into_iter().collect(),
        macros: collector.imports,
        module_css: collector.module_css,
    }
}

struct Collector<'s> {
    scoping: &'s Scoping,
    file: FileId,
    names: BTreeSet<String>,
    imports: Vec<MacroImport>,
    module_css: Vec<Span>,
    symbols: Vec<Vec<Option<SymbolId>>>,
    references: HashMap<SymbolId, Vec<Span>>,
}

impl Collector<'_> {
    fn span(&self, span: oxc::span::Span) -> Span {
        Span::new(self.file, span.start, span.end)
    }
}

impl<'a> Visit<'a> for Collector<'_> {
    fn visit_binding_identifier(&mut self, identifier: &BindingIdentifier<'a>) {
        self.names.insert(identifier.name.to_string());
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.names.insert(identifier.name.to_string());
        if let Some(reference) = identifier.reference_id.get()
            && let Some(symbol) = self.scoping.get_reference(reference).symbol_id()
        {
            let span = self.span(identifier.span);
            self.references.entry(symbol).or_default().push(span);
        }
    }

    fn visit_import_declaration(&mut self, declaration: &ImportDeclaration<'a>) {
        if declaration.source.value == stillwind_ir::framework::MODULE_CSS_IMPORT && !declaration.import_kind.is_type()
            && declaration.specifiers.as_ref().is_none_or(|specifiers| {
                specifiers.is_empty() || specifiers.iter().any(|specifier| {
                    !matches!(specifier, ImportDeclarationSpecifier::ImportSpecifier(specifier) if specifier.import_kind.is_type())
                })
            })
        {
            self.module_css.push(self.span(declaration.source.span));
        }
        if stillwind_ir::framework::from_macro_import(declaration.source.value.as_str()).is_some()
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
