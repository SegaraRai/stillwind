//! Snapshot-pinned compilation and structured, independently owned CSS rules.
//!
//! [`ModuleCompiler`] compiles one module analysis against one immutable design-system
//! snapshot and returns a [`CompiledModule`]: source transforms, a CSS artifact split
//! into independently owned rules, generated classes and symbols, diagnostics, and the
//! dependency sets a cache needs. Rule identity is content identity, so the project
//! layer can compare, move, or drop rules it did not emit itself. URLs found in
//! generated CSS are replaced with placeholders and reported, because the compiler
//! never resolves a path.
//!
//! A failed compilation still returns the artifact shape, but with empty transforms,
//! classes, symbols, and CSS. Callers must check [`CompiledModule::is_success`] before
//! applying anything from it.

use std::collections::BTreeSet;

use stillwind_css::relative_css_urls;
use stillwind_design_system::DesignSystemSnapshot;
use stillwind_diagnostics::{CompileDiagnostic, CompileError};
use stillwind_ir::{
    ClassExpression, GroupEntry, ModuleAnalysis, RuleExpression, StyleInput, StyleVariant,
};
use stillwind_source::{Fingerprint, ModuleId};

use crate::{
    CompilationDependencies, CompileResult, CompiledModule, Compiler, CompilerOptions,
    IdentifierContext, Identifiers, ModuleArtifactKey, ModuleCssArtifact, ModuleResourceReference,
};

/// Compiles module analyses against one immutable design system.
#[derive(Clone, Debug)]
pub struct ModuleCompiler {
    /// Pinned immutable design-system revision used to compile module analyses.
    design_system: DesignSystemSnapshot,
    /// Compiler configuration included in artifact identity.
    options: CompilerOptions,
}

impl ModuleCompiler {
    /// Creates a compiler pinned to `design_system`.
    ///
    /// The options' prefixes are resolved against the snapshot, so a stale options
    /// value cannot emit text that disagrees with the theme it is compiled against.
    pub fn new(design_system: DesignSystemSnapshot, options: CompilerOptions) -> Self {
        let options = options.resolved(&design_system);
        Self {
            design_system,
            options,
        }
    }

    /// The snapshot this compiler was created with.
    pub fn design_system(&self) -> &DesignSystemSnapshot {
        &self.design_system
    }

    /// The cache key for `module`; computing it does not compile anything.
    ///
    /// When the module carries asset support modules, their revisions are folded into
    /// `semantic_revision`, because changing a supporting module changes this module's
    /// output even though its own source did not move.
    pub fn key_for(&self, module: &ModuleAnalysis) -> ModuleArtifactKey {
        ModuleArtifactKey {
            compiler_options: Fingerprint::of(format!("{:?}", self.options)),
            analysis_revision: Fingerprint::of(format!(
                "{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}",
                module.input.revision(),
                module.ir,
                module.summary,
                module.external_assets,
                module.lint_directives,
                module.diagnostics,
                module.lint_findings,
                module.asset_support,
            )),
            compiler_version: concat!(
                env!("CARGO_PKG_VERSION"),
                "+",
                env!("STILLWIND_COMPILER_FINGERPRINT")
            )
            .into(),
            design_system: self.design_system.fingerprints().content.clone(),
            source_revision: module.source_revision.clone(),
            module_id: module.module_id.clone(),
            source_file_id: module.source.id().0,
            frontend_kind: module.frontend_kind.clone(),
            adapter_mode: format!("{:?}", module.ir.adapter),
            semantic_revision: if module.asset_support.is_empty() {
                module.semantic_revision.clone()
            } else {
                Some(Fingerprint::of(format!(
                    "{:?}{:?}",
                    module.semantic_revision,
                    module
                        .asset_support
                        .iter()
                        .map(|support| (&support.module_id, &support.source_revision, &support.ir))
                        .collect::<Vec<_>>()
                )))
            },
        }
    }

    /// Compiles the module together with the support modules its assets came from.
    ///
    /// Support modules contribute declaration CSS and symbols, with resources owned by
    /// their own source paths. Their source transforms are not returned here.
    fn compile_support_closure(
        &self,
        module: &ModuleAnalysis,
        identifiers: &IdentifierContext,
    ) -> CompileResult {
        let mut result = Compiler::compile_with_snapshot(
            &module.source,
            module,
            &self.design_system,
            &self.options,
            identifiers,
        );
        self.capture_resources(&mut result, &module.module_id);
        for support in &module.asset_support {
            let mut compiled = Compiler::compile_support_with_snapshot(
                &support.source,
                support,
                &self.design_system,
                &self.options,
                identifiers,
            );
            self.capture_resources(&mut compiled, &support.module_id);
            result.module_css.push_str(&compiled.module_css);
            result.contributions.extend(compiled.contributions);
            result.resources.extend(compiled.resources);
            result.symbols.extend(compiled.symbols);
            result.rejected_names.extend(compiled.rejected_names);
            result.diagnostics.extend(compiled.diagnostics);
        }
        result
            .rejected_names
            .extend(identifiers.drain_rejected().into_entries());
        result
    }

