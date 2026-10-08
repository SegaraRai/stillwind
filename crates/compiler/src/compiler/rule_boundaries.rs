//! Assigns support-query identities after conditional rules reach emission order.
//!
//! Renderers mark already-guarded subtrees with [`protect`]. After directive expansion
//! and rule ordering, feed every contribution of one owner to the same
//! [`RuleBoundaries`] through [`RuleBoundaries::render`] or
//! [`RuleBoundaries::render_mapped`]. Owners span an entire style call or an outer
//! authored rule, including its children.
//!
//! Unprotected declarations also participate in conflict detection. Protected rules
//! share a support-query name until a subsequent declaration batch interferes with the
//! current interval. Selector matching and conditional-value lowering belong to the
//! callers. Input must be balanced compiler-rendered CSS, with internal markers spelled
//! exactly as [`protect`] emits them.

use std::ops::Range;

use cssparser::{Parser, Token};

use stillwind_css::properties_interfere;
use stillwind_diagnostics::CompileDiagnostic;

use super::conditionals::{consume, declaration_values};
use crate::MappedStylesheet;
use crate::identifiers::{ConditionContext, ConditionRole, IdentifierContext};

const MARKER: &str = "@-stillwind-rule-boundary";

/// Wraps an already-guarded rule subtree in an internal marker for deferred isolation.
///
/// `rule` must be balanced CSS whose conditional declarations already contain their
/// false branches. Its text is retained verbatim. The returned wrapper must pass
/// through [`RuleBoundaries`] before publication, after all directives are expanded.
pub(super) fn protect(rule: &str) -> String {
    format!("{MARKER}{{{rule}}}")
}

/// Accumulated conflict state for allocating one owner's rule-merge barriers.
///
/// Reuse this state across the owner's contributions in final emission order. A
/// failed render can advance it, so retrying an owner requires fresh state and replay
/// of its earlier contributions.
pub(crate) struct RuleBoundaries<'a> {
    /// Stable identity distinct from every other owner sharing the name pool.
    owner: String,
    /// Zero-based interval included in the name hash, advanced once per conflicting
    /// batch.
    interval: usize,
    /// Current interval's decoded property names and trimmed, otherwise verbatim
    /// values.
    declarations: Vec<(String, String)>,
    identifiers: &'a IdentifierContext,
}

impl<'a> RuleBoundaries<'a> {
    /// Starts an empty interval for `owner` using the compilation's name allocator.
    ///
    /// `owner` must identify the same style call or authored rule on repeated renders
    /// and distinguish it from other owners. Names are allocated only when a protected
    /// declaration batch is reached.
    pub(crate) fn new(owner: String, identifiers: &'a IdentifierContext) -> Self {
        Self {
            owner,
            interval: 0,
            declarations: Vec::new(),
            identifiers,
        }
    }

    /// Replaces a contribution's internal markers with support queries in output order.
    ///
    /// `css` must satisfy the module's rendered-CSS contract and follow all
    /// contributions already consumed by this state. Returns rewritten CSS while
    /// retaining the final interval and declarations for the next contribution.
    /// Unmarked text is retained.
    ///
    /// # Errors
    ///
    /// Returns a naming failure without returning partial CSS. Conflict state and the
    /// shared identifier allocator may already have advanced.
    ///
    /// # Panics
    ///
    /// Malformed block boundaries or noncanonical internal markers can produce invalid
    /// edit offsets and panic during slicing.
    pub(crate) fn render(&mut self, css: &str) -> Result<String, CompileDiagnostic> {
        let edits = self.edits(css)?;
        let mut output = String::new();
        let mut cursor = 0;
        for (range, replacement) in edits {
            output.push_str(&css[cursor..range.start]);
            output.push_str(&replacement);
            cursor = range.end;
        }
        output.push_str(&css[cursor..]);
        Ok(output)
    }

