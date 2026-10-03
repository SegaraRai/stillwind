//! Explicit CSS fixtures for compiler tests that exercise the standard package. This
//! module is used by test and example targets rather than production hosts.
//!
//! The helpers read the real `packages/core` CSS from disk instead of embedding copies,
//! so a fixture cannot drift from the package it claims to use. `@import` expansion is
//! intentionally minimal: only `stillwind`, `stillwind/…` and relative paths inside the
//! fixture directory are supported, and an unsupported tail asserts instead of being
//! ignored.

use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use stillwind_source::{FileId, SourceError, SourceFile};

/// Builds a source buffer with `id` from CSS that has the standard package prepended.
///
/// CSS that imports `stillwind` itself is expanded instead, so a fixture can choose
/// between the defaults and an explicit import list.
///
/// # Errors
///
/// Returns [`SourceError::TooLarge`] if the expanded CSS exceeds the 32-bit source limit.
///
/// # Panics
///
/// Panics if [`with_defaults`] cannot read an imported file or expand its import tail.
pub fn source(id: FileId, css: impl AsRef<str>) -> Result<SourceFile, SourceError> {
    SourceFile::new(id, None, None, with_defaults(css.as_ref()))
}

/// Returns the standard CSS package's path relative to this crate's manifest.
pub fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/core")
}

/// Expands `@import` statements against the standard package.
///
/// Returns CSS with imports from the standard package and existing relative files
/// replaced by their contents. Unrecognized imports are copied unchanged.
///
/// # Panics
///
/// Panics if an imported file cannot be read or a supported import has a nonempty
/// tail other than `layer(...)`.
pub fn expand(css: &str) -> String {
    expand_from(css, &package_root())
}

/// Expands imports relative to `directory`, recursively.
///
/// An import that cannot be handled is copied through unchanged, so the caller decides
/// what to do with it; a `layer(...)` tail wraps the imported body.
fn expand_from(css: &str, directory: &Path) -> String {
    let mut output = String::new();
    let mut remaining = css;
    while let Some(start) = remaining.find("@import ") {
        output.push_str(&remaining[..start]);
        let rule = &remaining[start + 8..];
        let Some(end) = rule.find(';') else { break };
        let import = rule[..end].trim();
        let quote = import.chars().next().unwrap();
        let Some(close) = import[1..].find(quote).map(|close| close + 1) else {
            break;
        };
        let name = &import[1..close];
        let tail = import[close + 1..].trim();
        let path = if name == "stillwind" {
            Some(package_root().join("index.css"))
        } else if let Some(name) = name.strip_prefix("stillwind/") {
            Some(package_root().join("css").join(name))
        } else if name.starts_with("./") && directory.join(name).is_file() {
            Some(directory.join(name))
        } else {
            None
        };
        if let Some(path) = path {
            let source = std::fs::read_to_string(&path).unwrap();
            let body = expand_from(&source, path.parent().unwrap());
            if let Some(layer) = tail
                .strip_prefix("layer(")
                .and_then(|tail| tail.strip_suffix(')'))
            {
                write!(output, "@layer {layer}{{{body}}}").unwrap();
            } else {
                assert!(
                    tail.is_empty(),
                    "fixture must use a supported import: {import}"
                );
                output.push_str(&body);
            }
        } else {
            output.push_str(&remaining[start..=start + 8 + end]);
        }
        remaining = &rule[end + 1..];
    }
    output.push_str(remaining);
    output
}

/// Prepends the expanded default package to CSS that does not import it.
///
/// The defaults are expanded once and cached, because every fixture would otherwise
/// re-read the package from disk.
///
/// Returns expanded `css` when it contains a quoted name starting with `stillwind`.
/// Otherwise, returns the expanded default package followed by a newline and `css`.
///
/// # Panics
///
/// Panics if [`expand`] cannot read an imported file or expand its import tail.
pub fn with_defaults(css: &str) -> String {
    static DEFAULTS: OnceLock<String> = OnceLock::new();
    if css.contains("\"stillwind") || css.contains("'stillwind") {
        return expand(css);
    }
    let defaults = DEFAULTS.get_or_init(|| expand("@import \"stillwind\";"));
    format!("{defaults}\n{css}")
}
