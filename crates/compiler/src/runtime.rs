//! Helper allocation and import placement from structured code-generation uses.
//!
//! Lowering records each use of a framework runtime helper together with the span it
//! belongs to; [`RuntimeBindings`] turns those uses into local names and an import edit
//! plan. Three contracts matter here. A helper import is emitted into the macro import
//! statement that is being removed at that point in the file, so a use whose import
//! realm has no verifiable macro anchor fails the plan instead of guessing. Local names
//! avoid every reserved name the frontend reported and are allocated deterministically.
//! Replacements that land inside a projected attribute are entity-encoded, because a
//! raw quote would end the attribute early.

use std::collections::{BTreeMap, BTreeSet};

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_diagnostics::{CompileDiagnostic, CompileError, SourceFailureContext};
use stillwind_ir::{
    AttributeQuote, ModuleAnalysis, OutputFormat, ProjectionRegion, RUNTIME_IMPORT,
    RUNTIME_IMPORT_DEV,
};
use stillwind_source::{JoinText, ReplaceText, Span};

use crate::{IdentifierMode, SourceTransform, plan_macro_import_removals};

/// One compiler helper exported by the runtime.
///
/// The development runtime exports every helper under the same name, and its versions of
/// [`Self::Clsx`], [`Self::Cva`], [`Self::Merge`], [`Self::ToFragment`], and
/// [`Self::ViewTransitionName`] validate their inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum RuntimeHelper {
    /// The fragment key for class text of one output format.
    ClassKey,
    /// The `stillwindClsx` export.
    Clsx,
    /// The `stillwindCssValue` export.
    CssValue,
    /// The fragment key for generated custom properties of one output format.
    CustomPropertiesKey,
    /// Selects compiled CVA values and materializes props of one output format.
    Cva,
    /// Materializes fragments as props of one output format.
    Merge,
    /// The `stillwindRefClassChecked` export, which only the development runtime
    /// provides.
    RefClassChecked,
    /// The fragment key for authored style payloads of one output format.
    StyleKey,
    /// Converts a style payload for one output format.
    StyleValue,
    /// Converts a composition base to a fragment of one output format.
    ToFragment,
    /// The `stillwindViewTransitionName` export.
    ViewTransitionName,
}

impl RuntimeHelper {
    /// Returns the output format whose export this helper uses, or `None` when every
    /// output format shares one export.
    fn scope(self, output: OutputFormat) -> Option<OutputFormat> {
        match self {
            Self::Clsx | Self::CssValue | Self::RefClassChecked | Self::ViewTransitionName => None,
            Self::ClassKey
            | Self::CustomPropertiesKey
            | Self::Cva
            | Self::Merge
            | Self::StyleKey
            | Self::StyleValue
            | Self::ToFragment => Some(output),
        }
    }

    /// Returns the export name in the runtime for a scope from [`Self::scope`].
    ///
    /// A format-specific export name starts with the format's
    /// [`runtime_export_prefix`](stillwind_ir::OutputSpec::runtime_export_prefix).
    fn exported(self, scope: Option<OutputFormat>) -> LeanString {
        let name = match self {
            Self::ClassKey => "ClassKey",
            Self::Clsx => "stillwindClsx",
            Self::CssValue => "stillwindCssValue",
            Self::CustomPropertiesKey => "CustomPropertiesKey",
            Self::Cva => "Cva",
            Self::Merge => "Merge",
            Self::RefClassChecked => "stillwindRefClassChecked",
            Self::StyleKey => "StyleKey",
            Self::StyleValue => "StyleValue",
            Self::ToFragment => "ToFragment",
            Self::ViewTransitionName => "stillwindViewTransitionName",
        };
        match scope {
            Some(output) => {
                format_args!("{}{name}", output.spec().runtime_export_prefix).to_lean_string()
            }
            None => name.to_lean_string(),
        }
    }
}

/// One helper binding: its scope from [`RuntimeHelper::scope`], the helper, and the
/// import realm of its uses.
type HelperKey = (Option<OutputFormat>, RuntimeHelper, Option<u32>);

