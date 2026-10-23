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

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap};

use lean_string::{LeanStr, ToLeanStr};
use oxc::ast::ast::ModuleExportName;
use oxc::semantic::{Scoping, SymbolId};
use oxc::span::Span as OxcSpan;

use stillwind_diagnostics::{CompileError, LintFinding};
use stillwind_ir::{
    CssCall, CvaCall, CvaReplacement, KeyframesDefinition, MediaCall, OutputFormat, RefDefinition,
    StyleCall, ViewTransitionDefinition,
};
use stillwind_semantic::{
    AssetKind, AssetMetadata, AssetRecord, CheckedValue, ExportedValue, IdentityId, ImportBinding,
    ModuleSpecifier, ModuleSummary, ReExport, TypeFact,
};
use stillwind_source::{ModuleId, SourceFile, Span, Spanned};

use crate::local_span::LocalSpan;

/// Per-module extraction state.
///
/// The fields are filled by the visitors in the sibling modules and consumed by
/// [`MacroExtractor::module_summary`]; nothing here is serialized directly.
pub(crate) struct MacroExtractor<'a> {
    /// Identity used for declarations originating in this module.
    pub(crate) module_id: &'a ModuleId,
    /// Analysis buffer for Oxc spans; declaration identities use `original`
    /// coordinates.
    pub(crate) source: &'a SourceFile,
    /// Editable original source, including the complete SFC for projected input.
    pub(crate) original: &'a SourceFile,
    /// Analysis UTF-8 spans to original UTF-8 spans; synthetic spans have no mapping.
    pub(crate) coordinates: &'a dyn Fn(Span) -> Option<Span>,
    /// Oxc scoping table used to resolve references to symbols.
    pub(crate) scoping: &'a Scoping,
    /// Output format selected by each imported macro binding.
    pub(crate) style_symbols: HashMap<SymbolId, OutputFormat>,
    /// Symbols bound to a `style.ref()` declaration, with their definitions.
    pub(crate) ref_symbols: HashMap<SymbolId, RefDefinition>,
    /// Marker and provider uses in source order, matched within each style call.
    /// Ref definitions in declaration order.
    pub(crate) ref_definitions: Vec<RefDefinition>,
    /// Symbols bound to an keyframes declaration, with their names.
    pub(crate) keyframes_symbols: HashMap<SymbolId, Spanned<LeanStr>>,
    /// Animation declarations in declaration order.
    pub(crate) keyframes_declarations: Vec<KeyframesDefinition>,
    /// Ranges in the AST buffer that have already been handled as macro uses.
    /// Parsers add ranges even when their arguments are invalid. Validation also
    /// adds callee and tag ranges to prevent duplicate diagnostics. A recorded
    /// range therefore does not guarantee successful lowering.
    pub(crate) consumed_use_spans: BTreeSet<LocalSpan>,
    /// Symbols bound to a view-transition declaration, with their names.
    pub(crate) view_transition_symbols: HashMap<SymbolId, Spanned<LeanStr>>,
    /// View-transition declarations in declaration order.
    pub(crate) view_transition_declarations: Vec<ViewTransitionDefinition>,
    /// Symbolic value of each local binding the pass could resolve.
    pub(crate) local_values: HashMap<SymbolId, ExportedValue>,
    /// Style call sites in source order.
    pub(crate) style_calls: Vec<StyleCall>,
    /// CVA factory calls whose values use the ordinary style-input parser.
    pub(crate) cva_calls: Vec<CvaCall>,
    /// `style.media()` calls in source order.
    pub(crate) media_calls: Vec<MediaCall>,
    /// `style.css()` calls in source order.
    pub(crate) css_calls: Vec<CssCall>,
    /// Allows boolean literals as empty contributions while parsing CVA values.
    pub(crate) in_cva: bool,
    /// Diagnostics raised during extraction, in visit order.
    pub(crate) diagnostics: Vec<CompileError>,
    /// Lint findings raised during extraction, in visit order. Reporting settings never
    /// change the IR or the omitted inputs.
    pub(crate) lint_findings: Vec<LintFinding>,
    /// Source replacements that turn unsupported CVA entries into empty entries, in
    /// visit order.
    pub(crate) cva_replacements: Vec<CvaReplacement>,
    /// Whether extraction collects lint directives and findings that do not affect
    /// output. Findings for omitted inputs, fatal diagnostics, and IR extraction stay
    /// enabled.
    pub(crate) lint: bool,
    /// Import bindings of this module, in source order.
    pub(crate) imports: Vec<ImportBinding>,
    /// Ranges of parsed scope-only calls, used when removing their macro imports.
    /// They refer to the AST buffer until mapped back to the original source.
    pub(crate) consumed_macro_uses: Vec<Span>,
    /// Inputs whose source expressions are erased, including their nested macro reads.
    pub(crate) omitted_inputs: Vec<Span>,
    /// Module specifier literals of this module, in source order.
    pub(crate) module_specifiers: Vec<ModuleSpecifier>,
    /// Re-export edges of this module, in source order.
    pub(crate) re_exports: Vec<ReExport>,
    /// Exported bindings as symbol plus exported name, in visit order.
    pub(crate) export_symbols: Vec<(SymbolId, LeanStr)>,
    /// Checker facts keyed by original UTF-8 expression range.
    pub(crate) type_facts: HashMap<LocalSpan, CheckedValue>,
    /// AST-buffer ranges the checker should resolve, accumulated during the visit.
    pub(crate) type_probes: RefCell<BTreeSet<LocalSpan>>,
    /// AST-buffer receiver ranges used to identify Stillwind assets; missing facts
    /// leave the receiver as ordinary JavaScript without a diagnostic.
    pub(crate) use_probes: RefCell<BTreeSet<LocalSpan>>,
    /// Makes receiver probes optional while checking whether a use is a macro.
    /// Must be false outside `use_receiver_is` so probes needed for lowering stay
    /// subject to the required-query policy.
    pub(crate) probing_use: Cell<bool>,
    /// Metadata for identities declared in other modules.
    pub(crate) imported_assets: &'a BTreeMap<IdentityId, AssetMetadata>,
}

