//! Source edits for independent CSS sorting targets and stylesheet lint.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use lean_string::{LeanStr, LeanString, ToLeanString};
use serde::{Deserialize, Serialize};

use stillwind_css::{CssNode, CssNodeKind, parse_structure, properties_interfere};
use stillwind_design_system::{UtilityResolutionError, VariantSequenceOrder, resolve_variant};

use crate::compiler::{parse_variant_sequence, variant_sequences};
use crate::query::{CandidateContext, DesignSystemQuery};

/// Independent targets for CSS sorting. These settings also control sorting lint.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StylesheetSortOptions {
    /// Sorts utility arguments of individual `@apply` statements. Defaults to true.
    pub apply_utilities: bool,
    /// Orders adjacent `@apply` statements without reversing competing writes.
    /// Defaults to false. Statements retain their original argument spelling.
    pub apply_statements: bool,
    /// Orders adjacent applied `@variant` blocks without reversing competing writes.
    /// Defaults to false. Definition blocks and variant headers retain their spelling.
    pub variant_blocks: bool,
}

impl Default for StylesheetSortOptions {
    fn default() -> Self {
        Self {
            apply_utilities: true,
            apply_statements: false,
            variant_blocks: false,
        }
    }
}

/// One replacement shared by explicit CSS sorting and sorting lint fixes.
#[derive(Clone, Debug)]
pub struct StylesheetEdit {
    /// Half-open UTF-8 byte range in the original stylesheet.
    pub range: Range<usize>,
    /// Original CSS spelling with inner edits incorporated before outer movement.
    pub replacement: LeanStr,
    /// True when this replacement moves whole directives rather than utility arguments.
    pub(crate) directives: bool,
}

impl DesignSystemQuery {
    /// Plans CSS sorting with this query's captured targets and compiler order.
    /// `protected` contains half-open UTF-8 ranges that no replacement may intersect.
    /// All ranges address the original `css`. Returns nonoverlapping edits in source
    /// order without modifying the input. Only complete statements and blocks move.
    /// Comments, unsupported syntax, and unknown effects delimit movement. Competing
    /// property writes retain their relative order, even under different variants.
    ///
    /// # Errors
    ///
    /// Returns a malformed requirement from a matching utility definition.
    pub fn sort_stylesheet(
        &self,
        css: &str,
        protected: &[Range<usize>],
    ) -> Result<Vec<StylesheetEdit>, UtilityResolutionError> {
        let options = self.context.options.stylesheet_sort;
        if !options.apply_utilities && !options.apply_statements && !options.variant_blocks {
            return Ok(Vec::new());
        }
        let nodes = parse_structure(css);
        let mut candidates = Vec::new();
        application_candidates(css, &nodes, false, &mut candidates);
        let ranks = self.candidate_ranks(&candidates)?;
        let planner = Planner {
            query: self,
            css,
            protected,
            ranks: candidates
                .into_iter()
                .zip(ranks)
                .filter_map(|(raw, rank)| rank.map(|rank| (raw, rank)))
                .collect(),
        };
        let mut edits = planner.visit(&nodes, false)?;
        edits.sort_by_key(|edit| edit.range.start);
        Ok(edits)
    }
}

/// Holds original coordinates and one batch of ranks throughout recursive planning.
struct Planner<'a> {
    query: &'a DesignSystemQuery,
    css: &'a str,
    protected: &'a [Range<usize>],
    ranks: BTreeMap<&'a str, usize>,
}