/// Allocates local helper names and plans their imports for one module.
///
/// Uses are keyed by scope, helper, and import realm, so a module that renders in
/// several realms (a Svelte markup region and its script, for example) gets one binding
/// per realm and never one binding per use. Output formats share the bindings of helpers
/// that do not depend on the format.
pub(crate) struct RuntimeBindings<'a> {
    /// Projection regions of the module, which decide the import realm of each use.
    regions: &'a [ProjectionRegion],
    /// Names already bound in the module; never handed out as a local helper name.
    reserved: BTreeSet<LeanStr>,
    /// One local name per helper binding, allocated on first use.
    names: BTreeMap<HelperKey, LeanStr>,
    /// Every recorded use with its binding, in the order it was requested.
    uses: Vec<(HelperKey, Span)>,
    /// Output format of the call currently being rendered.
    output: OutputFormat,
    /// Whether the compilation runs in development mode.
    development: bool,
}

impl<'a> RuntimeBindings<'a> {
    /// Starts an empty binding set for `module`.
    ///
    /// The module's projection regions decide the import realm of each later use, and
    /// its reserved names are never handed out.
    pub(crate) fn new(module: &'a ModuleAnalysis) -> Self {
        Self {
            regions: module
                .input
                .projection()
                .map_or(&[], |projection| projection.editable_regions()),
            reserved: module.ir.imports.reserved_names.iter().cloned().collect(),
            names: BTreeMap::new(),
            uses: Vec::new(),
            output: OutputFormat::ClassNameObject,
            development: false,
        }
    }

    /// Returns the same bindings for a compilation in `mode`.
    ///
    /// The mode selects the module that helpers are imported from. The generated calls
    /// and export names are the same in both modes.
    pub(crate) fn in_mode(mut self, mode: IdentifierMode) -> Self {
        self.development = mode == IdentifierMode::Development;
        self
    }

    /// Returns whether this compilation requests explicit runtime validation.
    pub(crate) fn is_development(&self) -> bool {
        self.development
    }

    /// Selects the output format for helpers requested by the next call.
    pub(crate) fn select_output(&mut self, output: OutputFormat) {
        self.output = output;
    }

    /// Records a use of `helper` at `at` and returns the local name to call it with.
    ///
    /// Repeated uses of one helper binding share a single name and a single import. The
    /// name is the export name unless it collides with a reserved name, in which case a
    /// numeric suffix is appended until it is free. Allocation is deterministic for a
    /// fixed use order. All bindings share one reserved set.
    pub(crate) fn require(&mut self, helper: RuntimeHelper, at: Span) -> LeanStr {
        let realm = self
            .regions
            .iter()
            .find(|region| contains(region.source, at))
            .map(|region| region.import_realm);
        let scope = helper.scope(self.output);
        let key = (scope, helper, realm);
        self.uses.push((key, at));
        self.names
            .entry(key)
            .or_insert_with(|| {
                let base = helper.exported(scope);
                let mut name = base.clone();
                let mut suffix = 0_u64;
                while !self.reserved.insert((name.clone()).into()) {
                    suffix += 1;
                    name = format_args!("{base}_{suffix}").to_lean_string();
                }
                (name).into()
            })
            .clone()
    }

    /// Builds one import statement for the helper bindings anchored at one macro import.
    fn import_statement(&self, helpers: &BTreeSet<HelperKey>) -> LeanStr {
        let names = helpers
            .iter()
            .map(|key| {
                let exported = key.1.exported(key.0);
                match self.names.get(key) {
                    Some(local) if **local != *exported => {
                        format_args!("{exported} as {local}").to_lean_string()
                    }
                    _ => exported,
                }
            })
            .collect::<Vec<_>>()
            .join_text(", ");
        let runtime_module = if self.development {
            RUNTIME_IMPORT_DEV
        } else {
            RUNTIME_IMPORT
        };
        format_args!("import {{ {names} }} from \"{runtime_module}\";").to_lean_str()
    }

