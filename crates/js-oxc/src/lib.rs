//! Oxc-backed JS/TS/JSX parsing and import-bound macro extraction.

mod parser;

pub use parser::{ParsedJavaScript, parse, parse_with_type_facts};