    /// Finalizes a mapped contribution under the same input and ordering contract as
    /// [`Self::render`].
    ///
    /// Rewrites `css.css` and rebases its mappings and template segments through
    /// [`MappedStylesheet::replace_ranges`]. Pending keyframes and requirements remain
    /// attached to the contribution. Conflict state is retained for the next call.
    ///
    /// # Errors
    ///
    /// Returns a naming failure before changing `css`. Conflict state and the shared
    /// identifier allocator may already have advanced.
    ///
    /// # Panics
    ///
    /// Invalid rendered CSS or mapping coordinates can panic during edit application.
    pub(super) fn render_mapped(
        &mut self,
        css: &mut MappedStylesheet,
    ) -> Result<(), CompileDiagnostic> {
        let edits = self.edits(&css.css)?;
        css.replace_ranges(&edits);
        Ok(())
    }

    /// Consumes rendered `css` into ordered byte edits, advancing this owner's state.
    /// All ranges address the unchanged input. Equal-start edits retain traversal
    /// order, so a closing insertion precedes deletion of an enclosing marker at that
    /// offset.
    ///
    /// # Errors
    ///
    /// Returns a naming failure with any earlier state changes retained.
    fn edits(&mut self, css: &str) -> Result<Vec<(Range<usize>, String)>, CompileDiagnostic> {
        let mut edits = Vec::new();
        self.walk(css, 0..css.len(), false, &mut edits)?;
        edits.sort_by_key(|(range, _)| range.start);
        Ok(edits)
    }

    /// Accumulates one rule's declaration batch from `css`, including unprotected
    /// rules. A conflict with an earlier batch advances the interval once and clears
    /// its history before adding this batch. Declarations within the batch keep their
    /// existing order. Equal custom-property names and values are exempt only when the
    /// value lacks the literal substring `revert-rule`, allowing identical generated
    /// helpers to coexist.
    fn observe(&mut self, css: &str) {
        let declarations = declaration_values(css)
            .into_iter()
            .filter_map(|(property, value)| {
                let mut parser = Parser::new(&css[property]);
                let Ok(Token::Ident(property)) = parser.next() else {
                    return None;
                };
                Some((property.to_string(), css[value].trim().to_owned()))
            })
            .collect::<Vec<_>>();
        if declarations.iter().any(|(property, value)| {
            self.declarations.iter().any(|(previous, previous_value)| {
                properties_interfere(property, previous)
                    && !(property.starts_with("--")
                        && property == previous
                        && value == previous_value
                        && !value.contains("revert-rule"))
            })
        }) {
            self.interval += 1;
            self.declarations.clear();
        }
        self.declarations.extend(declarations);
    }

    /// Allocates the current owner/interval name and returns an opening `@supports`
    /// block. Call after observing the declarations it protects. The shared allocator
    /// memoizes the name, while this owner's conflict state stays unchanged.
    ///
    /// # Errors
    ///
    /// Returns the identifier allocator's failure for the rule-boundary name.
    fn support(&self) -> Result<String, CompileDiagnostic> {
        let name = self.identifiers.condition_helper(&ConditionContext {
            role: ConditionRole::Rule,
            text: format!("{}\0{}", self.owner, self.interval),
        })?;
        // `x` is a valid SVG geometry CSS property, allowing a one-character feature
        // probe. The gate preserves base declarations in browsers without `revert-rule`:
        // selecting it through `var()` would otherwise become invalid at computed-value
        // time and use the initial or inherited value instead of recascading.
        // The custom-property condition only distinguishes queries to prevent minifiers
        // from merging conflicting rules. It never declares a property on an element.
        Ok(format!(
            "@supports (x:revert-rule) and ({}:){{",
            name.property()
        ))
    }

