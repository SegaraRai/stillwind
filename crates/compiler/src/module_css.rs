//! Independently owned CSS rules and lossless artifact serialization.
//!
//! `ModuleCssArtifact` splits contributions into ordered rules; `GeneratedRule`
//! retains placement and registration dependencies alongside its content identity.
//! Hosts resolve resources separately; this module does not choose stylesheet placement.

use std::sync::Arc;

use cssparser::{ParseError, Parser, Token};

use stillwind_source::{Fingerprint, SourceFile, Span};

use crate::{CssContribution, MappedStylesheet};

/// Content identity of one generated CSS rule.
///
/// Compiler-produced keys hash serialization, placement, and support dependencies.
/// Public tuple construction can supply an unrelated fingerprint, so project publication
/// recomputes identity from the rule fields rather than trusting the stored key.
///
/// ```compile_fail
/// use stillwind_source::Fingerprint;
/// use stillwind_compiler::RuleKey;
/// let rule: RuleKey = Fingerprint::of(".x{}");
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct RuleKey(
    /// Fingerprint of rule serialization, placement, and support dependencies; direct
    /// construction does not verify it.
    pub Fingerprint,
);

/// One independently owned CSS rule.
///
/// The rule is kept as prelude plus body so a consumer can reorder, deduplicate, or
/// re-serialize it without parsing CSS. `prelude` preserves the authored serialization
/// whitespace, so [`GeneratedRule::to_css`] reproduces the emitter's bytes exactly for
/// a rule produced by `ModuleCssArtifact::from_css`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedRule {
    /// True for an emitter-classified keyframes definition destined for the keyframes
    /// slot; false for a rule that stays in the utilities contribution.
    pub keyframes: bool,
    /// Custom-property names requiring registration, independent of CSS references.
    /// Compiler-produced lists are sorted and unique; their order participates in the key.
    pub required_properties: Vec<String>,
    /// Content identity of serialization, placement, and support dependencies.
    pub key: RuleKey,
    /// Selector or at-rule header, including authored serialization whitespace.
    pub prelude: String,
    /// Declaration or nested-rule text without the outer braces. With `None`, `prelude`
    /// contains the entire serialization, including any statement terminator.
    pub body: Option<String>,
}

impl GeneratedRule {
    /// Computes identity from CSS serialization, placement, and dependency names.
    ///
    /// Ignores the stored `key` and does not mutate the rule. Dependency order is
    /// significant to this hash, so constructors must normalize equivalent lists if
    /// they need those lists to share an identity.
    pub fn content_key(&self) -> RuleKey {
        if !self.keyframes && self.required_properties.is_empty() {
            return RuleKey(Fingerprint::of(self.to_css()));
        }
        RuleKey(Fingerprint::of(format!(
            "{:?}:{}:{}",
            self.required_properties,
            self.keyframes,
            self.to_css()
        )))
    }

    /// Appends this rule and its dependencies to a producer's mapped contributions.
    ///
    /// Ordinary rules extend `output.css`; keyframes definitions append a separate
    /// entry to `output.keyframes`. Existing contribution order is preserved within
    /// each destination. `origin` is passed to [`MappedStylesheet::text`]; an unusable
    /// span leaves the emitted rule unmapped. Dependency names are not deduplicated.
    pub fn append_to(
        &self,
        output: &mut MappedStylesheet,
        origin: Option<(Arc<SourceFile>, Span)>,
    ) {
        output
            .required_properties
            .extend(self.required_properties.iter().cloned());
        if self.keyframes {
            let mut frames = MappedStylesheet::default();
            frames.text(&self.to_css(), origin);
            output.keyframes.push(frames);
        } else {
            output.text(&self.to_css(), origin);
        }
    }

    /// Serializes the stored prelude and optional brace-delimited body.
    ///
    /// With no body, returns the prelude unchanged; it does not add a semicolon.
    /// Placement and registration metadata are not included. Use [`Self::append_to`]
    /// when the result will be assembled through output slots.
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

impl ModuleCssArtifact {
    /// Splits contributions into independently owned rules in contribution order.
    ///
    /// Every rule inherits its contribution's placement and dependencies. Dependency
    /// names are sorted and deduplicated before computing the rule's content key.
    /// The input contributions are not modified.
    ///
    /// # Errors
    ///
    /// Returns the UTF-8 byte offset of an invalid block within the offending
    /// contribution. This parser result has no reporting classification: module
    /// compilation treats it as fatal, while candidate queries attach a lint finding.
    pub(crate) fn from_contributions(parts: &[CssContribution]) -> Result<Self, usize> {
        let mut output = Self::default();
        for part in parts {
            let mut artifact = Self::from_css(&part.css)?;
            for rule in &mut artifact.rules {
                rule.keyframes = part.keyframes;
                rule.required_properties
                    .clone_from(&part.required_properties);
                rule.required_properties.sort();
                rule.required_properties.dedup();
                rule.key = rule.content_key();
            }
            output.rules.extend(artifact.rules);
        }
        Ok(output)
    }

    /// Builds producer contributions for a stylesheet renderer's mapped entry point.
    ///
    /// Preserves order within the rule and keyframe destinations and retains explicit
    /// dependencies. Source mappings are empty because this artifact has no source
    /// buffers; callers with origins can use [`GeneratedRule::append_to`] instead.
    pub fn mapped(&self) -> MappedStylesheet {
        let mut output = MappedStylesheet::default();
        for rule in &self.rules {
            rule.append_to(&mut output, None);
        }
        output
    }

    /// Concatenates rule serializations in stored order for inspecting unassembled CSS.
    ///
    /// The string omits placement and dependency metadata. Use [`Self::mapped`] for
    /// template assembly rather than passing this text back to a raw-CSS renderer.
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
    pub(crate) fn from_css(css: &str) -> Result<Self, usize> {
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
                    .map_err(|_| body_start)?;
                let end = parser.position().byte_index();
                rules.push(GeneratedRule {
                    keyframes: false,
                    required_properties: Vec::new(),
                    key: RuleKey(Fingerprint::of(&css[start..end])),
                    prelude: css[start..body_start - 1].into(),
                    body: Some(css[body_start..body_end].into()),
                });
                start = end;
            } else if matches!(token, Token::Semicolon) {
                let end = parser.position().byte_index();
                rules.push(GeneratedRule {
                    keyframes: false,
                    required_properties: Vec::new(),
                    key: RuleKey(Fingerprint::of(&css[start..end])),
                    prelude: css[start..end].into(),
                    body: None,
                });
                start = end;
            }
        }
        if start < css.len() {
            rules.push(GeneratedRule {
                keyframes: false,
                required_properties: Vec::new(),
                key: RuleKey(Fingerprint::of(&css[start..])),
                prelude: css[start..].into(),
                body: None,
            });
        }
        Ok(Self { rules })
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
