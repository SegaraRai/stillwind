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

use cssparser::{ParseError, Parser, Token};
use serde::Serialize;

use stillwind_css::relative_css_urls;
use stillwind_design_system::DesignSystemSnapshot;
use stillwind_ir::{
    ClassExpression, GroupEntry, ModuleAnalysis, StyleDependency, StyleInput, StyleVariant,
};
use stillwind_semantic::{IdentityId, ModuleId};
use stillwind_source::{CompileError, Fingerprint, LintFinding, SourceFile};

use crate::transforms::apply_transforms;
use crate::{
    CompileResult, Compiler, CompilerOptions, GeneratedClass, GeneratedSymbol, SourceTransform,
    TransformError,
};

/// A source-transform failure as reported by [`CompiledModule::apply_to_source`].
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ModuleTransformError(#[from] pub TransformError);

/// The emitter produced CSS that cannot be split into rules.
#[derive(Debug, thiserror::Error)]
#[error("invalid generated CSS block at byte {offset}")]
pub(crate) struct CssArtifactError {
    offset: usize,
}

/// Content identity of one generated CSS rule.
///
/// Compiler-produced keys hash the rule serialization. The project layer trusts that
/// correspondence when sharing rules. Public tuple construction can supply an unrelated
/// fingerprint; the doctest only rules out an implicit conversion.
///
/// ```compile_fail
/// use stillwind_source::Fingerprint;
/// use stillwind_compiler::RuleKey;
/// let rule: RuleKey = Fingerprint::of(".x{}");
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct RuleKey(pub Fingerprint);

/// One independently owned CSS rule.
///
/// The rule is kept as prelude plus body so a consumer can reorder, deduplicate, or
/// re-serialize it without parsing CSS. `prelude` preserves the authored serialization
/// whitespace, so [`GeneratedRule::to_css`] reproduces the emitter's bytes exactly for
/// a rule produced by `ModuleCssArtifact::from_css`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedRule {
    /// Content identity of the rule's serialization.
    pub key: RuleKey,
    /// Selector or at-rule header, including authored serialization whitespace.
    pub prelude: String,
    /// Declaration/nested-rule block; `None` represents an at-rule statement.
    pub body: Option<String>,
}

impl GeneratedRule {
    /// The rule's serialization, ending in `;` for an at-rule statement.
    ///
    /// Re-serializing is not a parse: the stored strings are emitted as they are, so a
    /// round trip through [`ModuleCssArtifact`] is byte-for-byte lossless.
    pub fn to_css(&self) -> String {
        self.body.as_ref().map_or_else(
            || self.prelude.clone(),
            |body| format!("{}{{{body}}}", self.prelude),
        )
    }
}

/// The CSS one module contributes, split into rules other producers can own
/// individually.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModuleCssArtifact {
    /// Rules in emission order; the split never reorders them.
    pub rules: Vec<GeneratedRule>,
}

/// A URL in generated CSS that only the host can resolve.
///
/// The compiler cannot know where a file lives, so each reference becomes a unique
/// placeholder string and the host rewrites it later. Repeated references with the
/// same owner and decoded specifier share a placeholder; different owners are hashed
/// separately.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModuleResourceReference {
    /// Token standing in for the URL in the emitted CSS, quoted or inside `url(...)`.
    pub placeholder: String,
    /// Source path of the module whose CSS contained the reference.
    pub owner: String,
    /// Decoded URL specifier, without the surrounding CSS syntax.
    pub specifier: String,
}

impl ModuleCssArtifact {
    /// Concatenates the rules in stored order.
    pub fn to_css(&self) -> String {
        self.rules.iter().map(GeneratedRule::to_css).collect()
    }

