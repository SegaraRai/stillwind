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

use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_ir::{AttributeQuote, ModuleAnalysis, ProjectionRegion};
use stillwind_source::Span;

use crate::{SourceTransform, plan_macro_import_removals};

/// One exported helper of a per-framework runtime module.
///
/// The set mirrors the exports of [`stillwind_ir::FrameworkSpec::runtime_import`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum RuntimeHelper {
    /// The `stillwindAnimation` runtime export.
    Animation,
    /// The `stillwindAssetClass` runtime export.
    AssetClass,
    /// The `stillwindClassKey` runtime export.
    ClassKey,
    /// The `stillwindClsx` runtime export.
    Clsx,
    /// The `stillwindClsxChecked` export of the development runtime module.
    ClsxChecked,
    /// The `stillwindCssValue` runtime export.
    CssValue,
    /// The `stillwindCustomPropertiesKey` runtime export.
    CustomPropertiesKey,
    /// The `stillwindMerge` runtime export.
    Merge,
    /// The `stillwindStyleKey` runtime export.
    StyleKey,
    /// The `stillwindToFragment` runtime export.
    ToFragment,
    /// The `stillwindViewTransition` runtime export.
    ViewTransition,
    /// The `stillwindViewTransitionStyle` runtime export.
    ViewTransitionStyle,
}

impl RuntimeHelper {
    /// The export name in the runtime module, before local renaming.
    fn exported(self) -> &'static str {
        match self {
            Self::Animation => "stillwindAnimation",
            Self::AssetClass => "stillwindAssetClass",
            Self::ClassKey => "stillwindClassKey",
            Self::Clsx => "stillwindClsx",
            Self::ClsxChecked => "stillwindClsxChecked",
            Self::CssValue => "stillwindCssValue",
            Self::CustomPropertiesKey => "stillwindCustomPropertiesKey",
            Self::Merge => "stillwindMerge",
            Self::StyleKey => "stillwindStyleKey",
            Self::ToFragment => "stillwindToFragment",
            Self::ViewTransition => "stillwindViewTransition",
            Self::ViewTransitionStyle => "stillwindViewTransitionStyle",
        }
    }

    /// The development-only export that replaces this helper, when one exists.
    ///
    /// The checked helper keeps the release helper's call shape, so only the import
    /// changes between modes.
    fn development_variant(self) -> Option<Self> {
        match self {
            Self::Clsx => Some(Self::ClsxChecked),
            _ => None,
        }
    }
}

/// One macro import and the module its helpers are imported from.
///
/// A helper with a development variant is imported from the development module, which
/// has no export condition; the generated call itself does not change between modes.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ImportAnchor {
    /// Index of the macro import whose replacement carries the import statement.
    index: usize,
    /// Whether this group's helpers come from the development module.
    development: bool,
}

