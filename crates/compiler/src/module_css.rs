//! Producer CSS occurrences with content identities and support dependencies.
//!
//! [`ModuleCssArtifact::mapped`] supplies template input. [`GeneratedRule::append_to`]
//! attaches source attribution when the caller has an originating span.

use std::sync::Arc;

use lean_string::{LeanStr, ToLeanString};

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
    keyframes: bool,
    /// Registration dependencies, sorted and unique after construction.
    required_properties: Vec<LeanStr>,
    /// CSS syntax and its occurrence range in the original contribution.
    syntax: CssRule,
    /// Content identity computed after registration dependencies are normalized.
    key: RuleKey,
}

impl GeneratedRule {
    /// Creates one occurrence with its template placement and registration dependencies.
    /// `syntax` supplies CSS tokens and the original contribution range. `keyframes` selects
    /// the keyframes slot. Registration names are sorted and deduplicated; the renderer
    /// validates their CSS spelling and whether the template can emit their registrations.
    pub fn new(syntax: CssRule, keyframes: bool, mut required_properties: Vec<LeanStr>) -> Self {
        required_properties.sort();
        required_properties.dedup();
        let css = syntax.to_css();
        let key = if !keyframes && required_properties.is_empty() {
            RuleKey(Fingerprint::of(css))
        } else {
            let mut content = format_args!("{keyframes}\0").to_lean_string();
            for property in &required_properties {
                content.push_str(&property.len().to_lean_string());
                content.push(':');
                content.push_str(property);
            }
            content.push('\0');
            content.push_str(&css);
            RuleKey(Fingerprint::of(content))
        };
        Self {
            keyframes,
            required_properties,
            syntax,
            key,
        }
    }

    /// Returns whether this occurrence belongs in the keyframes slot.
    pub fn keyframes(&self) -> bool {
        self.keyframes
    }

    /// Returns registration names in lexical order without duplicates.
    pub fn required_properties(&self) -> &[LeanStr] {
        &self.required_properties
    }

    /// Borrows CSS tokens and their original contribution range without allowing mutation.
    pub fn syntax(&self) -> &CssRule {
        &self.syntax
    }

    /// Returns the stored identity of the CSS, template slot, and registration dependencies.
    pub fn content_key(&self) -> RuleKey {
        self.key.clone()
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
    pub fn to_css(&self) -> LeanStr {
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
            artifact
                .rules
                .extend(CssRule::parse(&part.css)?.into_iter().map(|syntax| {
                    GeneratedRule::new(syntax, part.keyframes, part.required_properties.clone())
                }));
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
        self.rules.iter().fold(String::new(), |mut output, rule| {
            output.push_str(&rule.to_css());
            output
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{CssRule, GeneratedRule};

    #[test]
    fn content_identity_tracks_css_placement_and_registration_set() {
        let red = GeneratedRule::new(
            CssRule::parse(".x{color:red}").unwrap().remove(0),
            false,
            Vec::new(),
        );
        let blue = GeneratedRule::new(
            CssRule::parse(".x{color:blue}").unwrap().remove(0),
            false,
            Vec::new(),
        );
        assert_ne!(red.content_key(), blue.content_key());
        let frames = GeneratedRule::new(blue.syntax().clone(), true, Vec::new());
        assert_ne!(frames.content_key(), blue.content_key());
        let registered = GeneratedRule::new(
            blue.syntax().clone(),
            true,
            vec!["--b".into(), "--a".into(), "--b".into()],
        );
        assert_eq!(registered.required_properties(), ["--a", "--b"]);
        assert!(registered.keyframes());
        assert_ne!(registered.content_key(), frames.content_key());
        let mut syntax = blue.syntax().clone();
        syntax.range = 12..24;
        let equivalent = GeneratedRule::new(syntax, true, vec!["--a".into(), "--b".into()]);
        assert_eq!(equivalent.content_key(), registered.content_key());
    }
}