    /// Splits emitted CSS into independently owned rules without changing its bytes.
    /// CSS tokenization keeps braces inside strings, functions and comments intact.
    ///
    /// Every rule is keyed by the fingerprint of its own serialization, so keys agree
    /// with [`GeneratedRule::to_css`].
    ///
    /// # Errors
    ///
    /// Returns the byte offset where an unterminated block starts.
    pub(crate) fn from_css(css: &str) -> Result<Self, CssArtifactError> {
        let mut parser = Parser::new(css);
        let mut start = 0;
        let mut rules = Vec::new();
        while let Ok(token) = parser.next_including_whitespace_and_comments() {
            let block = matches!(token, Token::CurlyBracketBlock);
            if block {
                let body_start = parser.position().byte_index();
                let body_end = parser
                    .parse_nested_block(|nested| {
                        while nested.next_including_whitespace_and_comments().is_ok() {}
                        Ok::<_, ParseError<()>>(nested.position().byte_index())
                    })
                    .map_err(|_| CssArtifactError { offset: body_start })?;
                let end = parser.position().byte_index();
                rules.push(GeneratedRule {
                    key: RuleKey(Fingerprint::of(&css[start..end])),
                    prelude: css[start..body_start - 1].into(),
                    body: Some(css[body_start..body_end].into()),
                });
                start = end;
            } else if matches!(token, Token::Semicolon) {
                let end = parser.position().byte_index();
                rules.push(GeneratedRule {
                    key: RuleKey(Fingerprint::of(&css[start..end])),
                    prelude: css[start..end].into(),
                    body: None,
                });
                start = end;
            }
        }
        if start < css.len() {
            rules.push(GeneratedRule {
                key: RuleKey(Fingerprint::of(&css[start..])),
                prelude: css[start..].into(),
                body: None,
            });
        }
        Ok(Self { rules })
    }
}

/// Everything that can change a module's compiled output, and nothing else.
///
/// Two equal keys mean the previously compiled artifact can be reused. The compiler's
/// own build fingerprint participates through `compiler_version`, so rebuilding the
/// compiler invalidates every cached artifact, and `analysis_revision` covers the
/// frontend's actual inputs rather than its implementation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleArtifactKey {
    /// Compiler version plus the build fingerprint of the compiler and its dependencies.
    pub compiler_version: String,
    /// Fingerprint of the resolved [`CompilerOptions`]; a prefix change is a separate key.
    pub compiler_options: Fingerprint,
    /// The frontend's actual compiler inputs, independent of its implementation.
    pub analysis_revision: Fingerprint,
    /// Content fingerprint of the design system the module was compiled against.
    pub design_system: Fingerprint,
    /// Fingerprint of the source buffer, so an edit invalidates the artifact.
    pub source_revision: Fingerprint,
    /// Identity of the module inside the project graph.
    pub module_id: ModuleId,
    /// Identity of the source file, which may differ from the module id after projection.
    pub source_file_id: u32,
    /// Which frontend produced the analysis, for example `oxc`.
    pub frontend_kind: String,
    /// Adapter mode the analysis was extracted under.
    pub adapter_mode: String,
    /// Fingerprint of the type facts and asset metadata that shaped the analysis, when any.
    pub semantic_revision: Option<Fingerprint>,
}

/// Definition sets a compiled module's output was derived from.
///
/// The sets are conservative: they may hold more names than the output actually uses,
/// so a change can invalidate a still-valid artifact. They must never hold fewer,
/// because an omission would keep stale CSS. `assets` lists the handle, scope,
/// animation, and view-transition identities the module referenced.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompilationDependencies {
    /// Candidate spellings the module mentions, including their variant prefixes.
    pub candidates: BTreeSet<String>,
    /// Theme token names whose values appear in the module's generated CSS.
    pub theme_tokens: BTreeSet<String>,
    /// All currently registered utility definition names, recorded conservatively.
    pub utilities: BTreeSet<String>,
    /// All currently registered variant definition names, recorded conservatively.
    pub variants: BTreeSet<String>,
    /// All currently registered custom-property definition names.
    pub properties: BTreeSet<String>,
    /// Identities of the assets the module used.
    pub assets: BTreeSet<IdentityId>,
}