    /// Replaces every resolvable URL in the emitted CSS with an opaque placeholder.
    ///
    /// References the design system already owns a placeholder for are left alone, so a
    /// theme asset keeps its host-facing identity. Placeholders are derived from the
    /// owner path and the specifier, and references are rewritten from the end of the
    /// CSS towards the start so earlier byte ranges stay valid.
    fn capture_resources(&self, result: &mut CompileResult, owner: &ModuleId) {
        for contribution in &mut result.contributions {
            let mut references = relative_css_urls(&contribution.css);
            references.retain(|reference| {
                !self
                    .design_system
                    .sources()
                    .resources
                    .iter()
                    .any(|resource| resource.placeholder == reference.specifier)
            });
            for reference in references.into_iter().rev() {
                let placeholder = format!(
                    "__stillwind_js_resource_{}__",
                    Fingerprint::of(format!(
                        "stillwind/resource/module/v1\0{owner}\0{}\0",
                        reference.specifier
                    ))
                    .0
                );
                let replacement = if reference.string {
                    format!("\"{placeholder}\"")
                } else {
                    format!("url(\"{placeholder}\")")
                };
                contribution
                    .css
                    .replace_range(reference.range, &replacement);
                result.resources.push(ModuleResourceReference {
                    placeholder,
                    owner: owner.clone(),
                    specifier: reference.specifier,
                });
            }
        }
        result.module_css = result
            .contributions
            .iter()
            .map(|part| part.css.as_str())
            .collect();
    }

    /// Compiles one module with explicit naming inputs.
    ///
    /// Integration diagnostics from the frontend are converted to source-spanning
    /// errors before any CSS is split, and the CSS artifact is empty as long as any
    /// diagnostic remains: a failed module must not hand half an artifact to the
    /// project layer.
    pub fn compile(&self, module: &ModuleAnalysis, identifiers: &Identifiers) -> CompiledModule {
        let identifiers = identifiers.context(self.options.hash_length);
        let result = self.compile_support_closure(module, &identifiers);
        let mut diagnostics = result.diagnostics;
        diagnostics.extend(
            module.integration_diagnostics.iter().map(|error| {
                CompileError::new(error.diagnostic.clone(), module.source.full_span())
            }),
        );
        let css = if diagnostics.is_empty() {
            match ModuleCssArtifact::from_contributions(&result.contributions) {
                Ok(css) => css,
                Err(error) => {
                    diagnostics.push(CompileError::new(
                        CompileDiagnostic::InvalidGeneratedCss { offset: error },
                        module.source.full_span(),
                    ));
                    ModuleCssArtifact::default()
                }
            }
        } else {
            ModuleCssArtifact::default()
        };
        let theme = self.design_system.theme();
        let dependencies = CompilationDependencies {
            candidates: compilation_candidates(module),
            // Conservative definition dependencies are safe for full invalidation.
            theme_tokens: theme
                .tokens_required_by(&result.module_css, false, &[])
                .iter()
                .map(|token| token.name.value.clone())
                .collect(),
            utilities: self
                .design_system
                .fingerprints()
                .utilities
                .keys()
                .cloned()
                .collect(),
            variants: self
                .design_system
                .fingerprints()
                .variants
                .keys()
                .cloned()
                .collect(),
            properties: self
                .design_system
                .fingerprints()
                .properties
                .keys()
                .cloned()
                .collect(),
            assets: module
                .summary
                .assets
                .iter()
                .map(|asset| asset.identity.clone())
                .chain(module.external_assets.iter().cloned())
                .collect(),
        };
        let success = diagnostics.is_empty();
        CompiledModule {
            style_effects: if success {
                result.style_effects
            } else {
                Vec::new()
            },
            composition: result.composition,
            resources: result.resources,
            key: self.key_for(module),
            code_transforms: if success {
                result.transforms
            } else {
                Vec::new()
            },
            css,
            classes: if success { result.classes } else { Vec::new() },
            symbols: if success { result.symbols } else { Vec::new() },
            rejected_names: result.rejected_names,
            diagnostics,
            lint_findings: result.lint_findings,
            dependencies,
        }
    }
}

/// Candidate spellings a module can mention, including `container` query bodies.
///
/// The walk is a conservative superset of what the module resolves: variant prefixes
/// are concatenated so a qualified candidate is recorded in its spelled form, and
/// unresolved candidates are included too, because they may resolve after a
/// design-system change.
fn compilation_candidates(module: &ModuleAnalysis) -> BTreeSet<String> {
    let mut candidates = BTreeSet::new();
    for call in &module.ir.style_calls {
        collect(&call.inputs, "", &mut candidates);
    }
    candidates
}

