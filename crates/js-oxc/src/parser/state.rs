use super::{
    AssetKind, AssetMetadata, AssetRecord, BTreeMap, HandleDefinition, HashMap, HashSet,
    IdentityId, MacroExtractor, ModuleExportName, ModuleSummary, RefAlternative, RefCell,
    ScopeDefinition, Scoping, SourceFile, Span, Spanned, TypeFact,
};

impl<'a> MacroExtractor<'a> {
    pub(super) fn new(
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
            type_probes: RefCell::default(),
        }
    }

    pub(super) fn imported_scope(&self, identity: &IdentityId) -> ScopeDefinition {
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

    pub(super) fn imported_ref(&self, alternative: &RefAlternative) -> Option<HandleDefinition> {
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

    pub(super) fn identity(&self, kind: AssetKind, name: &Spanned<String>) -> IdentityId {
        IdentityId::new(
            kind,
            self.source.path(),
            name.span.start.0,
            name.span.end.0,
            name.value.clone(),
        )
    }

    pub(super) fn module_summary(&self) -> ModuleSummary {
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

    pub(super) fn module_export_name(name: &ModuleExportName<'a>) -> String {
        match name {
            ModuleExportName::IdentifierName(name) => name.name.to_string(),
            ModuleExportName::IdentifierReference(name) => name.name.to_string(),
            ModuleExportName::StringLiteral(name) => name.value.to_string(),
        }
    }
}