/// One module's complete compiled output.
///
/// This is the compiler-to-project boundary. The fields are produced together, so a
/// consumer receives generated classes and symbols alongside CSS and source edits.
/// Marker classes need not have a CSS rule. Transform validity is checked when the
/// edits are rendered.
/// They are only meaningful when the module compiled without diagnostics.
#[derive(Clone, Debug)]
pub struct CompiledModule {
    /// Style dependencies the project layer must order this module after.
    pub composition: Vec<StyleDependency>,
    /// URLs found in `css`, replaced there by their placeholders.
    pub resources: Vec<ModuleResourceReference>,
    /// Cache identity of this artifact.
    pub key: ModuleArtifactKey,
    /// Replacements for the original source; empty after a failed compilation.
    pub code_transforms: Vec<SourceTransform>,
    /// The rules this module owns; empty after a failed compilation.
    pub css: ModuleCssArtifact,
    /// Classes generated for this module's callsites.
    pub classes: Vec<GeneratedClass>,
    /// Generated CSS identifiers with the identities they were allocated from.
    pub symbols: Vec<GeneratedSymbol>,
    /// Errors that failed the compilation, if any.
    pub diagnostics: Vec<CompileError>,
    /// Lint findings, which do not fail the compilation.
    pub lint_findings: Vec<LintFinding>,
    /// Definitions a change to which invalidates this artifact.
    pub dependencies: CompilationDependencies,
}

impl CompiledModule {
    /// Applies the module's transforms to the original source and returns the new text.
    ///
    /// The input source is not modified; the result is a fresh string with untouched
    /// intervals preserved byte for byte.
    ///
    /// # Errors
    ///
    /// Rejects failed compilation and invalid or overlapping source-transform spans.
    pub fn apply_to_source(&self, source: &SourceFile) -> Result<String, ModuleTransformError> {
        if !self.is_success() {
            return Err(ModuleTransformError(TransformError::CompilationFailed));
        }
        apply_transforms(source, &self.code_transforms).map_err(ModuleTransformError::from)
    }

    /// True when the module compiled without diagnostics.
    ///
    /// Transforms, classes, symbols, and CSS are empty whenever this is false, so a
    /// caller can use it as the single gate before applying output.
    pub fn is_success(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Builds a producer's output without assembling document CSS or support layers.
    ///
    /// `css` and `module_css` are both the module's own rules here: the stylesheet
    /// renderer later replaces `css` with the assembled document stylesheet while
    /// leaving `module_css` addressable for a host that transports CSS per module.
    pub fn to_module_output(&self, snapshot: &DesignSystemSnapshot) -> CompileResult {
        let module_css = self.css.to_css();
        CompileResult {
            composition: self.composition.clone(),
            resources: self.resources.clone(),
            css: module_css.clone(),
            module_css,
            classes: self.classes.clone(),
            symbols: self.symbols.clone(),
            transforms: self.code_transforms.clone(),
            diagnostics: self.diagnostics.clone(),
            lint_findings: self.lint_findings.clone(),
            lint_policy: snapshot.theme().config.lint_rules.clone(),
        }
    }
}

/// Compiles module analyses against one immutable design system.
#[derive(Clone, Debug)]
pub struct ModuleCompiler {
    design_system: DesignSystemSnapshot,
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
    fn compile_support_closure(&self, module: &ModuleAnalysis) -> CompileResult {
        let mut result = Compiler::compile_with_snapshot(
            &module.source,
            module,
            &self.design_system,
            &self.options,
        );
        self.capture_resources(&mut result, module.source.path());
        for support in &module.asset_support {
            let mut compiled = Compiler::compile_support_with_snapshot(
                &support.source,
                support,
                &self.design_system,
                &self.options,
            );
            self.capture_resources(&mut compiled, support.source.path());
            result.module_css.push_str(&compiled.module_css);
            result.resources.extend(compiled.resources);
            result.symbols.extend(compiled.symbols);
            result.diagnostics.extend(compiled.diagnostics);
        }
        result
    }