    /// Visits one complete rule-list or declaration-list range in rendered `css`.
    /// `range` uses UTF-8 byte offsets into the full input. `protected` carries an
    /// enclosing marker's scope through nested blocks. Appends unsorted edits in
    /// traversal order and advances conflict state for both protected and unprotected
    /// declarations.
    ///
    /// # Errors
    ///
    /// Returns a naming failure with prior edits and state changes retained.
    ///
    /// # Panics
    ///
    /// Panics on invalid slice boundaries. Unterminated blocks or noncanonical markers
    /// can also invalidate the offsets used during traversal.
    fn walk(
        &mut self,
        css: &str,
        range: Range<usize>,
        protected: bool,
        edits: &mut Vec<(Range<usize>, String)>,
    ) -> Result<(), CompileDiagnostic> {
        let children = blocks(css, range.clone());
        let mut cursor = range.start;
        for block in children {
            self.declarations(css, cursor..block.header.start, protected, edits)?;
            if let Some(marker) = block.marker {
                edits.push((marker..block.body.start, String::new()));
                self.walk(css, block.body.clone(), true, edits)?;
                edits.push((block.body.end..block.body.end + 1, String::new()));
            } else if protected && !block.at_rule && blocks(css, block.body.clone()).is_empty() {
                self.observe(&css[block.body.clone()]);
                edits.push((block.header.start..block.header.start, self.support()?));
                edits.push((block.body.end + 1..block.body.end + 1, "}".into()));
            } else {
                self.walk(css, block.body.clone(), protected, edits)?;
            }
            cursor = block.body.end + 1;
        }
        self.declarations(css, cursor..range.end, protected, edits)
    }

    /// Consumes a declaration run between child blocks of `css`.
    /// `range` must be a valid UTF-8 byte range with no child rules. When `protected`,
    /// its surrounding context must permit a conditional group containing declarations.
    /// Appends support-block insertions for a nonempty protected batch after observing
    /// it. A fragment with no recognized declarations changes neither state nor edits.
    ///
    /// # Errors
    ///
    /// Returns a naming failure after observing the batch, without appending its edits.
    ///
    /// # Panics
    ///
    /// Panics if `range` is outside `css`, reversed, or splits a UTF-8 scalar.
    fn declarations(
        &mut self,
        css: &str,
        range: Range<usize>,
        protected: bool,
        edits: &mut Vec<(Range<usize>, String)>,
    ) -> Result<(), CompileDiagnostic> {
        if declaration_values(&css[range.clone()]).is_empty() {
            return Ok(());
        }
        self.observe(&css[range.clone()]);
        if protected {
            edits.push((range.start..range.start, self.support()?));
            edits.push((range.end..range.end, "}".into()));
        }
        Ok(())
    }
}

/// Immediate child block coordinates used to preserve wrappers while editing their
/// bodies.
struct Block {
    /// Half-open byte range into the full CSS, excluding `{` and including leading
    /// trivia.
    header: Range<usize>,
    /// Half-open content range into the full CSS, excluding both braces.
    body: Range<usize>,
    at_rule: bool,
    /// Byte offset of an internal marker's `@`, after any leading comments or
    /// whitespace.
    marker: Option<usize>,
}

