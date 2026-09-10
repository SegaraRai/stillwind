//! Helper allocation and import placement from structured code-generation uses.

use std::collections::{BTreeMap, BTreeSet};

use stillwind_ir::ModuleAnalysis;
use stillwind_source::Span;

use crate::{ImportPlanError, SourceTransform, plan_macro_import_removals};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum RuntimeHelper {
    Animation,
    AssetClass,
    Clsx,
    HandleClass,
    Ref,
    Scope,
    ViewTransition,
    ViewTransitionStyle,
}

impl RuntimeHelper {
    fn exported(self) -> &'static str {
        match self {
            Self::Animation => "stillwindAnimation",
            Self::AssetClass => "stillwindAssetClass",
            Self::Clsx => "stillwindClsx",
            Self::HandleClass => "stillwindHandleClass",
            Self::Ref => "stillwindRef",
            Self::Scope => "stillwindScope",
            Self::ViewTransition => "stillwindViewTransition",
            Self::ViewTransitionStyle => "stillwindViewTransitionStyle",
        }
    }
}

pub(crate) struct RuntimeBindings {
    reserved: BTreeSet<String>,
    names: BTreeMap<RuntimeHelper, String>,
    uses: Vec<(RuntimeHelper, Span)>,
}

impl RuntimeBindings {
    pub(crate) fn new(reserved: &[String]) -> Self {
        Self {
            reserved: reserved.iter().cloned().collect(),
            names: BTreeMap::new(),
            uses: Vec::new(),
        }
    }

    pub(crate) fn require(&mut self, helper: RuntimeHelper, at: Span) -> String {
        self.uses.push((helper, at));
        self.names
            .entry(helper)
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

    pub(crate) fn plan(
        &self,
        module: &ModuleAnalysis,
        lowered: &[SourceTransform],
    ) -> Result<Vec<SourceTransform>, ImportPlanError> {
        let mut edits = plan_macro_import_removals(&module.source, &module.ir.imports, lowered)?;
        let mut anchors: BTreeMap<usize, BTreeSet<RuntimeHelper>> = BTreeMap::new();
        for &(helper, at) in &self.uses {
            let realm = runtime_realm(module, at)?;
            let anchor = module
                .ir
                .imports
                .macros
                .iter()
                .position(|import| realm.is_none_or(|realm| contains(realm, import.span)))
                .ok_or(ImportPlanError::MissingRuntimeAnchor { span: at })?;
            anchors.entry(anchor).or_default().insert(helper);
        }
        for (anchor, helpers) in anchors {
            let span = module.ir.imports.macros[anchor].span;
            let names = helpers
                .into_iter()
                .map(|helper| {
                    let exported = helper.exported();
                    let local = &self.names[&helper];
                    if local == exported {
                        local.clone()
                    } else {
                        format!("{exported} as {local}")
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            let import = format!(
                "import {{ {names} }} from \"{}\";",
                module.ir.adapter.spec().runtime_import
            );
            if let Some(edit) = edits.iter_mut().find(|edit| edit.call_span == span) {
                if !edit.replacement.is_empty() {
                    edit.replacement.push('\n');
                }
                edit.replacement.push_str(&import);
            } else {
                // A selected anchor must have contained a successfully lowered macro.
                return Err(ImportPlanError::MissingRuntimeAnchor { span });
            }
        }
        Ok(edits)
    }
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.file == inner.file && outer.start <= inner.start && inner.end <= outer.end
}

fn runtime_realm(module: &ModuleAnalysis, at: Span) -> Result<Option<Span>, ImportPlanError> {
    let Some(projection) = &module.input.projection else {
        return Ok(None);
    };
    let mut regions = projection
        .regions
        .iter()
        .filter(|region| contains(region.source, at));
    let region = regions
        .next()
        .ok_or(ImportPlanError::MissingRuntimeAnchor { span: at })?;
    if regions.next().is_some() {
        return Err(ImportPlanError::InvalidMetadata { span: at });
    }
    let mut realms = projection
        .realms
        .iter()
        .filter(|realm| realm.id == region.import_realm);
    let realm = realms
        .next()
        .ok_or(ImportPlanError::MissingRuntimeAnchor { span: at })?;
    if realms.next().is_some() {
        return Err(ImportPlanError::InvalidMetadata { span: at });
    }
    module.source.slice(realm.source)?;
    module.source.slice(region.source)?;
    Ok(Some(realm.source))
}