impl Planner<'_> {
    /// Plans inner edits before moving complete siblings. `in_rule` identifies an
    /// applied variant; definition bodies do not participate in source sorting.
    fn visit(
        &self,
        nodes: &[CssNode],
        in_rule: bool,
    ) -> Result<Vec<StylesheetEdit>, UtilityResolutionError> {
        let options = self.query.context.options.stylesheet_sort;
        let mut edits = Vec::new();
        for node in nodes {
            match &node.kind {
                CssNodeKind::StyleRule { block, .. } => {
                    edits.extend(self.visit(&block.children, true)?);
                }
                CssNodeKind::AtRule {
                    name,
                    block: Some(block),
                    ..
                } if !definition(name) && (name != "variant" || in_rule) => {
                    edits.extend(self.visit(&block.children, in_rule)?);
                }
                CssNodeKind::Apply { candidates }
                    if options.apply_utilities && self.complete(node) =>
                {
                    if let Some(edit) = self.application(candidates) {
                        edits.push(edit);
                    }
                }
                _ => {}
            }
        }
        let mut run = Vec::new();
        for node in nodes {
            let item = self.item(node, in_rule)?;
            let continues = item.as_ref().is_some_and(|item| {
                run.last().is_none_or(|previous: &MoveItem| {
                    item.key.same_kind(&previous.key)
                        && self.css[previous.range.end..item.range.start]
                            .trim()
                            .is_empty()
                        && self.permits(&(previous.range.end..item.range.start))
                })
            });
            if !continues {
                self.move_run(&run, &mut edits);
                run.clear();
            }
            if let Some(item) = item {
                run.push(item);
            }
        }
        self.move_run(&run, &mut edits);
        Ok(edits)
    }

    /// Returns a replacement for complete utility arguments, preserving separators.
    /// Unknown candidates and comment gaps separate independently sorted runs.
    fn application(&self, candidates: &[Range<usize>]) -> Option<StylesheetEdit> {
        let (first, last) = (candidates.first()?, candidates.last()?);
        let range = first.start..last.end;
        if !self.permits(&range) {
            return None;
        }
        let mut order: Vec<_> = (0..candidates.len()).collect();
        let rank = |index: usize| {
            self.ranks
                .get(&self.css[candidates[index].clone()])
                .copied()
        };
        let mut start = 0;
        for position in 0..=candidates.len() {
            let unknown = position < candidates.len() && rank(position).is_none();
            let comment = position > 0
                && position < candidates.len()
                && !self.css[candidates[position - 1].end..candidates[position].start]
                    .trim()
                    .is_empty();
            if position == candidates.len() || unknown || comment {
                order[start..position].sort_by_key(|&index| rank(index));
                start = position + usize::from(unknown);
            }
        }
        if order
            .iter()
            .enumerate()
            .all(|(index, &other)| index == other)
        {
            return None;
        }
        let mut replacement = LeanString::new();
        let mut cursor = range.start;
        for (slot, &index) in order.iter().enumerate() {
            replacement.push_str(&self.css[cursor..candidates[slot].start]);
            replacement.push_str(&self.css[candidates[index].clone()]);
            cursor = candidates[slot].end;
        }
        Some(StylesheetEdit {
            range,
            replacement: replacement.into(),
            directives: false,
        })
    }

    /// Returns a movable sibling only when its complete effects and ordering key resolve.
    fn item(
        &self,
        node: &CssNode,
        in_rule: bool,
    ) -> Result<Option<MoveItem>, UtilityResolutionError> {
        if !self.complete(node) || !self.permits(&node.range) {
            return Ok(None);
        }
        let options = self.query.context.options.stylesheet_sort;
        let key = match &node.kind {
            CssNodeKind::Apply { candidates }
                if options.apply_statements && !candidates.is_empty() =>
            {
                let Some(mut keys) = candidates
                    .iter()
                    .map(|range| self.ranks.get(&self.css[range.clone()]).copied())
                    .collect::<Option<Vec<_>>>()
                else {
                    return Ok(None);
                };
                keys.sort_unstable();
                SortKey::Apply(keys)
            }
            CssNodeKind::AtRule {
                name,
                prelude,
                block: Some(_),
                ..
            } if in_rule && name == "variant" && options.variant_blocks => {
                let Some(keys) = self.variant_keys(&self.css[prelude.clone()]) else {
                    return Ok(None);
                };
                SortKey::Variant(keys)
            }
            _ => return Ok(None),
        };
        let Some(properties) = self.effects(std::slice::from_ref(node), node.range.clone())? else {
            return Ok(None);
        };
        Ok(Some(MoveItem {
            range: node.range.clone(),
            key,
            properties,
        }))
    }

    /// Prepares every comma-separated alternative without changing its written order.
    fn variant_keys(&self, prelude: &str) -> Option<Vec<VariantSequenceOrder>> {
        if prelude.trim().is_empty() {
            return None;
        }
        let theme = self.query.context.design_system.theme();
        let mut keys = variant_sequences(prelude)
            .into_iter()
            .map(|range| {
                let variants = parse_variant_sequence(&prelude[range])?;
                variants
                    .iter()
                    .all(|variant| resolve_variant(variant, theme).is_ok())
                    .then(|| VariantSequenceOrder::new(&variants, theme))
            })
            .collect::<Option<Vec<_>>>()?;
        keys.sort_by(VariantSequenceOrder::compare);
        Some(keys)
    }

    /// Collects possible writes, disregarding selectors and conditions. Any unsupported
    /// rule or unparsed text makes movement unavailable rather than proving exclusion.
    fn effects(
        &self,
        nodes: &[CssNode],
        range: Range<usize>,
    ) -> Result<Option<BTreeSet<LeanStr>>, UtilityResolutionError> {
        let mut properties = BTreeSet::new();
        let mut cursor = range.start;
        for node in nodes {
            if !self.css[cursor..node.range.start].trim().is_empty() {
                return Ok(None);
            }
            cursor = node.range.end;
            match &node.kind {
                CssNodeKind::StyleRule { block, .. } => {
                    let Some(writes) = self.effects(&block.children, block.contents.clone())?
                    else {
                        return Ok(None);
                    };
                    properties.extend(writes);
                }
                CssNodeKind::AtRule {
                    name,
                    prelude,
                    block: Some(block),
                } if matches!(
                    name.as_str(),
                    "media" | "supports" | "container" | "starting-style"
                ) || (name == "variant"
                    && self.variant_keys(&self.css[prelude.clone()]).is_some()) =>
                {
                    let Some(writes) = self.effects(&block.children, block.contents.clone())?
                    else {
                        return Ok(None);
                    };
                    properties.extend(writes);
                }
                CssNodeKind::AtRule { .. } => {
                    return Ok(None);
                }
                CssNodeKind::Apply { candidates } => {
                    for range in candidates {
                        let Some(css) = self.query.preview_candidate(
                            &self.css[range.clone()],
                            CandidateContext::Utility,
                        )?
                        else {
                            return Ok(None);
                        };
                        let Some(writes) = preview_properties(&css) else {
                            return Ok(None);
                        };
                        properties.extend(writes);
                    }
                }
                CssNodeKind::Declaration { property, .. } => {
                    properties.insert(
                        self.css[property.clone()]
                            .trim()
                            .to_ascii_lowercase()
                            .into(),
                    );
                }
            }
        }
        Ok(self.css[cursor..range.end]
            .trim()
            .is_empty()
            .then_some(properties))
    }

    /// Orders ready siblings by compiler key while retaining the original order of
    /// every pair with competing writes. The resulting order is stable and idempotent.
    fn move_run(&self, items: &[MoveItem], edits: &mut Vec<StylesheetEdit>) {
        let mut pending: Vec<_> = (0..items.len()).collect();
        let mut order = Vec::new();
        while !pending.is_empty() {
            let next = pending
                .iter()
                .enumerate()
                .filter(|&(position, &index)| {
                    pending[..position]
                        .iter()
                        .all(|&before| !items[before].competes(&items[index]))
                })
                .min_by(|&(_, &a), &(_, &b)| items[a].key.compare(&items[b].key))
                .map_or(0, |(position, _)| position);
            order.push(pending.remove(next));
        }
        if order
            .iter()
            .enumerate()
            .all(|(index, &other)| index == other)
        {
            return;
        }
        let range = items[0].range.start..items[items.len() - 1].range.end;
        let text: Vec<_> = items
            .iter()
            .map(|item| edited(self.css, item.range.clone(), edits))
            .collect();
        let mut replacement = LeanString::new();
        for (slot, &index) in order.iter().enumerate() {
            if slot > 0 {
                replacement.push_str(&self.css[items[slot - 1].range.end..items[slot].range.start]);
            }
            replacement.push_str(&text[index]);
        }
        edits.retain(|edit| !(range.start <= edit.range.start && edit.range.end <= range.end));
        edits.push(StylesheetEdit {
            range,
            replacement: replacement.into(),
            directives: true,
        });
    }

    /// Requires a terminating semicolon or closing brace in the original text.
    fn complete(&self, node: &CssNode) -> bool {
        self.css[node.range.clone()].ends_with([';', '}'])
    }

    /// Rejects any operation that intersects a caller-protected range.
    fn permits(&self, range: &Range<usize>) -> bool {
        !self
            .protected
            .iter()
            .any(|other| range.start < other.end && other.start < range.end)
    }
}