/// Returns immediate child blocks in source order within a complete CSS list range.
/// `range` must address balanced rendered CSS on UTF-8 boundaries. Returned coordinates
/// refer to the full `css` buffer. Functions and custom-property brace values stay
/// opaque, and marker offsets assume the literal spelling emitted by [`protect`].
///
/// # Panics
///
/// Panics if `range` is invalid for slicing. A noncanonical marker spelling can
/// underflow the calculation of its start offset.
fn blocks(css: &str, range: Range<usize>) -> Vec<Block> {
    let fragment = &css[range.clone()];
    let mut parser = Parser::new(fragment);
    let mut start = 0;
    let mut output = Vec::new();
    while let Ok(token) = parser.next_including_whitespace_and_comments() {
        match token {
            Token::Semicolon => start = parser.position().byte_index(),
            Token::CurlyBracketBlock => {
                let inner = parser.position().byte_index();
                let mut header = Parser::new(&fragment[start..inner - 1]);
                let first = header.next();
                let at_rule = matches!(first, Ok(Token::AtKeyword(_)));
                let boundary =
                    matches!(&first, Ok(Token::AtKeyword(name)) if name.as_ref() == &MARKER[1..]);
                let custom = matches!(first, Ok(Token::Ident(name)) if name.starts_with("--"))
                    && matches!(header.next(), Ok(Token::Colon));
                let marker = boundary
                    .then(|| range.start + start + header.position().byte_index() - MARKER.len());
                let end = consume(&mut parser);
                if !custom {
                    output.push(Block {
                        header: range.start + start..range.start + inner - 1,
                        body: range.start + inner..range.start + end,
                        at_rule,
                        marker,
                    });
                    start = end + 1;
                }
            }
            Token::Function(_) | Token::ParenthesisBlock | Token::SquareBracketBlock => {
                consume(&mut parser);
            }
            _ => {}
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identifiers::{GeneratorSet, IdentifierMode};

    fn render(css: &str) -> String {
        let identifiers =
            IdentifierContext::new(GeneratorSet::default(), IdentifierMode::Release, 5);
        RuleBoundaries::new("test".into(), &identifiers)
            .render(css)
            .unwrap()
    }

    fn names(css: &str) -> Vec<&str> {
        css.split("and (--")
            .skip(1)
            .map(|tail| tail.split(':').next().unwrap())
            .collect()
    }

    #[test]
    fn intervals_accumulate_until_a_property_reappears() {
        let css = ["color:red", "width:1px", "color:blue", "height:2px"]
            .map(|body| protect(&format!(".x{{{body}}}")))
            .join("");
        let output = render(&css);
        let names = names(&output);
        assert_eq!(names.len(), 4, "{output}");
        assert_eq!(names[0], names[1]);
        assert_ne!(names[1], names[2]);
        assert_eq!(names[2], names[3]);
        assert!(names.iter().all(|name| name.len() == 5));
        assert!(!output.contains(MARKER));
    }

    #[test]
    fn shorthand_logical_all_and_unprotected_rules_advance_intervals() {
        for (first, middle, last) in [
            ("border:0", "", "border-color:red"),
            ("margin-left:0", "", "margin-inline-start:1px"),
            ("color:red", "", "all:initial"),
            ("color:red", ".x{width:1px}", "width:2px"),
        ] {
            let output = render(&format!(
                "{}{middle}{}",
                protect(&format!(".x{{{first}}}")),
                protect(&format!(".x{{{last}}}"))
            ));
            let names = names(&output);
            assert_ne!(names[0], names[1], "{output}");
        }
    }

    #[test]
    fn nested_boundaries_use_leaf_declarations_and_keep_custom_blocks_opaque() {
        let output = render(&protect(&format!(
            ".x{{--data:{{a:b}};color:red;@media print{{width:1px;}}{} }}",
            protect("&:hover{color:blue}")
        )));
        let names = names(&output);
        assert_eq!(names.len(), 3, "{output}");
        assert_eq!(names[0], names[1]);
        assert_ne!(names[1], names[2]);
        assert!(output.contains("--data:{a:b}"), "{output}");
        assert!(!output.contains(MARKER));
    }

    #[test]
    fn equal_helpers_do_not_split_independent_conditional_properties() {
        let output = render(&format!(
            "{}{}",
            protect(".x{--gate:var(--a) var(--b);color:var(--gate)}"),
            protect(".x{--gate:var(--a) var(--b);width:var(--gate)}")
        ));
        let names = names(&output);
        assert_eq!(names[0], names[1]);
    }

    #[test]
    fn comments_before_protected_rules_survive_finalization() {
        let output = render(&format!(".x{{/*! keep */{} }}", protect("&{color:red}")));
        assert!(output.contains("/*! keep */"), "{output}");
        assert!(!output.contains(MARKER), "{output}");
        assert_eq!(names(&output).len(), 1);
    }

    #[test]
    fn property_comments_and_escapes_do_not_hide_conflicts() {
        let output = render(&format!(
            "{}{}",
            protect(".x{/* comment */color:red}"),
            protect(r".x{c\6flor:blue}")
        ));
        let names = names(&output);
        assert_ne!(names[0], names[1], "{output}");
    }

    #[test]
    fn distinct_owners_never_share_an_interval_identity() {
        let identifiers =
            IdentifierContext::new(GeneratorSet::default(), IdentifierMode::Release, 5);
        let css = protect(".x{color:red}");
        let left = RuleBoundaries::new("left".into(), &identifiers)
            .render(&css)
            .unwrap();
        let right = RuleBoundaries::new("right".into(), &identifiers)
            .render(&css)
            .unwrap();
        assert_ne!(names(&left), names(&right));
    }
}