impl<'a> MacroExtractor<'a> {
    /// Builds an extractor for one parsed module.
    ///
    /// `type_facts` and `imported_assets` come from the host checker and are looked up
    /// by original UTF-8 span; passing empty maps is the normal no-checker path.
    pub(crate) fn new(
        source: &'a SourceFile,
        module_id: &'a ModuleId,
        scoping: &'a Scoping,
        type_facts: &[TypeFact],
        imported_assets: &'a BTreeMap<IdentityId, AssetMetadata>,
        original: &'a SourceFile,
        coordinates: &'a dyn Fn(Span) -> Option<Span>,
    ) -> Self {
        Self {
            source,
            module_id,
            original,
            coordinates,
            scoping,
            style_symbols: HashMap::new(),
            ref_symbols: HashMap::new(),
            ref_definitions: Vec::new(),
            keyframes_symbols: HashMap::new(),
            keyframes_declarations: Vec::new(),
            consumed_use_spans: BTreeSet::new(),
            view_transition_symbols: HashMap::new(),
            view_transition_declarations: Vec::new(),
            local_values: HashMap::new(),
            style_calls: Vec::new(),
            cva_calls: Vec::new(),
            media_calls: Vec::new(),
            css_calls: Vec::new(),
            in_cva: false,
            diagnostics: Vec::new(),
            lint_findings: Vec::new(),
            cva_replacements: Vec::new(),
            lint: true,
            imports: Vec::new(),
            consumed_macro_uses: Vec::new(),
            omitted_inputs: Vec::new(),
            module_specifiers: Vec::new(),
            re_exports: Vec::new(),
            export_symbols: Vec::new(),
            type_facts: type_facts
                .iter()
                .map(|fact| (LocalSpan::new(fact.start, fact.end), fact.value.clone()))
                .collect(),
            imported_assets,
            type_probes: RefCell::default(),
            use_probes: RefCell::default(),
            probing_use: Cell::new(false),
        }
    }

