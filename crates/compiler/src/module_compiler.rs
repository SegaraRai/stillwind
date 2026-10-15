//! Compiles modules using a fixed design-system snapshot.
//!
//! [`ModuleCompiler`] takes a module analysis and returns a [`CompiledModule`] with
//! source edits, CSS rules, classes, symbols, diagnostics, and dependencies. Rule keys
//! identify content, so the project layer can compare and manage individual rules.
//! The compiler replaces CSS URLs with placeholders for the host to resolve.
//! [`ModuleCompiler::editors`] instead captures occurrences and replacement plans for
//! display queries. Captures render each site's CSS independently of artifact
//! publication.
//!
//! Failed compilation returns empty transforms, classes, symbols, and CSS.
//! Callers must check [`CompiledModule::is_success`] before applying output.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::relative_css_urls;
use stillwind_design_system::DesignSystemSnapshot;
use stillwind_diagnostics::{CompileDiagnostic, CompileError, LintKind};
use stillwind_ir::{
    ClassExpression, GroupEntry, ModuleAnalysis, RuleExpression, StyleInput, StyleVariant,
};
use stillwind_source::{Fingerprint, ModuleId};

use crate::compiler::reserve_preview_names;
use crate::identifiers::PreviewNames;
use crate::{
    CompilationDependencies, CompileResult, CompiledModule, Compiler, CompilerOptions,
    IdentifierContext, Identifiers, ModuleArtifactKey, ModuleCssArtifact, ModuleEditor,
    ModuleResourceReference,
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
    /// Captures finalized `modules` in module identity order without publishing
    /// artifacts. Analyses must contain finalized IR and the supporting analyses for
    /// referenced assets. Repeated module identities keep the last analysis supplied.
    /// `identifiers` supplies the normal generation and validation policy for capture
    /// and replacement queries.
    ///
    /// All captures share display names reserved for the batch's declarations and
    /// helpers. Each capture retains its analysis and settings for later queries.
    /// Failed occurrences remain as invalid sites, and an empty iterator returns an
    /// empty vector.
    pub fn editors<'a>(
        &self,
        modules: impl IntoIterator<Item = &'a ModuleAnalysis>,
        identifiers: &Identifiers,
    ) -> Vec<ModuleEditor> {
        let modules: BTreeMap<_, _> = modules
            .into_iter()
            .map(|module| (&module.module_id, module))
            .collect();
        let names = Rc::new(RefCell::new(PreviewNames::default()));
        for module in modules.values() {
            reserve_preview_names(module, &mut names.borrow_mut());
        }
        modules
            .values()
            .map(|module| {
                ModuleEditor::new(
                    module,
                    self.design_system.clone(),
                    self.options.clone(),
                    identifiers.clone(),
                    Fingerprint::of(
                        format_args!(
                            "{:?}{:?}{:?}{:?}{:?}",
                            module.module_id,
                            module.source_revision,
                            self.design_system.fingerprints().content,
                            self.options,
                            module
                                .asset_support
                                .iter()
                                .map(|support| (&support.module_id, &support.source_revision))
                                .collect::<Vec<_>>(),
                        )
                        .to_lean_string(),
                    ),
                    Rc::clone(&names),
                )
            })
            .collect()
    }

    /// Checks the available IR without returning generated artifacts or publishing a
    /// module.
    ///
    /// Frontend failures remain diagnostics. Other calls that the frontend could lower
    /// still contribute local utility findings. The caller must preserve the analysis
    /// source registry.
    pub fn diagnose(&self, module: &ModuleAnalysis, identifiers: &Identifiers) -> CompileResult {
        let mut local = module.clone();
        local.diagnostics.clear();
        let context = identifiers.context(self.options.hash_length);
        let mut result = Compiler::compile_support_with_snapshot(
            &local.source,
            &local,
            &self.design_system,
            &self.options,
            &context,
        );
        if !module.diagnostics.is_empty() || !module.integration_diagnostics.is_empty() {
            result
                .lint_findings
                .retain(|finding| finding.kind() != LintKind::NoUnusedLintSuppressions);
        }
        let mut diagnostics = module.diagnostics.clone();
        diagnostics.extend(result.diagnostics);
        diagnostics.extend(
            module.integration_diagnostics.iter().map(|error| {
                CompileError::new(error.diagnostic.clone(), module.source.full_span())
            }),
        );
        CompileResult {
            diagnostics,
            lint_findings: result.lint_findings,
            source_lint_findings: result.source_lint_findings,
            lint_settings: result.lint_settings,
            ..CompileResult::default()
        }
    }

    /// Creates a compiler that keeps the supplied `design_system` snapshot.
    ///
    /// Resolves prefixes in `options` against the snapshot so output uses the theme's
    /// prefix settings even if the supplied options are outdated.
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

    /// Returns the cache key for `module` without compiling it.
    ///
    /// If the module has asset support, `semantic_revision` also hashes the support
    /// modules' revisions. Changing those modules can change this module's output
    /// even when its own source is unchanged.
    pub fn key_for(&self, module: &ModuleAnalysis) -> ModuleArtifactKey {
        ModuleArtifactKey {
            compiler_options: Fingerprint::of(format_args!("{:?}", self.options).to_lean_string()),
            analysis_revision: Fingerprint::of(
                format_args!(
                    "{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}",
                    module.input.revision(),
                    module.ir,
                    module.summary,
                    module.external_assets,
                    module.lint_directives,
                    module.diagnostics,
                    module.lint_findings,
                    module.asset_support,
                )
                .to_lean_string(),
            ),
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
            frontend_kind: (module.frontend_kind.clone()),
            adapter_mode: module.ir.adapter,
            semantic_revision: if module.asset_support.is_empty() {
                module.semantic_revision.clone()
            } else {
                Some(Fingerprint::of(
                    format_args!(
                        "{:?}{:?}",
                        module.semantic_revision,
                        module
                            .asset_support
                            .iter()
                            .map(|support| (
                                &support.module_id,
                                &support.source_revision,
                                &support.ir
                            ))
                            .collect::<Vec<_>>()
                    )
                    .to_lean_string(),
                ))
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
            if references.is_empty() {
                continue;
            }
            let mut css: LeanString = std::mem::take(&mut contribution.css).into();
            for reference in references.into_iter().rev() {
                let placeholder = format_args!(
                    "__stillwind_js_resource_{}__",
                    Fingerprint::of(
                        format_args!(
                            "stillwind/resource/module/v1\0{owner}\0{}\0",
                            reference.specifier
                        )
                        .to_lean_string()
                    )
                    .0
                )
                .to_lean_string();
                let replacement = if reference.string {
                    format_args!("\"{placeholder}\"").to_lean_string()
                } else {
                    format_args!("url(\"{placeholder}\")").to_lean_string()
                };
                css.replace_range(reference.range, &replacement);
                result.resources.push(ModuleResourceReference {
                    placeholder: (placeholder).into(),
                    owner: owner.clone(),
                    specifier: reference.specifier,
                });
            }
            contribution.css = css.into();
        }
        result.module_css = result
            .contributions
            .iter()
            .map(|part| part.css.as_str())
            .collect();
    }

    /// Compiles one module with explicit naming inputs.
    ///
    /// Frontend integration errors receive the module's full source span. If any
    /// compilation error remains, the returned CSS artifact is empty.
    ///
    /// Successful artifacts keep the effects of this module's own style calls.
    /// Asset support modules supply declarations without adding style-call effects.
    ///
    /// Spans in `module` must refer to its editable source, and its support records
    /// must be valid. `identifiers` supplies the naming rules. Each call creates a
    /// separate naming context. Errors are returned in the artifact's diagnostics.
    /// Failed compilation clears style effects, CSS, classes, symbols, and transforms.
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
            utilities: self.design_system.utility_names().clone(),
            variants: self.design_system.variant_names().clone(),
            properties: self.design_system.property_names().clone(),
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
            lint_settings: result.lint_settings,
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
            source_lint_findings: result.source_lint_findings,
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
fn compilation_candidates(module: &ModuleAnalysis) -> BTreeSet<LeanStr> {
    let mut candidates = BTreeSet::new();
    for call in module.ir.style_inputs() {
        collect(&call.inputs, "", &mut candidates);
    }
    candidates
}

fn collect(inputs: &[StyleInput], prefix: &str, candidates: &mut BTreeSet<LeanStr>) {
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

fn collect_class(expression: &ClassExpression, prefix: &str, candidates: &mut BTreeSet<LeanStr>) {
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
fn collect_rule(expression: &RuleExpression, prefix: &str, candidates: &mut BTreeSet<LeanStr>) {
    match expression {
        RuleExpression::Utilities(value) => {
            candidates.extend(
                value
                    .value
                    .split_whitespace()
                    .map(|token| format_args!("{prefix}{token}").to_lean_str()),
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
                StyleVariant::Prefix(value) => {
                    format_args!("{prefix}{}", value.value).to_lean_string()
                }
                StyleVariant::Ref { .. } | StyleVariant::Container { .. } => {
                    prefix.to_lean_string()
                }
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
    use stillwind_test_support::css_source;

    use super::*;
    use crate::{GeneratedRule, SourceTransform};

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
        let theme = css_source(
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
        assert_eq!(compiled.css.rules, [] as [GeneratedRule; 0]);
        assert_eq!(compiled.code_transforms, [] as [SourceTransform; 0]);
    }
}
