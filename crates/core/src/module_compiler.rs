//! Snapshot-pinned compilation and structured, independently owned CSS rules.

use std::collections::BTreeSet;

use cssparser::{ParseError, Parser, Token};

use stillwind_ir::StyleInput;
use stillwind_semantic::IdentityId;
use stillwind_source::{CompileError, LintFinding, SourceFile};

use crate::transforms::apply_transforms;
use crate::{
    CompileResult, Compiler, DesignSystemSnapshot, Fingerprint, GeneratedClass, GeneratedSymbol,
    ModuleAnalysis, ModuleId, SourceTransform, TransformError,
};

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ModuleTransformError(#[from] pub TransformError);

/// Content identity of one generated CSS rule.
///
/// ```compile_fail
/// use stillwind_core::{Fingerprint, RuleKey};
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

impl ModuleCssArtifact {
    pub fn to_css(&self) -> String {
        self.rules.iter().map(GeneratedRule::to_css).collect()
    }

    /// Lossless boundary between the existing emitter and rule ownership. CSS
    /// tokenization keeps braces inside strings, functions and comments intact.
    pub(crate) fn from_css(css: &str) -> Self {
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
                    .expect("generated CSS block is valid");
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
        Self { rules }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleArtifactKey {
    pub compiler_version: String,
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

    pub fn to_legacy(&self, snapshot: &DesignSystemSnapshot) -> CompileResult {
        let module_css = self.css.to_css();
        CompileResult {
            css: if self.is_success() {
                snapshot.render_stylesheet(&module_css)
            } else {
                String::new()
            },
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

#[derive(Clone, Debug)]
pub struct ModuleCompiler {
    design_system: DesignSystemSnapshot,
}

impl ModuleCompiler {
    pub fn new(design_system: DesignSystemSnapshot) -> Self {
        Self { design_system }
    }
    pub fn design_system(&self) -> &DesignSystemSnapshot {
        &self.design_system
    }

    pub fn key_for(&self, module: &ModuleAnalysis) -> ModuleArtifactKey {
        ModuleArtifactKey {
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
            semantic_revision: module.semantic_revision.clone(),
        }
    }

    pub fn compile(&self, module: &ModuleAnalysis) -> CompiledModule {
        let result =
            Compiler::compile_with_snapshot(&module.source, module.parsed(), &self.design_system);
        let mut diagnostics = result.diagnostics;
        diagnostics.extend(module.integration_diagnostics.iter().map(|error| {
            CompileError::new(
                "SHFACT001",
                error.message.clone(),
                module.source.full_span(),
            )
        }));
        let css = if diagnostics.is_empty() {
            ModuleCssArtifact::from_css(&result.module_css)
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
            let artifact = ModuleCssArtifact::from_css(css);
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
            proptest::prop_assert_eq!(ModuleCssArtifact::from_css(&css).to_css(), css);
        }
    }
}