fn collect(inputs: &[StyleInput], prefix: &str, candidates: &mut BTreeSet<String>) {
    for input in inputs {
        match input {
            StyleInput::Array(inputs) => {
                collect(inputs, prefix, candidates);
            }
            StyleInput::Class(expression) => {
                collect_class(expression, prefix, candidates);
            }
            StyleInput::Conditional { branches, .. } => {
                collect(branches.consequent(), prefix, candidates);
                if let Some(alternate) = branches.alternate() {
                    collect(alternate, prefix, candidates);
                }
            }
            _ => {}
        }
    }
}

fn collect_class(expression: &ClassExpression, prefix: &str, candidates: &mut BTreeSet<String>) {
    match expression {
        ClassExpression::Rule(rule) => {
            collect_rule(rule, prefix, candidates);
        }
        ClassExpression::Array(elements) => {
            for element in elements {
                collect_class(element, prefix, candidates);
            }
        }
        ClassExpression::Conditional { branches, .. } => {
            for expression in branches
                .consequent()
                .iter()
                .chain(branches.alternate().into_iter().flatten())
            {
                collect_class(expression, prefix, candidates);
            }
        }
        ClassExpression::Group { entries, .. } => {
            for entry in entries {
                if let GroupEntry::Class(expression) = entry {
                    collect_class(expression, prefix, candidates);
                }
            }
        }
        ClassExpression::ClassValue { .. } => {}
    }
}

/// Adds authored utility candidates in a rule subtree to the compilation set.
/// Literal variant prefixes are included; ref and container variants do not
/// alter the candidate spelling passed to the utility resolver.
fn collect_rule(expression: &RuleExpression, prefix: &str, candidates: &mut BTreeSet<String>) {
    match expression {
        RuleExpression::Utilities(value) => {
            candidates.extend(
                value
                    .value
                    .split_whitespace()
                    .map(|token| format!("{prefix}{token}")),
            );
        }
        RuleExpression::Array(elements)
        | RuleExpression::Group {
            inputs: elements, ..
        } => {
            for element in elements {
                collect_rule(element, prefix, candidates);
            }
        }
        RuleExpression::Conditional { branches, .. } => {
            for expression in branches
                .consequent()
                .iter()
                .chain(branches.alternate().into_iter().flatten())
            {
                collect_rule(expression, prefix, candidates);
            }
        }
        RuleExpression::Variant { variant, inputs } => {
            let prefix = match variant {
                StyleVariant::Prefix(value) => format!("{prefix}{}", value.value),
                StyleVariant::Ref { .. } | StyleVariant::Container { .. } => prefix.to_owned(),
            };
            for input in inputs {
                collect_rule(input, &prefix, candidates);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
    use stillwind_diagnostics::CompileDiagnostic;
    use stillwind_ir::{ScriptLanguage, ScriptModuleKind, SourceAnalyzer};
    use stillwind_js_oxc::OxcSourceAnalyzer;
    use stillwind_semantic::TypeFacts;
    use stillwind_source::{CssSourceName, FileId, Fingerprint, ModuleId, SourceFile};

    use super::*;

    /// Facts whose revisions no longer match the scan cannot be used, and the compiler
    /// reports that as a diagnostic with no CSS or transforms.
    #[test]
    fn unusable_checker_facts_become_a_compilation_error() {
        let source = Arc::new(
            SourceFile::new(
                FileId(1),
                None,
                None,
                "import { style } from 'stillwind/react'; export const x = style('flex');",
            )
            .unwrap(),
        );
        let scan = OxcSourceAnalyzer.scan(
            source,
            ModuleId::from("test.tsx"),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        );
        let facts = TypeFacts {
            schema: TypeFacts::SCHEMA,
            module_id: ModuleId::from("entry.tsx"),
            source_revision: Fingerprint::of("stale"),
            semantic_revision: Fingerprint::of("facts"),
            facts: vec![],
            assets: BTreeMap::new(),
        };
        let analysis = OxcSourceAnalyzer.finalize(scan, Some(&facts));
        assert!(analysis.semantic_revision.is_none());
        assert_eq!(analysis.integration_diagnostics.len(), 1);
        let theme = crate::css_fixture::source(
            FileId(90),
            "@utility flex { display:flex; } @stillwind-slot utilities;",
        )
        .unwrap();
        let design = DesignSystemSnapshot::from_source(
            &theme,
            CssSourceName::from("test.css"),
            DesignSystemOptions::default(),
        )
        .unwrap();
        let compiled = ModuleCompiler::new(design, CompilerOptions::default())
            .compile(&analysis, &Identifiers::default());
        assert!(
            compiled
                .diagnostics
                .iter()
                .any(|d| d.diagnostic == CompileDiagnostic::InvalidTypeScriptFacts)
        );
        assert!(compiled.css.rules.is_empty());
        assert!(compiled.code_transforms.is_empty());
    }
}