    /// Replaces every resolvable URL in the emitted CSS with an opaque placeholder.
    ///
    /// References the design system already owns a placeholder for are left alone, so a
    /// theme asset keeps its host-facing identity. Placeholders are derived from the
    /// owner path and the specifier, and references are rewritten from the end of the
    /// CSS towards the start so earlier byte ranges stay valid.
    fn capture_resources(&self, result: &mut CompileResult, owner: &str) {
        let mut references = relative_css_urls(&result.module_css);
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
                    "{}:{owner}{}:{}",
                    owner.len(),
                    reference.specifier.len(),
                    reference.specifier
                ))
                .0
            );
            let replacement = if reference.string {
                format!("\"{placeholder}\"")
            } else {
                format!("url(\"{placeholder}\")")
            };
            result
                .module_css
                .replace_range(reference.range, &replacement);
            result.resources.push(ModuleResourceReference {
                placeholder,
                owner: owner.into(),
                specifier: reference.specifier,
            });
        }
    }

    /// Compiles one module into its artifact.
    ///
    /// Integration diagnostics from the frontend are converted to source-spanning
    /// errors before any CSS is split, and the CSS artifact is empty as long as any
    /// diagnostic remains: a failed module must not hand half an artifact to the
    /// project layer.
    pub fn compile(&self, module: &ModuleAnalysis) -> CompiledModule {
        let result = self.compile_support_closure(module);
        let mut diagnostics = result.diagnostics;
        diagnostics.extend(module.integration_diagnostics.iter().map(|error| {
            CompileError::new(
                "SHFACT001",
                error.message.clone(),
                module.source.full_span(),
            )
        }));
        let css = if diagnostics.is_empty() {
            match ModuleCssArtifact::from_css(&result.module_css) {
                Ok(css) => css,
                Err(error) => {
                    diagnostics.push(CompileError::new(
                        "SHCSS001",
                        error.to_string(),
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
                .tokens_required_by(&result.module_css, false)
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
    fn collect(inputs: &[StyleInput], prefix: &str, candidates: &mut BTreeSet<String>) {
        for input in inputs {
            match input {
                StyleInput::Class(expression) => collect_class(expression, prefix, candidates),
                StyleInput::ContainerQuery { value, .. } => {
                    candidates.extend(
                        value
                            .value
                            .split_whitespace()
                            .map(|token| format!("{prefix}{token}")),
                    );
                }
                _ => {}
            }
        }
    }

    fn collect_class(
        expression: &ClassExpression,
        prefix: &str,
        candidates: &mut BTreeSet<String>,
    ) {
        match expression {
            ClassExpression::Utilities(value) => {
                candidates.extend(
                    value
                        .value
                        .split_whitespace()
                        .map(|token| format!("{prefix}{token}")),
                );
            }
            ClassExpression::Array(elements) => {
                for element in elements {
                    collect_class(element, prefix, candidates);
                }
            }
            ClassExpression::Conditional {
                consequent,
                alternate,
                ..
            } => {
                for expression in consequent.iter().chain(alternate.iter()) {
                    collect_class(expression, prefix, candidates);
                }
            }
            ClassExpression::Variant { variant, inputs } => {
                let prefix = match variant {
                    StyleVariant::Prefix(value) => format!("{prefix}{}", value.value),
                    StyleVariant::Ref { .. } => prefix.to_owned(),
                };
                for input in inputs {
                    collect_class(input, &prefix, candidates);
                }
            }
            ClassExpression::Group { entries, .. } => {
                for entry in entries {
                    if let GroupEntry::Class(expression) = entry {
                        collect_class(expression, prefix, candidates);
                    }
                }
            }
            _ => {}
        }
    }
    let mut candidates = BTreeSet::new();
    for call in &module.ir.style_calls {
        collect(&call.inputs, "", &mut candidates);
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_roundtrip_preserves_comments_strings_and_statement_rules() {
        for css in [
            "",
            "@layer base;",
            "/*a*/ .x{content:'}';color:red;}\n",
            "@media print{.x{color:red;}}",
        ] {
            let artifact = ModuleCssArtifact::from_css(css).unwrap();
            assert_eq!(artifact.to_css(), css);
            for rule in artifact.rules {
                assert_eq!(rule.key, RuleKey(Fingerprint::of(rule.to_css())));
            }
        }
    }

    proptest::proptest! {
        #[test]
        fn small_rule_serialization_is_lossless(name in "[a-z]{1,15}", value in 0u16..1000) {
            let css = format!(".{name}{{width:{value}px;}}");
            proptest::prop_assert_eq!(ModuleCssArtifact::from_css(&css).unwrap().to_css(), css);
        }
    }
}
