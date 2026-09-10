//! Explicit CSS fixtures for compiler tests that exercise the standard package.
//! This module is included only by tests, never by a production host.

use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub fn source(
    id: stillwind_source::FileId,
    path: &str,
    css: impl AsRef<str>,
) -> Result<stillwind_source::SourceFile, stillwind_source::SourceError> {
    stillwind_source::SourceFile::new(id, path, with_defaults(css.as_ref()))
}

pub fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/core")
}

pub fn expand(css: &str) -> String {
    expand_from(css, &package_root())
}

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

pub fn with_defaults(css: &str) -> String {
    static DEFAULTS: OnceLock<String> = OnceLock::new();
    if css.contains("\"stillwind") || css.contains("'stillwind") {
        return expand(css);
    }
    let defaults = DEFAULTS.get_or_init(|| expand("@import \"stillwind\";"));
    format!("{defaults}\n{css}")
}
