//! Snapshot-pinned compilation and structured, independently owned CSS rules.

use std::collections::BTreeSet;

use cssparser::{ParseError, Parser, Token};
use serde::Serialize;

use stillwind_css::relative_css_urls;
use stillwind_design_system::DesignSystemSnapshot;
use stillwind_ir::{ModuleAnalysis, StyleInput};
use stillwind_semantic::{IdentityId, ModuleId};
use stillwind_source::{CompileError, FileId, Fingerprint, LintFinding, SourceFile, Span};

use crate::transforms::apply_transforms;
use crate::{
    CompileResult, Compiler, CompilerOptions, GeneratedClass, GeneratedSymbol, SourceTransform,
    StylesheetRenderer, TransformError,
};

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ModuleTransformError(#[from] pub TransformError);

#[derive(Debug, thiserror::Error)]
#[error("invalid generated CSS block at byte {offset}")]
pub(crate) struct CssArtifactError {
    offset: usize,
}

/// Content identity of one generated CSS rule.
///
/// ```compile_fail
/// use stillwind_source::Fingerprint;
/// use stillwind_compiler::RuleKey;
/// let rule: RuleKey = Fingerprint::of(".x{}");
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct RuleKey(pub Fingerprint);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedRule {
    pub key: RuleKey,
    /// Selector or at-rule header, including authored serialization whitespace.
    pub prelude: String,
    /// Declaration/nested-rule block; `None` represents an at-rule statement.
    pub body: Option<String>,
}

impl GeneratedRule {
    pub fn to_css(&self) -> String {
        self.body.as_ref().map_or_else(
            || self.prelude.clone(),
            |body| format!("{}{{{body}}}", self.prelude),
        )
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModuleCssArtifact {
    pub rules: Vec<GeneratedRule>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModuleResourceReference {
    pub placeholder: String,
    pub owner: String,
    pub specifier: String,
}

impl ModuleCssArtifact {
    pub fn to_css(&self) -> String {
        self.rules.iter().map(GeneratedRule::to_css).collect()
    }

    /// Lossless boundary between the existing emitter and rule ownership. CSS
    /// tokenization keeps braces inside strings, functions and comments intact.
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleArtifactKey {
    pub compiler_version: String,
    pub compiler_options: Fingerprint,
    /// The frontend's actual compiler inputs, independent of its implementation.
    pub analysis_revision: Fingerprint,
    pub design_system: Fingerprint,
    pub source_revision: Fingerprint,
    pub module_id: ModuleId,
    pub source_file_id: u32,
    pub frontend_kind: String,
    pub adapter_mode: String,
    pub semantic_revision: Option<Fingerprint>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompilationDependencies {
    pub candidates: BTreeSet<String>,
    pub theme_tokens: BTreeSet<String>,
    pub utilities: BTreeSet<String>,
    pub variants: BTreeSet<String>,
    pub properties: BTreeSet<String>,
    pub assets: BTreeSet<IdentityId>,
}

#[derive(Clone, Debug)]
pub struct CompiledModule {
    pub resources: Vec<ModuleResourceReference>,
    pub key: ModuleArtifactKey,
    pub code_transforms: Vec<SourceTransform>,
    pub css: ModuleCssArtifact,
    pub classes: Vec<GeneratedClass>,
    pub symbols: Vec<GeneratedSymbol>,
    pub diagnostics: Vec<CompileError>,
    pub lint_findings: Vec<LintFinding>,
    pub dependencies: CompilationDependencies,
}

impl CompiledModule {
    /// # Errors
    /// Returns invalid or overlapping source-transform spans.
    pub fn apply_to_source(&self, source: &SourceFile) -> Result<String, ModuleTransformError> {
        apply_transforms(source, &self.code_transforms).map_err(ModuleTransformError::from)
    }

    pub fn is_success(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn to_output(
        &self,
        snapshot: &DesignSystemSnapshot,
        options: &CompilerOptions,
    ) -> CompileResult {
        let module_css = self.css.to_css();
        let mut result = CompileResult {
            resources: self.resources.clone(),
            module_css,
            classes: self.classes.clone(),
            symbols: self.symbols.clone(),
            transforms: self.code_transforms.clone(),
            diagnostics: self.diagnostics.clone(),
            lint_findings: self.lint_findings.clone(),
            lint_policy: snapshot.theme().config.lint_rules.clone(),
            ..CompileResult::default()
        };
        if self.is_success() {
            match StylesheetRenderer::new(snapshot, options).render_stylesheet(&result.module_css) {
                Ok(css) => result.css = css,
                Err(error) => {
                    result.diagnostics.push(CompileError::new(
                        "SHCSS001",
                        error.to_string(),
                        Span::new(FileId(self.key.source_file_id), 0, 0),
                    ));
                    result.module_css.clear();
                    result.transforms.clear();
                    result.classes.clear();
                    result.symbols.clear();
                }
            }
        }
        result
    }
}

#[derive(Clone, Debug)]
pub struct ModuleCompiler {
    design_system: DesignSystemSnapshot,
    options: CompilerOptions,
}

impl ModuleCompiler {
    pub fn new(design_system: DesignSystemSnapshot, options: CompilerOptions) -> Self {
        let options = options.resolved(&design_system);
        Self {
            design_system,
            options,
        }
    }

    pub fn design_system(&self) -> &DesignSystemSnapshot {
        &self.design_system
    }

    pub fn key_for(&self, module: &ModuleAnalysis) -> ModuleArtifactKey {
        ModuleArtifactKey {
            compiler_options: Fingerprint::of(format!("{:?}", self.options)),
            analysis_revision: Fingerprint::of(format!(
                "{:?}{:?}{:?}{:?}{:?}{:?}{:?}",
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

    fn compile_support_closure(&self, module: &ModuleAnalysis) -> CompileResult {
        let mut result = Compiler::compile_with_snapshot(
            &module.source,
            module,
            &self.design_system,
            &self.options,
        );
        self.capture_resources(&mut result, module.source.path());
        for support in &module.asset_support {
            let mut compiled = Compiler::compile_with_snapshot(
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
            candidates: module
                .ir
                .style_calls
                .iter()
                .flat_map(|call| &call.inputs)
                .filter_map(|input| match input {
                    StyleInput::UtilityString(value)
                    | StyleInput::ConditionalUtilityString { value, .. }
                    | StyleInput::SelectorUtilityString { value, .. }
                    | StyleInput::ConditionalSelectorUtilityString { value, .. }
                    | StyleInput::HandleSelectorUtilityString { value, .. }
                    | StyleInput::ConditionalHandleSelectorUtilityString { value, .. }
                    | StyleInput::ContainerQuery { value, .. } => Some(value.value.as_str()),
                    _ => None,
                })
                .flat_map(str::split_whitespace)
                .map(str::to_owned)
                .collect(),
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