/// A complete sibling and its possible writes, all in original source coordinates.
struct MoveItem {
    range: Range<usize>,
    key: SortKey,
    properties: BTreeSet<LeanStr>,
}

impl MoveItem {
    /// Returns whether either item writes a property that competes with the other's.
    fn competes(&self, other: &Self) -> bool {
        self.properties
            .iter()
            .any(|a| other.properties.iter().any(|b| properties_interfere(a, b)))
    }
}

/// Compiler keys for one homogeneous run of siblings.
enum SortKey {
    Apply(Vec<usize>),
    Variant(Vec<VariantSequenceOrder>),
}

impl SortKey {
    /// Returns whether these siblings can belong to the same movement run.
    fn same_kind(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Apply(_), Self::Apply(_)) | (Self::Variant(_), Self::Variant(_))
        )
    }

    /// Compares prepared keys from the same snapshot and rank batch.
    fn compare(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Apply(a), Self::Apply(b)) => a.cmp(b),
            (Self::Variant(a), Self::Variant(b)) => a
                .iter()
                .zip(b)
                .map(|(a, b)| a.compare(b))
                .find(|order| *order != Ordering::Equal)
                .unwrap_or_else(|| a.len().cmp(&b.len())),
            _ => Ordering::Equal,
        }
    }
}