    /// Returns the import edits and macro-removal edits for the recorded uses.
    ///
    /// Each binding is imported once, in the replacement of the macro import that
    /// anchors its first use, so imports appear next to the code that used the macro. An
    /// anchor is a macro import that this compilation removes or rewrites; when a use
    /// has none, the plan fails rather than dropping the import.
    ///
    /// # Errors
    ///
    /// Returns [`CompileError`] for a missing or ambiguous realm anchor, a missing
    /// module-CSS target, or metadata that cannot be sliced from the source.
    pub(crate) fn plan(
        &self,
        module: &ModuleAnalysis,
        lowered: &[SourceTransform],
    ) -> Result<Vec<SourceTransform>, CompileError> {
        let mut edits = plan_macro_import_removals(&module.source, &module.ir.imports, lowered)?;
        let mut anchored = BTreeSet::new();
        let mut anchors: BTreeMap<usize, BTreeSet<HelperKey>> = BTreeMap::new();
        for &(key, at) in &self.uses {
            if anchored.insert(key) {
                let anchor = runtime_anchor(module, at, &edits)?;
                anchors.entry(anchor).or_default().insert(key);
            }
        }
        for (anchor, helpers) in anchors {
            let span = module.ir.imports.macros[anchor].span;
            let import = self.import_statement(&helpers);
            if let Some(edit) = edits.iter_mut().find(|edit| edit.call_span == span) {
                if !edit.replacement.is_empty() {
                    edit.replacement.push('\n');
                }
                edit.replacement.push_str(&import);
            } else {
                // A selected anchor must have contained a successfully lowered macro.
                return Err(CompileError::new(
                    CompileDiagnostic::MissingAnchorEdit { span },
                    span,
                ));
            }
        }
        for &span in &module.ir.imports.module_css {
            module.source.slice(span).map_err(|error| {
                CompileError::new(
                    CompileDiagnostic::source_failure(&error, SourceFailureContext::MacroImport),
                    module.source.full_span(),
                )
            })?;
            let target = module.input.module_css_import.as_ref().ok_or_else(|| {
                CompileError::new(CompileDiagnostic::MissingModuleCssTarget { span }, span)
            })?;
            let quoted = serde_json::to_string(target).map_err(|_| {
                CompileError::new(CompileDiagnostic::InvalidMacroImportMetadata { span }, span)
            })?;
            edits.push(SourceTransform {
                call_span: span,
                replacement: (quoted.replace_char('<', "\\u003c")),
                expressions: Vec::new(),
            });
        }
        if module.ir.imports.module_css.is_empty() && !module.input.stylesheet_imports.is_empty() {
            let anchor = module
                .ir
                .imports
                .macros
                .first()
                .ok_or_else(|| {
                    CompileError::new(
                        CompileDiagnostic::MissingRuntimeAnchor {
                            span: module.source.full_span(),
                        },
                        module.source.full_span(),
                    )
                })?
                .span;
            let edit = edits
                .iter_mut()
                .find(|edit| edit.call_span == anchor)
                .ok_or_else(|| {
                    CompileError::new(
                        CompileDiagnostic::MissingAnchorEdit { span: anchor },
                        anchor,
                    )
                })?;
            for specifier in &module.input.stylesheet_imports {
                edit.replacement.push_str("\nimport ");
                let quoted = serde_json::to_string(specifier).map_err(|_| {
                    CompileError::new(
                        CompileDiagnostic::InvalidMacroImportMetadata { span: anchor },
                        anchor,
                    )
                })?;
                edit.replacement
                    .push_str(&quoted.replace_char('<', "\\u003c"));
                edit.replacement.push(';');
            }
        }
        Ok(edits)
    }
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.file == inner.file && outer.start <= inner.start && inner.end <= outer.end
}