    /// Records that the source expression at `span` is erased from generated code.
    /// Nested macro reads inside it are erased with it.
    pub(crate) fn omit_input(&mut self, span: OxcSpan) {
        self.omitted_inputs.push(self.source_span(span));
    }

    /// Returns whether `span` lies inside an expression recorded by
    /// [`Self::omit_input`].
    pub(crate) fn input_is_omitted(&self, span: OxcSpan) -> bool {
        let span = self.source_span(span);
        self.omitted_inputs.iter().any(|omitted| {
            omitted.file == span.file && omitted.start <= span.start && span.end <= omitted.end
        })
    }

    /// Looks up a fact by its original UTF-8 expression range.
    pub(crate) fn type_fact(&self, span: OxcSpan) -> Option<&CheckedValue> {
        let span = (self.coordinates)(self.source_span(span))?;
        self.type_facts
            .get(&LocalSpan::new(span.start.0, span.end.0))
    }

    /// Creates the declaration-site identity of an asset declared in this module.
    ///
    /// The span is the declaration's name, so uses elsewhere resolve back to this
    /// declaration rather than to their own position.
    pub(crate) fn identity(&self, kind: AssetKind, name: &Spanned<LeanStr>) -> IdentityId {
        let span = (self.coordinates)(name.span).unwrap_or(self.original.full_span());
        IdentityId::new(
            kind,
            self.module_id.clone(),
            span.start.0,
            span.end.0,
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
        let mut summary = ModuleSummary::new(self.module_id.clone());
        summary.imports.clone_from(&self.imports);
        summary
            .module_specifiers
            .clone_from(&self.module_specifiers);
        summary.re_exports.clone_from(&self.re_exports);

        for call in &self.style_calls {
            let span = (self.coordinates)(call.span).unwrap_or(self.original.full_span());
            summary.assets.push(AssetRecord {
                identity: IdentityId::new(
                    AssetKind::Style,
                    self.module_id.clone(),
                    span.start.0,
                    span.end.0,
                    "",
                ),
                metadata: AssetMetadata { label: None },
                call_start: call.span.start.0,
                call_end: call.span.end.0,
            });
        }
        for definition in &self.ref_definitions {
            self.push_asset(
                &mut summary,
                AssetKind::Ref,
                &definition.binding,
                definition.label.as_ref(),
                definition.call_span,
            );
        }
        for definition in &self.keyframes_declarations {
            self.push_asset(
                &mut summary,
                AssetKind::Keyframes,
                &definition.binding,
                definition.label.as_ref(),
                definition.call_span,
            );
        }
        for definition in &self.view_transition_declarations {
            self.push_asset(
                &mut summary,
                AssetKind::ViewTransition,
                &definition.binding,
                definition.label.as_ref(),
                definition.call_span,
            );
        }

        for (symbol, exported) in &self.export_symbols {
            if let Some(value) = self.local_values.get(symbol) {
                summary.exports.insert(exported.clone(), value.clone());
            }
        }
        summary
    }

    /// Appends one declaration's asset record, carrying the label the producer
    /// published.
    ///
    /// The label is part of the declaration's generated name, so it travels with the
    /// record to the modules that reference the declaration.
    fn push_asset(
        &self,
        summary: &mut ModuleSummary,
        kind: AssetKind,
        binding: &Spanned<LeanStr>,
        label: Option<&Spanned<LeanStr>>,
        call_span: Span,
    ) {
        summary.assets.push(AssetRecord {
            identity: self.identity(kind, binding),
            metadata: AssetMetadata {
                label: label.map(|label| label.value.clone()),
            },
            call_start: call_span.start.0,
            call_end: call_span.end.0,
        });
    }

    /// Returns the exported name of a module export name node.
    ///
    /// String-literal export names are supported, so `export { x as "y-z" }` keeps the
    /// spelling users import.
    pub(crate) fn module_export_name(name: &ModuleExportName<'a>) -> LeanStr {
        match name {
            ModuleExportName::IdentifierName(name) => name.name.to_lean_str(),
            ModuleExportName::IdentifierReference(name) => name.name.to_lean_str(),
            ModuleExportName::StringLiteral(name) => name.value.to_lean_str(),
        }
    }
}
