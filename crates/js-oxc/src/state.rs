//! The extractor's working state and the summary it publishes.
//!
//! [`MacroExtractor`] is one pass over one module: the semantic scoping table, the
//! symbols it recognized as macro bindings, the definitions and uses it found, and the
//! diagnostics it produced. [`MacroExtractor::module_summary`] turns that state into
//! the per-module facts other modules see.
//!
//! Symbol maps are the reason a shadowed local binding cannot be mistaken for the
//! macro. Identity is created in [`MacroExtractor::identity`], which always anchors to
//! the declaration's own span in this module.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use oxc::ast::ast::ModuleExportName;
use oxc::semantic::{Scoping, SymbolId};

use stillwind_diagnostics::{CompileError, LintFinding};
use stillwind_ir::{
    AdapterKind, KeyframesDefinition, RefDefinition, StyleCall, ViewTransitionDefinition,
    ViewTransitionKeyApplication,
};
use stillwind_semantic::{
    AssetKind, AssetMetadata, AssetRecord, ExportedValue, IdentityId, ImportBinding,
    ModuleSpecifier, ModuleSummary, ReExport, TypeFact,
};
use stillwind_source::{SourceFile, Spanned};

use crate::refs::RefUse;

impl<'a> MacroExtractor<'a> {
    /// Builds an extractor for one parsed module.
    ///
    /// `type_facts` and `imported_assets` come from the host checker and are looked up
    /// by span; passing empty maps is the normal no-checker path.
    pub(crate) fn new(
        source: &'a SourceFile,
        scoping: &'a Scoping,
        type_facts: &[TypeFact],
        imported_assets: &'a BTreeMap<IdentityId, AssetMetadata>,
    ) -> Self {
        Self {
            source,
            scoping,
            style_symbols: HashSet::new(),
            adapter: None,
            ref_symbols: HashMap::new(),
            ref_uses: Vec::new(),
            ref_definitions: Vec::new(),
            keyframes_symbols: HashMap::new(),
            keyframes_declarations: Vec::new(),
            view_transition_symbols: HashMap::new(),
            view_transition_declarations: Vec::new(),
            view_transition_key_applications: Vec::new(),
            view_transition_key_symbols: HashMap::new(),
            local_values: HashMap::new(),
            style_calls: Vec::new(),
            diagnostics: Vec::new(),
            lint_findings: Vec::new(),
            jsx_path: Vec::new(),
            next_jsx_element: 0,
            imports: Vec::new(),
            module_specifiers: Vec::new(),
            re_exports: Vec::new(),
            export_symbols: Vec::new(),
            type_facts: type_facts
                .iter()
                .map(|fact| ((fact.start, fact.end), fact.value.clone()))
                .collect(),
            imported_assets,
            type_probes: RefCell::default(),
        }
    }

    /// Creates the declaration-site identity of an asset declared in this module.
    ///
    /// The span is the declaration's name, so uses elsewhere resolve back to this
    /// declaration rather than to their own position.
    pub(crate) fn identity(&self, kind: AssetKind, name: &Spanned<String>) -> IdentityId {
        IdentityId::new(
            kind,
            self.source.path(),
            name.span.start.0,
            name.span.end.0,
            name.value.clone(),
        )
    }

