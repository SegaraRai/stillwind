//! Names authored CSS already uses, reserved before any generated name is allocated.
//!
//! [`ReservedNames`] collects what a stylesheet already declares in the namespaces the
//! compiler allocates names in. Generated identifiers are checked against these, so a
//! generated name can never silently change the meaning of an authored declaration or
//! reference. Names are stored in the canonical spelling the pools compare: CSS escapes
//! are decoded, so `--a\:b` and an unescaped `--a:b` are one reservation.
//!
//! The scan is deliberately lexical and conservative: it reads declarations, `var()`
//! references, `@layer` statements and blocks, and `@keyframes` names. It does not
//! resolve imports, which the caller does before scanning, and it does not attempt to
//! interpret values.
//!
//! TODO: CSS a module authors rather than imports is not scanned here. The compiler
//! receives its declarations already resolved, so that text only exists on the host
//! side; reserving it requires the host to supply those names with the analysis.

use std::collections::BTreeSet;

/// Names one document already uses, by CSS namespace.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReservedNames {
    /// Custom property names, including the leading `--`, from declarations,
    /// `var()` references, and `@property` registrations.
    pub custom_properties: BTreeSet<String>,
    /// `@layer` names, both statement and block forms.
    pub layers: BTreeSet<String>,
    /// `@keyframes` names.
    pub keyframes: BTreeSet<String>,
}

impl ReservedNames {
    /// Whether nothing was reserved.
    pub fn is_empty(&self) -> bool {
        self.custom_properties.is_empty() && self.layers.is_empty() && self.keyframes.is_empty()
    }

    /// Reserves every name `css` uses.
    ///
    /// Repeated scans accumulate; a name already reserved is kept once.
    pub fn collect(&mut self, css: &str) {
        let bytes = css.as_bytes();
        let mut cursor = 0;
        while cursor < bytes.len() {
            let rest = &css[cursor..];
            if rest.starts_with("/*") {
                cursor += rest.find("*/").map_or(rest.len(), |end| end + 2);
                continue;
            }
            if bytes[cursor] == b'\'' || bytes[cursor] == b'"' {
                cursor = skip_string(css, cursor);
                continue;
            }
            if rest.starts_with("--") {
                let (name, next) = read_custom_property(css, cursor);
                self.custom_properties.insert(name);
                cursor = next;
                continue;
            }
            if let Some((keyword, next)) = at_keyword(css, cursor) {
                match keyword.as_str() {
                    "layer" => {
                        let (names, next) = read_layer_names(css, next);
                        self.layers.extend(names);
                        cursor = next;
                    }
                    "keyframes" => {
                        let (name, next) = read_ident(css, next);
                        if !name.is_empty() {
                            self.keyframes.insert(name);
                        }
                        cursor = next;
                    }
                    "property" => {
                        // A registration names the property it describes.
                        let (name, next) = read_custom_property(css, next);
                        if name.starts_with("--") {
                            self.custom_properties.insert(name);
                        }
                        cursor = next;
                    }
                    _ => cursor = next,
                }
                continue;
            }
            cursor += next_char_len(css, cursor);
        }
    }
}

/// Reads the `@name` at `cursor`, returning the lowercase keyword and the next offset.
fn at_keyword(css: &str, cursor: usize) -> Option<(String, usize)> {
    let rest = &css[cursor..];
    if !rest.starts_with('@') {
        return None;
    }
    let (name, next) = read_ident(css, cursor + 1);
    let start = css[cursor..next].len() + cursor;
    let mut end = next;
    while end < css.len() && css.as_bytes()[end].is_ascii_whitespace() {
        end += 1;
    }
    let _ = start;
    Some((name.to_ascii_lowercase(), end))
}

/// Reads one identifier, decoding escapes, and returns it with the next offset.
fn read_ident(css: &str, cursor: usize) -> (String, usize) {
    let mut name = String::new();
    let mut offset = cursor;
    let bytes = css.as_bytes();
    while offset < bytes.len() {
        let Some(character) = css[offset..].chars().next() else {
            break;
        };
        if character == '\\' {
            let (decoded, next) = decode_escape(css, offset);
            name.push_str(&decoded);
            offset = next;
            continue;
        }
        if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
            name.push(character);
            offset += character.len_utf8();
            continue;
        }
        if !name.is_empty() && character as u32 > 0x7f && !character.is_whitespace() {
            // Non-ASCII identifier characters stay verbatim; CSS does not require
            // escaping them, so they compare equal to an authored spelling.
            name.push(character);
            offset += character.len_utf8();
            continue;
        }
        break;
    }
    (name, offset)
}