/// Collects utility spellings for one batch whose relative ranks remain comparable.
fn application_candidates<'a>(
    css: &'a str,
    nodes: &[CssNode],
    in_rule: bool,
    output: &mut Vec<&'a str>,
) {
    for node in nodes {
        if let CssNodeKind::Apply { candidates } = &node.kind {
            output.extend(candidates.iter().map(|range| &css[range.clone()]));
        }
        if let CssNodeKind::AtRule { name, .. } = &node.kind
            && (definition(name) || (name == "variant" && !in_rule))
        {
            continue;
        }
        application_candidates(
            css,
            node.children(),
            in_rule || matches!(node.kind, CssNodeKind::StyleRule { .. }),
            output,
        );
    }
}

/// Returns whether an at-rule defines the design system rather than authored output.
fn definition(name: &str) -> bool {
    matches!(
        name,
        "theme"
            | "utility"
            | "custom-variant"
            | "stillwind-utility"
            | "stillwind-variant"
            | "stillwind-property"
            | "stillwind-registry"
    )
}

/// Collects generated declaration properties without assuming that conditions exclude
/// each other. Unknown rules and registration effects make the preview unsuitable.
fn preview_properties(css: &str) -> Option<BTreeSet<LeanStr>> {
    fn collect(css: &str, nodes: &[CssNode], properties: &mut BTreeSet<LeanStr>) -> Option<()> {
        for node in nodes {
            match &node.kind {
                CssNodeKind::StyleRule { block, .. } => {
                    collect(css, &block.children, properties)?;
                }
                CssNodeKind::AtRule {
                    name,
                    block: Some(block),
                    ..
                } if matches!(
                    name.as_str(),
                    "media" | "supports" | "container" | "starting-style"
                ) =>
                {
                    collect(css, &block.children, properties)?;
                }
                CssNodeKind::Declaration { property, .. } => {
                    properties.insert(css[property.clone()].trim().to_ascii_lowercase().into());
                }
                _ => {
                    return None;
                }
            }
        }
        Some(())
    }
    let mut properties = BTreeSet::new();
    collect(css, &parse_structure(css), &mut properties)?;
    (!properties.is_empty()).then_some(properties)
}

/// Applies already planned inner replacements to one original node slice.
fn edited(css: &str, range: Range<usize>, edits: &[StylesheetEdit]) -> LeanString {
    let mut text = css[range.clone()].to_lean_string();
    let mut inner: Vec<_> = edits
        .iter()
        .filter(|edit| range.start <= edit.range.start && edit.range.end <= range.end)
        .collect();
    inner.sort_by_key(|edit| std::cmp::Reverse(edit.range.start));
    for edit in inner {
        text.replace_range(
            edit.range.start - range.start..edit.range.end - range.start,
            &edit.replacement,
        );
    }
    text
}