    /// Builds the module summary from the collected state.
    ///
    /// Assets are appended in kind order (styles, refs, keyframes, view transitions),
    /// not source order. Exports are keyed by exported name and only contain bindings
    /// whose value resolved to a symbolic form; a binding that was reassigned or chosen
    /// conditionally is deliberately absent so the host checker can answer instead of
    /// this pass guessing.
    pub(crate) fn module_summary(&self) -> ModuleSummary {
        let mut summary = ModuleSummary::new(self.source.path());
        summary.imports.clone_from(&self.imports);
        summary
            .module_specifiers
            .clone_from(&self.module_specifiers);
        summary.re_exports.clone_from(&self.re_exports);

        for call in &self.style_calls {
            summary.assets.push(AssetRecord {
                identity: IdentityId::new(
                    AssetKind::Style,
                    self.source.path(),
                    call.span.start.0,
                    call.span.end.0,
                    "",
                ),
                metadata: AssetMetadata { label: None },
                call_start: call.span.start.0,
                call_end: call.span.end.0,
            });
        }
        for definition in &self.ref_definitions {
            summary.assets.push(AssetRecord {
                identity: self.identity(AssetKind::Ref, &definition.binding),
                metadata: AssetMetadata {
                    label: definition.label.as_ref().map(|label| label.value.clone()),
                },
                call_start: definition.call_span.start.0,
                call_end: definition.call_span.end.0,
            });
        }
        for definition in &self.keyframes_declarations {
            summary.assets.push(AssetRecord {
                identity: self.identity(AssetKind::Keyframes, &definition.binding),
                metadata: AssetMetadata { label: None },
                call_start: definition.call_span.start.0,
                call_end: definition.call_span.end.0,
            });
        }
        for definition in &self.view_transition_declarations {
            summary.assets.push(AssetRecord {
                identity: self.identity(AssetKind::ViewTransition, &definition.binding),
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

    /// Returns the exported name of a module export name node.
    ///
    /// String-literal export names are supported, so `export { x as "y-z" }` keeps the
    /// spelling users import.
    pub(crate) fn module_export_name(name: &ModuleExportName<'a>) -> String {
        match name {
            ModuleExportName::IdentifierName(name) => name.name.to_string(),
            ModuleExportName::IdentifierReference(name) => name.name.to_string(),
            ModuleExportName::StringLiteral(name) => name.value.to_string(),
        }
    }
}

/// Per-module extraction state.
///
/// The fields are filled by the visitors in the sibling modules and consumed by
/// [`MacroExtractor::module_summary`]; nothing here is serialized directly.
pub(crate) struct MacroExtractor<'a> {
    /// Adapter selected from a macro import, once one was seen.
    pub(crate) adapter: Option<AdapterKind>,
    /// Source being analysed; every produced span is tagged with its id.
    pub(crate) source: &'a SourceFile,
    /// Oxc scoping table used to resolve references to symbols.
    pub(crate) scoping: &'a Scoping,
    /// Symbols bound to the imported `style` macro.
    pub(crate) style_symbols: HashSet<SymbolId>,
    /// Symbols bound to a `style.ref()` declaration, with their definitions.
    pub(crate) ref_symbols: HashMap<SymbolId, RefDefinition>,
    /// Every ref use, in source order, used for ancestry validation. An attachment is
    /// also what makes a ref available as the boundary of a bounded ref.
    pub(crate) ref_uses: Vec<RefUse>,
    /// Ref definitions in declaration order.
    pub(crate) ref_definitions: Vec<RefDefinition>,
    /// Symbols bound to an keyframes declaration, with their names.
    pub(crate) keyframes_symbols: HashMap<SymbolId, Spanned<String>>,
    /// Animation declarations in declaration order.
    pub(crate) keyframes_declarations: Vec<KeyframesDefinition>,
    /// Symbols bound to a view-transition declaration, with their names.
    pub(crate) view_transition_symbols: HashMap<SymbolId, Spanned<String>>,
    /// View-transition declarations in declaration order.
    pub(crate) view_transition_declarations: Vec<ViewTransitionDefinition>,
    pub(crate) view_transition_key_applications: Vec<ViewTransitionKeyApplication>,
    /// Bindings whose value is a transition that already carries its key, by symbol.
    pub(crate) view_transition_key_symbols: HashMap<SymbolId, Vec<IdentityId>>,
    /// Symbolic value of each local binding the pass could resolve.
    pub(crate) local_values: HashMap<SymbolId, ExportedValue>,
    /// Style call sites in source order.
    pub(crate) style_calls: Vec<StyleCall>,
    /// Diagnostics raised during extraction, in visit order.
    pub(crate) diagnostics: Vec<CompileError>,
    /// Advisory findings raised during extraction, in visit order.
    pub(crate) lint_findings: Vec<LintFinding>,
    /// Element indices of the JSX ancestors of the node being visited.
    pub(crate) jsx_path: Vec<usize>,
    /// Counter that gives each visited JSX element a stable path index.
    pub(crate) next_jsx_element: usize,
    /// Import bindings of this module, in source order.
    pub(crate) imports: Vec<ImportBinding>,
    /// Module specifier literals of this module, in source order.
    pub(crate) module_specifiers: Vec<ModuleSpecifier>,
    /// Re-export edges of this module, in source order.
    pub(crate) re_exports: Vec<ReExport>,
    /// Exported bindings as symbol plus exported name, in visit order.
    pub(crate) export_symbols: Vec<(SymbolId, String)>,
    /// Checker facts keyed by expression span.
    pub(crate) type_facts: HashMap<(u32, u32), ExportedValue>,
    /// Spans the checker should resolve, accumulated during the visit.
    pub(crate) type_probes: RefCell<BTreeSet<(u32, u32)>>,
    /// Metadata for identities declared in other modules.
    pub(crate) imported_assets: &'a BTreeMap<IdentityId, AssetMetadata>,
}
