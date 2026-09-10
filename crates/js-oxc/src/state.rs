use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use oxc::ast::ast::ModuleExportName;
use oxc::semantic::{Scoping, SymbolId};

use stillwind_ir::{
    AnimationDefinition, HandleDefinition, ScopeDefinition, StyleCall, ViewTransitionDefinition,
};
use stillwind_semantic::{
    AssetKind, AssetMetadata, AssetRecord, ExportedValue, IdentityId, ImportBinding,
    ModuleSpecifier, ModuleSummary, ReExport, TypeFact,
};
use stillwind_source::{CompileError, SourceFile, Spanned};

use crate::refs::{HandleUse, ScopeUse};

impl<'a> MacroExtractor<'a> {
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

    pub(crate) fn identity(&self, kind: AssetKind, name: &Spanned<String>) -> IdentityId {
        IdentityId::new(
            kind,
            self.source.path(),
            name.span.start.0,
            name.span.end.0,
            name.value.clone(),
        )
    }

    pub(crate) fn module_summary(&self) -> ModuleSummary {
        let mut summary = ModuleSummary::new(self.source.path());
        summary.imports.clone_from(&self.imports);
        summary
            .module_specifiers
            .clone_from(&self.module_specifiers);
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

    pub(crate) fn module_export_name(name: &ModuleExportName<'a>) -> String {
        match name {
            ModuleExportName::IdentifierName(name) => name.name.to_string(),
            ModuleExportName::IdentifierReference(name) => name.name.to_string(),
            ModuleExportName::StringLiteral(name) => name.value.to_string(),
        }
    }
}

pub(crate) struct MacroExtractor<'a> {
    pub(crate) adapter: Option<stillwind_ir::AdapterKind>,
    pub(crate) source: &'a SourceFile,
    pub(crate) scoping: &'a Scoping,
    pub(crate) style_symbols: HashSet<SymbolId>,
    pub(crate) handle_symbols: HashMap<SymbolId, HandleDefinition>,
    pub(crate) scope_symbols: HashMap<SymbolId, ScopeDefinition>,
    pub(crate) handle_uses: Vec<HandleUse>,
    pub(crate) scope_uses: Vec<ScopeUse>,
    pub(crate) handle_definitions: Vec<HandleDefinition>,
    pub(crate) scope_definitions: Vec<ScopeDefinition>,
    pub(crate) animation_symbols: HashMap<SymbolId, Spanned<String>>,
    pub(crate) animation_declarations: Vec<AnimationDefinition>,
    pub(crate) view_transition_symbols: HashMap<SymbolId, Spanned<String>>,
    pub(crate) view_transition_declarations: Vec<ViewTransitionDefinition>,
    pub(crate) local_values: HashMap<SymbolId, ExportedValue>,
    pub(crate) style_calls: Vec<StyleCall>,
    pub(crate) diagnostics: Vec<CompileError>,
    pub(crate) jsx_path: Vec<usize>,
    pub(crate) next_jsx_element: usize,
    pub(crate) imports: Vec<ImportBinding>,
    pub(crate) module_specifiers: Vec<ModuleSpecifier>,
    pub(crate) re_exports: Vec<ReExport>,
    pub(crate) export_symbols: Vec<(SymbolId, String)>,
    pub(crate) type_facts: HashMap<(u32, u32), ExportedValue>,
    pub(crate) type_probes: RefCell<BTreeSet<(u32, u32)>>,
    pub(crate) imported_assets: &'a BTreeMap<IdentityId, AssetMetadata>,
}