#[cfg(test)]
mod tests {
    use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
    use stillwind_source::FileId;
    use stillwind_test_support::css_source;

    use super::{StylesheetSortOptions, edited};
    use crate::{CompilerOptions, DesignSystemQuery};

    fn query(options: StylesheetSortOptions) -> DesignSystemQuery {
        let source = css_source(FileId(1), "@utility a {display:block;} @utility b {margin:1px;} @utility c {padding:2px;} @utility d {padding:4px;} @custom-variant hover (&:hover); @custom-variant focus (&:focus);").unwrap();
        DesignSystemQuery::new(
            DesignSystemSnapshot::from_source(
                &source,
                "theme.css".into(),
                DesignSystemOptions::default(),
            )
            .unwrap(),
            CompilerOptions {
                stylesheet_sort: options,
                ..CompilerOptions::default()
            },
        )
    }

    fn sorted(query: &DesignSystemQuery, css: &str) -> String {
        let edits = query.sort_stylesheet(css, &[]).unwrap();
        let result = edited(css, 0..css.len(), &edits).to_string();
        assert!(
            query.sort_stylesheet(&result, &[]).unwrap().is_empty(),
            "not idempotent: {result}"
        );
        result
    }

    #[test]
    fn targets_are_independent_and_outer_moves_absorb_inner_edits() {
        let css = ".a { @apply c; @apply b a; }";
        for (arguments, statements, expected) in [
            (false, false, css),
            (true, false, ".a { @apply c; @apply a b; }"),
            (false, true, ".a { @apply b a; @apply c; }"),
            (true, true, ".a { @apply a b; @apply c; }"),
        ] {
            let query = query(StylesheetSortOptions {
                apply_utilities: arguments,
                apply_statements: statements,
                variant_blocks: false,
            });
            assert_eq!(sorted(&query, css), expected);
        }
    }

    #[test]
    fn movement_preserves_competing_writes_across_different_variants() {
        let query = query(StylesheetSortOptions {
            apply_statements: true,
            variant_blocks: true,
            ..StylesheetSortOptions::default()
        });
        for css in [
            ".a { @apply d; @apply c; }",
            ".a { @apply focus:d; @apply hover:c; }",
            ".a { @variant focus { @apply d; } @variant hover { @apply c; } }",
            ".a { @variant focus { padding:2px; } @variant hover { padding-left:4px; } }",
            ".a { @variant focus { margin-left:2px; } @variant hover { margin-inline-start:4px; } }",
            ".a { @variant focus { all:initial; } @variant hover { display:flex; } }",
            ".a { @variant focus { --brand:red; } @variant hover { --brand:blue; } }",
        ] {
            assert_eq!(sorted(&query, css), css);
        }
        assert_eq!(
            sorted(&query, ".a { @apply d; @apply a; @apply c; @apply b; }"),
            ".a { @apply a; @apply b; @apply d; @apply c; }"
        );
        assert_eq!(
            sorted(
                &query,
                ".a { @variant focus { @apply c; } @variant hover { @apply a; } }"
            ),
            ".a { @variant hover { @apply a; } @variant focus { @apply c; } }"
        );
    }

    #[test]
    fn comments_unknown_inputs_definitions_and_protected_ranges_delimit_edits() {
        let query = query(StylesheetSortOptions {
            apply_statements: true,
            variant_blocks: true,
            ..StylesheetSortOptions::default()
        });
        for css in [
            ".a { @apply c; /* keep */ @apply a; }",
            ".a { @apply c; color:red; @apply a; }",
            ".a { @apply c; @apply unknown; @apply a; }",
            ".a { @apply c /* keep */ a; }",
            "@variant focus { @apply c a; @slot; }",
            "@utility example { @apply c a; }",
            ".a { @apply c a",
        ] {
            assert_eq!(sorted(&query, css), css);
        }
        let css = ".a { @apply c a; @apply b; }";
        assert!(
            query
                .sort_stylesheet(css, std::slice::from_ref(&(0..css.len())))
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            sorted(&query, ".a { @apply c a unknown b a; }"),
            ".a { @apply a c unknown a b; }"
        );
    }
}