/// Finds the macro import statement whose replacement receives the import for `at`.
///
/// The anchor is the first macro import with an entry in `edits` in `at`'s import realm,
/// following parent realms when a realm contains none. Direct input has one realm.
/// Returns [`CompileDiagnostic::EditOutsideEditableRegion`] when no editable region
/// contains `at`, and [`CompileDiagnostic::MissingRuntimeAnchor`] when no realm in its
/// chain contains such an import.
fn runtime_anchor(
    module: &ModuleAnalysis,
    at: Span,
    edits: &[SourceTransform],
) -> Result<usize, CompileError> {
    let edited = |span: Span| edits.iter().any(|edit| edit.call_span == span);
    let Some(projection) = module.input.projection() else {
        return module
            .ir
            .imports
            .macros
            .iter()
            .position(|import| edited(import.span))
            .ok_or_else(|| {
                CompileError::new(CompileDiagnostic::MissingRuntimeAnchor { span: at }, at)
            });
    };
    let region = projection_region(module, at)?.ok_or_else(|| {
        CompileError::new(
            CompileDiagnostic::EditOutsideEditableRegion { span: at },
            at,
        )
    })?;
    for realm in projection.realm_chain(region.import_realm) {
        if let Some(anchor) = module
            .ir
            .imports
            .macros
            .iter()
            .position(|import| edited(import.span) && contains(realm.source, import.span))
        {
            return Ok(anchor);
        }
    }
    Err(CompileError::new(
        CompileDiagnostic::MissingRuntimeAnchor { span: at },
        at,
    ))
}

/// Returns the editable region containing `at`, or `None` for direct input.
///
/// # Errors
///
/// Returns [`CompileDiagnostic::EditOutsideEditableRegion`] when projected input has no
/// region containing `at`.
fn projection_region(
    module: &ModuleAnalysis,
    at: Span,
) -> Result<Option<&ProjectionRegion>, CompileError> {
    let Some(projection) = module.input.projection() else {
        return Ok(None);
    };
    let region = projection
        .editable_regions()
        .iter()
        .find(|region| contains(region.source, at))
        .ok_or_else(|| {
            CompileError::new(
                CompileDiagnostic::EditOutsideEditableRegion { span: at },
                at,
            )
        })?;
    Ok(Some(region))
}

/// Escapes quote characters in replacements that land inside a projected attribute.
///
/// A projected region is authored markup, so a generated replacement that contains the
/// attribute's own quote character would terminate it. The entity spelling is used
/// instead, and the surrounding characters are verified so a bad region is reported
/// rather than producing broken output.
///
/// Source expression intervals are rebased through entity expansion; encoded characters
/// map to their original scalar and unaffected text retains its exact correspondence.
/// Edits outside quoted regions are left unchanged.
///
/// # Errors
///
/// Returns [`CompileDiagnostic::EditOutsideEditableRegion`] when no region contains an
/// edit. Returns [`CompileDiagnostic::AttributeQuoteMismatch`] when a quoted region is
/// not delimited by its recorded quote.
/// Earlier edits may already be encoded; the caller discards the plan on failure.
///
/// # Panics
///
/// Panics when an edit carries expression ranges that are not ordered, in bounds, and
/// scalar-aligned within its replacement, because encoding those ranges slices the
/// replacement directly.
///
/// `edits` must therefore be the transforms this pass's own compilation lowered -
/// [`RuntimeBindings::plan`] and the fragment renderer build them together with their
/// replacement text. Do not re-enter this pass with transforms carried over from
/// another compilation or reconstructed from host metadata.
pub(crate) fn encode_projected_edits(
    module: &ModuleAnalysis,
    edits: &mut [SourceTransform],
) -> Result<(), CompileError> {
    for edit in edits {
        let Some(region) = projection_region(module, edit.call_span)? else {
            continue;
        };
        let Some(quote) = region.attribute_quote else {
            continue;
        };
        let (character, entity) = match quote {
            AttributeQuote::Single => ('\'', "&#39;"),
            AttributeQuote::Double => ('"', "&quot;"),
        };
        let start = region.source.start.0 as usize;
        let end = region.source.end.0 as usize;
        let text = module.source.text().as_bytes();
        if start == 0
            || text.get(start - 1) != Some(&(character as u8))
            || text.get(end) != Some(&(character as u8))
        {
            return Err(CompileError::new(
                CompileDiagnostic::AttributeQuoteMismatch {
                    span: region.source,
                },
                region.source,
            ));
        }
        edit.encode_attribute(character, entity);
    }
    Ok(())
}

#[cfg(test)]
mod runtime_anchor_tests {
    use std::sync::Arc;