/// Reads a custom property name including its `--`, or an empty string.
fn read_custom_property(css: &str, cursor: usize) -> (String, usize) {
    let (name, next) = read_ident(css, cursor);
    if name.starts_with("--") && name.len() > 2 {
        (name, next)
    } else {
        (String::new(), next)
    }
}

/// Reads the comma-separated layer names of a statement or block.
fn read_layer_names(css: &str, cursor: usize) -> (Vec<String>, usize) {
    let mut names = Vec::new();
    let mut offset = cursor;
    loop {
        while offset < css.len() && css.as_bytes()[offset].is_ascii_whitespace() {
            offset += 1;
        }
        if offset < css.len() && css[offset..].starts_with("/*") {
            let rest = &css[offset..];
            offset += rest.find("*/").map_or(rest.len(), |end| end + 2);
            continue;
        }
        let (name, next) = read_ident(css, offset);
        if name.is_empty() {
            break;
        }
        names.push(name);
        offset = next;
        while offset < css.len() && css.as_bytes()[offset].is_ascii_whitespace() {
            offset += 1;
        }
        if offset < css.len() && css.as_bytes()[offset] == b',' {
            offset += 1;
            continue;
        }
        break;
    }
    (names, offset)
}

/// Decodes one CSS escape, returning the text it stands for and the next offset.
fn decode_escape(css: &str, cursor: usize) -> (String, usize) {
    let bytes = css.as_bytes();
    let mut offset = cursor + 1;
    if offset >= bytes.len() {
        return (String::new(), offset);
    }
    if bytes[offset].is_ascii_hexdigit() {
        let start = offset;
        while offset < bytes.len() && offset - start < 6 && bytes[offset].is_ascii_hexdigit() {
            offset += 1;
        }
        let value = u32::from_str_radix(&css[start..offset], 16).unwrap_or(0xfffd);
        while offset < bytes.len() && bytes[offset].is_ascii_whitespace() {
            offset += 1;
        }
        let character = char::from_u32(value).unwrap_or('\u{fffd}');
        return (character.to_string(), offset);
    }
    match css[offset..].chars().next() {
        Some(character) => (character.to_string(), offset + character.len_utf8()),
        None => (String::new(), offset),
    }
}

/// Skips a quoted string, returning the offset just past its closing quote.
fn skip_string(css: &str, cursor: usize) -> usize {
    let bytes = css.as_bytes();
    let quote = bytes[cursor];
    let mut offset = cursor + 1;
    while offset < bytes.len() {
        match bytes[offset] {
            b'\\' => offset += 2,
            character if character == quote => return offset + 1,
            _ => offset += 1,
        }
    }
    offset
}

/// Byte length of the character at `cursor`.
fn next_char_len(css: &str, cursor: usize) -> usize {
    css[cursor..].chars().next().map_or(1, char::len_utf8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_declarations_references_and_registrations_once() {
        let mut reserved = ReservedNames::default();
        reserved.collect(
            ":root{--sw-user:1;--sw-user:2;} .x{color:var(--sw-user);width:var(--w, 1px);}",
        );
        assert_eq!(
            reserved
                .custom_properties
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["--sw-user".to_owned(), "--w".to_owned()]
        );
    }

    #[test]
    fn collects_layers_statement_blocks_and_keyframes() {
        let mut reserved = ReservedNames::default();
        reserved.collect("@layer base, theme;@layer sw-anon-1{@keyframes spin{to{opacity:0}}}");
        assert_eq!(
            reserved.layers.iter().cloned().collect::<Vec<_>>(),
            vec![
                "base".to_owned(),
                "sw-anon-1".to_owned(),
                "theme".to_owned()
            ]
        );
        assert_eq!(
            reserved.keyframes.iter().cloned().collect::<Vec<_>>(),
            vec!["spin".to_owned()]
        );
    }

    #[test]
    fn decodes_escapes_so_reservations_compare_in_one_spelling() {
        let mut reserved = ReservedNames::default();
        reserved.collect(".x{--a\\:b:1;} .y{color:var(--a\\:b);}");
        assert_eq!(
            reserved
                .custom_properties
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["--a:b".to_owned()]
        );
    }

    #[test]
    fn ignores_comments_strings_and_spaced_hyphens() {
        let mut reserved = ReservedNames::default();
        reserved
            .collect("/* --sw-comment:1; */ .x{content:\"--sw-string\";margin:calc(1px - -2px);}");
        assert!(reserved.is_empty(), "{reserved:?}");
    }

    #[test]
    fn collects_property_registrations() {
        let mut reserved = ReservedNames::default();
        reserved.collect("@property --sw-registered{syntax:'<length>';inherits:false;}");
        assert_eq!(
            reserved
                .custom_properties
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["--sw-registered".to_owned()]
        );
    }
}
