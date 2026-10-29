//! Preserved CSS notices and their compile-time declaration blocks.
//!
//! [`CssBanner`] validates one complete important comment. [`banner_blocks`] extracts
//! declarations without reprinting their comments, and [`without_banner_blocks`] masks
//! those declarations while retaining UTF-8 source offsets.

use std::ops::Range;

use lean_string::LeanStr;

use crate::structure::{CssNode, CssNodeKind, parse_structure};

/// A malformed banner comment or declaration, located in the supplied CSS.
#[derive(Clone, Debug, thiserror::Error)]
#[error("invalid CSS banner at byte {0}: expected one /*! ... */ comment")]
pub struct CssBannerError(
    /// UTF-8 byte offset of the invalid declaration in the supplied CSS.
    pub usize,
);

/// One complete important comment whose original text is retained in output.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CssBanner(LeanStr);

impl CssBanner {
    /// Validates `comment` without trimming or rewriting it.
    ///
    /// # Errors
    ///
    /// Returns an error unless the whole string is a single closed important comment.
    pub fn new(comment: &str) -> Result<Self, CssBannerError> {
        if !comment.starts_with("/*!")
            || !comment.ends_with("*/")
            || comment[3..comment.len() - 2].contains("*/")
        {
            return Err(CssBannerError(0));
        }
        Ok(Self(comment.into()))
    }

    /// Returns the complete comment, including delimiters and original whitespace.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One `@stillwind-banner` declaration and its complete UTF-8 byte range.
#[derive(Clone, Debug)]
pub struct CssBannerBlock {
    /// Complete declaration range in the input CSS.
    pub range: Range<usize>,
    /// Preserved important comment inside the declaration.
    pub banner: CssBanner,
}

/// Extracts banner declarations at every rule depth, ignoring strings and comments.
///
/// # Errors
///
/// Returns an error for a bodyless declaration, a prelude, or a body that contains
/// anything besides whitespace and one important comment.
pub fn banner_blocks(css: &str) -> Result<Vec<CssBannerBlock>, CssBannerError> {
    fn collect(
        css: &str,
        nodes: &[CssNode],
        output: &mut Vec<CssBannerBlock>,
    ) -> Result<(), CssBannerError> {
        for node in nodes {
            if let CssNodeKind::AtRule {
                name,
                prelude,
                block,
            } = &node.kind
                && name == "stillwind-banner"
            {
                let Some(block) = block.as_ref().filter(|block| block.close.is_some()) else {
                    return Err(CssBannerError(node.range.start));
                };
                if !css[prelude.clone()].trim().is_empty() {
                    return Err(CssBannerError(node.range.start));
                }
                let banner = CssBanner::new(css[block.contents.clone()].trim())
                    .map_err(|_| CssBannerError(node.range.start))?;
                output.push(CssBannerBlock {
                    range: node.range.clone(),
                    banner,
                });
            } else {
                collect(css, node.children(), output)?;
            }
        }
        Ok(())
    }
    if !css.contains("@stillwind-banner") {
        return Ok(Vec::new());
    }
    let mut output = Vec::new();
    collect(css, &parse_structure(css), &mut output)?;
    Ok(output)
}

/// Masks valid banner blocks with ASCII spaces, preserving newlines and byte offsets.
///
/// # Errors
///
/// Returns the first invalid declaration without producing partial text.
pub fn without_banner_blocks(css: &str) -> Result<String, CssBannerError> {
    let blocks = banner_blocks(css)?;
    let mut masked = css.to_owned();
    for block in blocks.into_iter().rev() {
        let spaces: String = css[block.range.clone()]
            .bytes()
            .map(|byte| {
                if matches!(byte, b'\n' | b'\r') {
                    char::from(byte)
                } else {
                    ' '
                }
            })
            .collect();
        masked.replace_range(block.range, &spaces);
    }
    Ok(masked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_comment_text_and_source_offsets() {
        let comment = "/*! 日本語 🧭\n  Copyright */";
        let css = format!("@theme {{ @stillwind-banner {{ {comment} }} --x: red; }}");
        let blocks = banner_blocks(&css).unwrap();
        assert_eq!(blocks[0].banner.as_str(), comment);
        let masked = without_banner_blocks(&css).unwrap();
        assert_eq!(masked.len(), css.len());
        assert_eq!(masked.find("--x"), css.find("--x"));
        assert_eq!(masked.lines().count(), css.lines().count());
    }

    #[test]
    fn rejects_non_comment_content_and_ignores_quoted_directives() {
        for css in [
            "@stillwind-banner;",
            "@stillwind-banner",
            "@stillwind-banner { /*! notice */",
            "@stillwind-banner extra { /*! notice */ }",
            "@stillwind-banner { /*! one */ /*! two */ }",
            "@stillwind-banner { color: red; }",
        ] {
            assert!(banner_blocks(css).is_err(), "{css}");
        }
        assert!(
            banner_blocks("a{content:'@stillwind-banner;'}")
                .unwrap()
                .is_empty()
        );
    }
}