    use stillwind_ir::{
        ConverterIdentity, ImportRealm, ModuleAnalysis, PreparedSource, ProjectedSource,
        ProjectionRegion, ScriptLanguage, ScriptModuleKind, ScriptSyntax, SourceAnalyzer,
        SourceDocument, SourceFormat, SourceInput, SourceInputError, SourceProjection,
    };
    use stillwind_js_oxc::OxcSourceAnalyzer;
    use stillwind_source::{
        FileId, ModuleId, ProjectionMap, SourceFile, SourceProvenance, VolarMapping,
    };

    use super::*;

    fn analysis(text: &str) -> ModuleAnalysis {
        let source = SourceFile::new(FileId(1), None, None, text).unwrap();
        OxcSourceAnalyzer.finalize(
            OxcSourceAnalyzer.scan(
                Arc::new(source),
                ModuleId::from("test.tsx"),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous,
            ),
            None,
        )
    }

    /// Returns removal edits for the macro imports at `indices`.
    fn removals(module: &ModuleAnalysis, indices: &[usize]) -> Vec<SourceTransform> {
        indices
            .iter()
            .map(|&index| SourceTransform {
                call_span: module.ir.imports.macros[index].span,
                replacement: LeanString::new(),
                expressions: Vec::new(),
            })
            .collect()
    }

    /// A module without a projection anchors on its first edited macro import, whatever
    /// its output format, and a module without one is reported instead of guessing.
    #[test]
    fn runtime_anchor_uses_the_first_edited_macro_import_without_a_projection() {
        let module = analysis(
            "import { style } from 'stillwind/react'; import { style as v } from 'stillwind/vue'; style('flex'); v('flex');",
        );
        let at = module.source.full_span();
        assert_eq!(
            runtime_anchor(&module, at, &removals(&module, &[0, 1])).unwrap(),
            0
        );
        assert_eq!(
            runtime_anchor(&module, at, &removals(&module, &[1])).unwrap(),
            1
        );
        assert!(matches!(
            runtime_anchor(&module, at, &[]),
            Err(CompileError {
                diagnostic: CompileDiagnostic::MissingRuntimeAnchor { .. },
                ..
            })
        ));
        let module = analysis("export const x = 1;");
        assert!(matches!(
            runtime_anchor(&module, module.source.full_span(), &[]),
            Err(CompileError {
                diagnostic: CompileDiagnostic::MissingRuntimeAnchor { .. },
                ..
            })
        ));
    }

    fn projected_input(
        realms: Vec<ImportRealm>,
        regions: Vec<ProjectionRegion>,
    ) -> Result<SourceInput, SourceInputError> {
        let text = "import { style } from 'stillwind/vue'; const x = style('flex');";
        let document = SourceDocument::new(
            Arc::new(SourceFile::new(FileId(1), None, None, text).unwrap()),
            SourceFormat::Vue,
            None,
        );
        let code = Arc::new(SourceFile::new(FileId(2), None, None, text).unwrap());
        let projection = SourceProjection::try_new(
            code,
            ScriptSyntax::new(ScriptLanguage::Tsx, ScriptModuleKind::Unambiguous),
            ProjectionMap::Volar {
                mappings: vec![VolarMapping {
                    source_offsets: vec![0],
                    generated_offsets: vec![0],
                    lengths: vec![u32::try_from(text.len()).unwrap()],
                    generated_lengths: None,
                }],
            },
            ConverterIdentity {
                name: "test-converter".to_lean_str(),
                version: "0".to_lean_str(),
                options_fingerprint: "none".to_lean_str(),
            },
            FileId(3),
            realms,
            regions,
        )?;
        let projected = ProjectedSource::try_new(document, projection)?;
        Ok(SourceInput::new(
            ModuleId::from("test.vue"),
            PreparedSource::Projected(projected),
        ))
    }

