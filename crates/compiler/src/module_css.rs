//! Producer CSS occurrences with content identities and support dependencies.
//!
//! [`ModuleCssArtifact::mapped`] supplies template input. [`GeneratedRule::append_to`]
//! attaches source attribution when the caller has an originating span.

use std::sync::Arc;

use stillwind_css::CssRule;
use stillwind_source::{Fingerprint, SourceFile, Span};

use crate::{CssContribution, MappedStylesheet};

/// Content identity, excluding occurrence and producer identity.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct RuleKey(pub Fingerprint);

/// A generated rule and the support required wherever it is used.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedRule {
    /// Whether this occurrence belongs in the keyframes slot.
    pub keyframes: bool,
    /// Registration dependencies, sorted and unique for compiler artifacts.
    pub required_properties: Vec<String>,
    /// Fingerprint captured at construction. Call [`Self::content_key`] after mutations.
    pub key: RuleKey,
    /// CSS syntax and its occurrence range in the original contribution.
    pub syntax: CssRule,
}

impl GeneratedRule {
    /// Hashes the current CSS text, template slot and registration dependencies.
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

    /// Appends an occurrence to its template destination with source attribution.
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

    /// Serializes tokens, excluding support metadata.
    pub fn to_css(&self) -> String {
        self.syntax.to_css()
    }
}

/// Ordered occurrences owned by one producer.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModuleCssArtifact {
    /// Reference order before linking and optimization.
    pub rules: Vec<GeneratedRule>,
}

impl ModuleCssArtifact {
    pub(crate) fn from_contributions(parts: &[CssContribution]) -> Result<Self, usize> {
        let mut artifact = Self::default();
        for part in parts {
            let mut parsed = Self::from_css(&part.css)?;
            for rule in &mut parsed.rules {
                rule.keyframes = part.keyframes;
                rule.required_properties
                    .clone_from(&part.required_properties);
                rule.required_properties.sort();
                rule.required_properties.dedup();
                rule.key = rule.content_key();
            }
            artifact.rules.extend(parsed.rules);
        }
        Ok(artifact)
    }

    /// Builds unoptimized template input without source mappings, preserving occurrences.
    pub fn mapped(&self) -> MappedStylesheet {
        let mut mapped = MappedStylesheet::default();
        for rule in &self.rules {
            rule.append_to(&mut mapped, None);
        }
        mapped
    }

    /// Serializes reference order without expanding template slots.
    pub fn to_css(&self) -> String {
        self.rules.iter().map(GeneratedRule::to_css).collect()
    }

    pub(crate) fn from_css(css: &str) -> Result<Self, usize> {
        Ok(Self {
            rules: CssRule::parse(css)?
                .into_iter()
                .map(|node| {
                    let mut rule = GeneratedRule {
                        keyframes: false,
                        required_properties: Vec::new(),
                        key: RuleKey(Fingerprint::of("")),
                        syntax: node,
                    };
                    rule.key = rule.content_key();
                    rule
                })
                .collect(),
        })
    }
}