/// Allocates local helper names and plans their imports for one module.
///
/// Uses are keyed by helper and import realm, so a module that renders in several
/// realms (a Svelte markup region and its script, for example) gets one import per
/// realm and never one import per use.
pub(crate) struct RuntimeBindings<'a> {
    /// Projection regions of the module, which decide the import realm of each use.
    regions: &'a [ProjectionRegion],
    /// Names already bound in the module; never handed out as a local helper name.
    reserved: BTreeSet<String>,
    /// One local name per helper and realm, allocated on first use.
    names: BTreeMap<(RuntimeHelper, Option<u32>), String>,
    /// Every recorded use, in the order it was requested.
    uses: Vec<(RuntimeHelper, Span, Option<u32>)>,
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
                .projection
                .as_ref()
                .map_or(&[], |projection| projection.regions.as_slice()),
            reserved: module.ir.imports.reserved_names.iter().cloned().collect(),
            names: BTreeMap::new(),
            uses: Vec::new(),
            development: false,
        }
    }

    /// Returns the same bindings for a compilation in `mode`.
    ///
    /// The mode selects the module a helper is imported from and, for a helper with a
    /// development variant, the export itself. The generated call is the same in both
    /// modes, because only the import differs.
    pub(crate) fn in_mode(mut self, mode: crate::IdentifierMode) -> Self {
        self.development = mode == crate::IdentifierMode::Development;
        self
    }

    /// Records a use of `helper` at `at` and returns the local name to call it with.
    ///
    /// Repeated uses of one helper in one realm share a single name and a single import.
    /// The name is the export name unless it collides with a reserved name, in which case
    /// a numeric suffix is appended until it is free. Allocation is deterministic
    /// for a fixed use order; different realms compete for names in one reserved set.
    pub(crate) fn require(&mut self, helper: RuntimeHelper, at: Span) -> String {
        let realm = self
            .regions
            .iter()
            .find(|region| contains(region.source, at))
            .map(|region| region.import_realm);
        self.uses.push((helper, at, realm));
        self.names
            .entry((helper, realm))
            .or_insert_with(|| {
                let base = helper.exported();
                let mut name = base.to_owned();
                let mut suffix = 0_u64;
                while !self.reserved.insert(name.clone()) {
                    suffix += 1;
                    name = format!("{base}_{suffix}");
                }
                name
            })
            .clone()
    }

    /// Builds one import statement for a group of helpers sharing a module and anchor.
    ///
    /// A helper with a development variant is imported under its development export name
    /// and bound to the release name, so the generated call sites are mode independent.
    fn import_statement(
        &self,
        module: &ModuleAnalysis,
        anchor: ImportAnchor,
        helpers: BTreeSet<(RuntimeHelper, Option<u32>)>,
    ) -> String {
        let names = helpers
            .into_iter()
            .map(|(helper, realm_id)| {
                let local = self
                    .local_name(helper, realm_id)
                    .unwrap_or_else(|| helper.exported().to_owned());
                let exported = match helper.development_variant() {
                    Some(development) if anchor.development => development.exported(),
                    _ => helper.exported(),
                };
                if local == exported {
                    local
                } else {
                    format!("{exported} as {local}")
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        let spec = module.ir.adapter.spec();
        let runtime_module = if anchor.development {
            spec.runtime_import_dev
        } else {
            spec.runtime_import
        };
        format!("import {{ {names} }} from \"{runtime_module}\";")
    }

    ///
    /// A helper whose development variant has a different export name is bound under
    /// the release name in both modes, so the generated call sites do not depend on the
    /// mode.
    fn local_name(&self, helper: RuntimeHelper, realm: Option<u32>) -> Option<String> {
        self.names.get(&(helper, realm)).cloned()
    }

    /// Returns the import edits and macro-removal edits for the recorded uses.
    ///
    /// Each realm's helpers are appended to the replacement of the macro import that
    /// anchors it, so imports appear next to the code that used the macro. An anchor is
    /// only selected when this compilation lowered a macro inside it; otherwise the plan
    /// fails rather than dropping the import.
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
        let mut anchors: BTreeMap<ImportAnchor, BTreeSet<(RuntimeHelper, Option<u32>)>> =
            BTreeMap::new();
        for &(helper, at, realm_id) in &self.uses {
            let anchor = runtime_anchor(module, at)?;
            anchors
                .entry(ImportAnchor {
                    index: anchor,
                    development: self.development && helper.development_variant().is_some(),
                })
                .or_default()
                .insert((helper, realm_id));
        }
        for (anchor, helpers) in anchors {
            let span = module.ir.imports.macros[anchor.index].span;
            let import = self.import_statement(module, anchor, helpers);
            if let Some(edit) = edits.iter_mut().find(|edit| edit.call_span == span) {
                if !edit.replacement.is_empty() {
                    edit.replacement.push('\n');
                }
                edit.replacement.push_str(&import);
            } else {
                // A selected anchor must have contained a successfully lowered macro.
                return Err(CompileError::new(
                    CompileErrorKind::MissingRuntimeAnchor { span },
                    span,
                ));
            }
        }
        for &span in &module.ir.imports.module_css {
            module.source.slice(span).map_err(|error| {
                CompileError::new(
                    CompileErrorKind::import_source_failure(&error),
                    module.source.full_span(),
                )
            })?;
            let target = module.input.module_css_import.as_ref().ok_or_else(|| {
                CompileError::new(CompileErrorKind::MissingModuleCssTarget { span }, span)
            })?;
            let quoted = serde_json::to_string(target).map_err(|_| {
                CompileError::new(CompileErrorKind::InvalidImportMetadata { span }, span)
            })?;
            edits.push(SourceTransform {
                call_span: span,
                replacement: quoted.replace('<', "\\u003c"),
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
                        CompileErrorKind::MissingRuntimeAnchor {
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
                        CompileErrorKind::MissingRuntimeAnchor { span: anchor },
                        anchor,
                    )
                })?;
            for specifier in &module.input.stylesheet_imports {
                edit.replacement.push_str("\nimport ");
                let quoted = serde_json::to_string(specifier).map_err(|_| {
                    CompileError::new(
                        CompileErrorKind::InvalidImportMetadata { span: anchor },
                        anchor,
                    )
                })?;
                edit.replacement.push_str(&quoted.replace('<', "\\u003c"));
                edit.replacement.push(';');
            }
        }
        Ok(edits)
    }
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.file == inner.file && outer.start <= inner.start && inner.end <= outer.end
}

/// Finds the macro import statement that owns `at`'s import realm.
///
/// The realm chain is followed outward only when a realm holds no macro import of its
/// own. A cycle, a duplicated realm id, or a missing parent is invalid metadata: the
/// caller gets an error instead of an arbitrary anchor.
fn runtime_anchor(module: &ModuleAnalysis, at: Span) -> Result<usize, CompileError> {
    let Some(projection) = &module.input.projection else {
        return if module.ir.imports.macros.is_empty() {
            Err(CompileError::new(
                CompileErrorKind::MissingRuntimeAnchor { span: at },
                at,
            ))
        } else {
            Ok(0)
        };
    };
    let region = projection_region(module, at)?.ok_or_else(|| {
        CompileError::new(CompileErrorKind::MissingRuntimeAnchor { span: at }, at)
    })?;
    module.source.slice(region.source).map_err(|error| {
        CompileError::new(
            CompileErrorKind::import_source_failure(&error),
            module.source.full_span(),
        )
    })?;
    let mut visited = BTreeSet::new();
    let mut id = region.import_realm;
    loop {
        if !visited.insert(id) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidImportMetadata { span: at },
                at,
            ));
        }
        let mut realms = projection.realms.iter().filter(|realm| realm.id == id);
        let realm = realms.next().ok_or_else(|| {
            CompileError::new(CompileErrorKind::MissingRuntimeAnchor { span: at }, at)
        })?;
        if realms.next().is_some() {
            return Err(CompileError::new(
                CompileErrorKind::InvalidImportMetadata { span: at },
                at,
            ));
        }
        module.source.slice(realm.source).map_err(|error| {
            CompileError::new(
                CompileErrorKind::import_source_failure(&error),
                module.source.full_span(),
            )
        })?;
        if let Some(anchor) = module
            .ir
            .imports
            .macros
            .iter()
            .position(|import| contains(realm.source, import.span))
        {
            return Ok(anchor);
        }
        id = realm.parent.ok_or_else(|| {
            CompileError::new(CompileErrorKind::MissingRuntimeAnchor { span: at }, at)
        })?;
    }
}

/// The innermost projection region containing `at`, if the module has a projection.
///
/// # Errors
///
/// Returns invalid metadata when no region contains the span or when two regions do,
/// because overlapping regions would make the lookup order-dependent.
fn projection_region(
    module: &ModuleAnalysis,
    at: Span,
) -> Result<Option<&ProjectionRegion>, CompileError> {
    let Some(projection) = &module.input.projection else {
        return Ok(None);
    };
    let mut regions = projection
        .regions
        .iter()
        .filter(|region| contains(region.source, at));
    let region = regions.next().ok_or_else(|| {
        CompileError::new(CompileErrorKind::MissingRuntimeAnchor { span: at }, at)
    })?;
    if regions.next().is_some() {
        return Err(CompileError::new(
            CompileErrorKind::InvalidImportMetadata { span: at },
            at,
        ));
    }
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
/// Returns invalid metadata when regions cannot be resolved unambiguously or a quoted
/// region is not delimited by its recorded quote. Earlier edits may already be encoded;
/// the caller discards the plan on failure.
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
                CompileErrorKind::InvalidImportMetadata {
                    span: region.source,
                },
                region.source,
            ));
        }
        edit.encode_attribute(character, entity);
    }
    Ok(())
}