    fn projected(realms: Vec<ImportRealm>, regions: Vec<ProjectionRegion>) -> ModuleAnalysis {
        let analysis = OxcSourceAnalyzer.finalize(
            OxcSourceAnalyzer.scan_input(projected_input(realms, regions).unwrap()),
            None,
        );
        assert!(
            analysis.diagnostics.is_empty(),
            "{:?}",
            analysis.diagnostics
        );
        assert_eq!(analysis.source.id(), FileId(1));
        assert_eq!(analysis.input.parser_source().id(), FileId(2));
        assert_eq!(
            analysis.input.projection().unwrap().analysis_id(),
            FileId(3)
        );
        assert_ne!(analysis.provenance, [] as [SourceProvenance; 0]);
        assert!(analysis.provenance.iter().all(|entry| {
            entry.original.file == FileId(1) && entry.projected.file == FileId(2)
        }));
        analysis
    }

    fn region(import_realm: u32, start: u32, end: u32) -> ProjectionRegion {
        ProjectionRegion {
            source: Span::new(FileId(1), start, end),
            import_realm,
            attribute_quote: None,
        }
    }

    fn realm(id: u32, start: u32, end: u32, parent: Option<u32>) -> ImportRealm {
        ImportRealm {
            id,
            source: Span::new(FileId(1), start, end),
            parent,
        }
    }

    /// A realm without a macro import of its own anchors on the enclosing realm that
    /// owns one, and a chain that ends without one is reported instead.
    #[test]
    fn runtime_anchor_follows_the_realm_chain_outward() {
        let at = Span::new(FileId(1), 50, 55);
        let nested = projected(
            vec![realm(1, 45, 60, Some(2)), realm(2, 0, 44, None)],
            vec![region(1, 45, 60)],
        );
        assert_eq!(
            runtime_anchor(&nested, at, &removals(&nested, &[0])).unwrap(),
            0
        );
        let orphan = projected(vec![realm(1, 45, 60, None)], vec![region(1, 45, 60)]);
        assert!(matches!(
            runtime_anchor(&orphan, at, &removals(&orphan, &[0])),
            Err(CompileError {
                diagnostic: CompileDiagnostic::MissingRuntimeAnchor { .. },
                ..
            })
        ));
    }

    #[test]
    fn projected_input_rejects_invalid_realm_metadata() {
        assert!(matches!(
            projected_input(
                vec![realm(1, 45, 60, Some(2)), realm(2, 45, 60, Some(1))],
                vec![region(1, 45, 60)],
            ),
            Err(SourceInputError::ImportRealmCycle { .. })
        ));
        assert!(matches!(
            projected_input(
                vec![realm(1, 0, 44, None), realm(1, 0, 44, None)],
                vec![region(1, 50, 55)],
            ),
            Err(SourceInputError::DuplicateImportRealm { .. })
        ));
        assert!(matches!(
            projected_input(vec![realm(1, 45, 60, Some(2))], vec![region(1, 45, 60)]),
            Err(SourceInputError::MissingImportRealmParent { .. })
        ));
        assert!(matches!(
            projected_input(vec![realm(1, 45, 60, None)], vec![region(2, 45, 60)]),
            Err(SourceInputError::MissingRegionImportRealm { .. })
        ));
    }

    #[test]
    fn projected_input_rejects_invalid_spans_and_overlapping_regions() {
        assert!(matches!(
            projected_input(vec![realm(1, 0, 100, None)], vec![region(1, 45, 60)]),
            Err(SourceInputError::InvalidImportRealmSpan { .. })
        ));
        assert!(matches!(
            projected_input(vec![realm(1, 0, 60, None)], vec![region(1, 45, 100)]),
            Err(SourceInputError::InvalidEditableRegionSpan { .. })
        ));
        assert!(matches!(
            projected_input(
                vec![realm(1, 45, 60, None)],
                vec![region(1, 45, 60), region(1, 50, 55)],
            ),
            Err(SourceInputError::OverlappingEditableRegions { .. })
        ));
    }

    #[test]
    fn runtime_anchor_reports_queries_outside_regions() {
        let single = projected(vec![realm(1, 45, 60, None)], vec![region(1, 45, 60)]);
        assert!(matches!(
            runtime_anchor(
                &single,
                single.ir.style_calls[0].span,
                &removals(&single, &[0])
            ),
            Err(CompileError {
                diagnostic: CompileDiagnostic::EditOutsideEditableRegion { .. },
                ..
            })
        ));
    }
}
