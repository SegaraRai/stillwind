//! Divides a group into parts whose candidates cannot change each other's CSS, so that
//! each part can be collapsed alone.
//!
//! Comparing the complete group for every proposal becomes slow when a group has
//! hundreds of candidates. Two candidates are put in the same part when they set
//! interfering properties, when one writes a custom property that the other uses, or
//! when one proposal may replace both. A replacement in one part therefore cannot
//! change the CSS of another part.
//!
//! The module collapses each part alone. It then orders the accepted proposals of all
//! parts by `rank`, as the complete group would accept them. When a proposal in one
//! part can change the CSS of a candidate in another part, the module joins the two
//! parts and collapses the joined part again. Other parts keep their earlier result.

use std::collections::{BTreeMap, BTreeSet};

use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::{CssRule, Declaration, properties_interfere};
use stillwind_design_system::UtilityResolutionError;

use super::super::index::rank;
use super::compare::physical_properties;
use super::group::Group;
use crate::query::DesignSystemQuery;

/// The CSS properties and custom properties that one utility sets or uses.
pub(super) struct Footprint {
    /// Standard properties that any rule of the utility declares.
    properties: BTreeSet<LeanStr>,
    /// Custom properties that the utility declares.
    writes: BTreeSet<LeanStr>,
    /// Every custom property name that appears in the CSS of the utility or in its
    /// registration requirements. It also contains the names in `writes`.
    names: BTreeSet<LeanStr>,
}

/// The result of `collapse_component` for one part.
pub(super) enum Component {
    /// The proposals that were accepted, in the order of acceptance. The part now
    /// contains the replacements.
    Accepted(Vec<Acceptance>),
    /// A proposal can change the CSS of the candidate at this index of the complete
    /// group. That candidate belongs to another part.
    Related(usize),
    /// The CSS of the part cannot be compared, so only the complete group can be
    /// collapsed.
    Incomparable,
}

/// One proposal that `collapse_component` accepted.
pub(super) struct Acceptance {
    /// The proposal as it was listed, before it was replaced by its canonical spelling.
    /// Its `rank` decides the order of acceptances across parts.
    pub(super) proposal: LeanStr,
    /// The position that the proposal received as a new candidate. It is `None` when
    /// the proposal was already in the group.
    pub(super) position: Option<usize>,
}

impl DesignSystemQuery {
    /// Replaces candidates in `group` with proposals until no proposal can be accepted.
    /// Each new candidate gets the position `next_position`, and `next_position`
    /// increases by one for each new candidate, in the order of acceptance.
    ///
    /// The result is usually the same as the result of `collapse_component` for the
    /// complete group. It can contain fewer candidates when one candidate of the group
    /// has a declaration that `Declaration::parse` rejects, such as a property with a
    /// vendor prefix. In that case, the comparison of the complete group cannot merge
    /// declarations by property, and it rejects some replacements. A part without that
    /// candidate accepts them. For example, the complete group
    /// `z-10 z-[100] box-decoration-clone` keeps `z-10`, but its part removes `z-10`,
    /// which `z-100` overrides.
    ///
    /// # Errors
    ///
    /// Returns malformed requirements of a candidate or of a proposal.
    pub(super) fn collapse_group(
        &self,
        group: &mut Group,
        next_position: &mut usize,
    ) -> Result<(), UtilityResolutionError> {
        if group.values.len() < 2 {
            return Ok(());
        }
        let declarations: Vec<_> = group
            .values
            .iter()
            .map(|value| self.candidate_declarations(value))
            .collect();
        if self
            .collapse_proposals(&group.values, &declarations)
            .is_empty()
        {
            return Ok(());
        }
        let before: Vec<_> = group.decorated();
        if self.canonical_set(&before)?.is_none() {
            return Ok(());
        }
        let footprints: Vec<_> = group
            .values
            .iter()
            .map(|value| self.footprint(value))
            .collect();
        let mut parents = self.collapse_links(&declarations, &footprints);
        // Positions above every real position keep the order within each part until
        // the merge below assigns the real positions. They depend only on the first
        // member, so a part keeps its result when other parts are joined.
        let stride = group.values.len() + 1;
        let mut finished = BTreeMap::new();
        let parts = 'parts: loop {
            let parts = parts(&mut parents);
            if parts.len() < 2 {
                self.collapse_component(group, next_position, &[])?;
                return Ok(());
            }
            for part in &parts {
                if finished.contains_key(part) {
                    continue;
                }
                let outside: Vec<_> = footprints
                    .iter()
                    .enumerate()
                    .filter(|(member, _)| !part.contains(member))
                    .filter_map(|(member, footprint)| Some((member, footprint.as_ref()?)))
                    .collect();
                let mut subset = group.subset(part);
                let mut temporary = usize::MAX / 2 + part[0] * stride;
                match self.collapse_component(&mut subset, &mut temporary, &outside)? {
                    Component::Accepted(acceptances) => {
                        finished.insert(part.clone(), (subset, acceptances));
                    }
                    // The proposal can change the CSS of that utility, so both parts
                    // must be collapsed together.
                    Component::Related(member) => {
                        union(&mut parents, part[0], member);
                        continue 'parts;
                    }
                    Component::Incomparable => {
                        self.collapse_component(group, next_position, &[])?;
                        return Ok(());
                    }
                }
            }
            break parts;
        };
        let collapsed: Vec<_> = parts
            .iter()
            .filter_map(|part| finished.remove(part))
            .collect();

        // The complete group accepts its first proposal in rank order in each round.
        // A replacement in one part keeps the proposals of other parts unchanged, so
        // the complete group takes the lowest next accepted proposal among the parts.
        let mut cursors = vec![0; collapsed.len()];
        let mut positions = BTreeMap::new();
        while let Some((part, acceptance)) = collapsed
            .iter()
            .enumerate()
            .filter_map(|(part, (_, acceptances))| {
                acceptances
                    .get(cursors[part])
                    .map(|acceptance| (part, acceptance))
            })
            .min_by(|(_, left), (_, right)| rank(&left.proposal, &right.proposal))
        {
            if let Some(temporary) = acceptance.position {
                positions.insert(temporary, *next_position);
                *next_position += 1;
            }
            cursors[part] += 1;
        }
        group.values.clear();
        group.positions.clear();
        for (subset, _) in collapsed {
            for (value, position) in subset.values.into_iter().zip(subset.positions) {
                group.values.push(value);
                group
                    .positions
                    .push(positions.get(&position).copied().unwrap_or(position));
            }
        }
        Ok(())
    }

    /// Returns the parents of a union-find structure in which joined candidates may
    /// change each other's CSS or share a proposal.
    ///
    /// `declarations` and `footprints` must describe the candidates of the group in the
    /// same order. A candidate without a footprint is joined with every other
    /// candidate.
    fn collapse_links(
        &self,
        declarations: &[Option<Vec<Declaration>>],
        footprints: &[Option<Footprint>],
    ) -> Vec<usize> {
        let mut parents: Vec<_> = (0..footprints.len()).collect();
        let mut users = BTreeMap::<_, Vec<_>>::new();
        for (index, sources) in declarations.iter().enumerate() {
            for declaration in sources.iter().flatten() {
                for family in self.property_families(&declaration.property) {
                    users.entry(family).or_default().push(index);
                }
            }
        }
        for (family, members) in &users {
            if family.ends_with("-*") {
                for member in members {
                    union(&mut parents, members[0], *member);
                }
            }
        }
        for left in 0..footprints.len() {
            for right in left + 1..footprints.len() {
                if find(&mut parents, left) == find(&mut parents, right) {
                    continue;
                }
                let related = match (&footprints[left], &footprints[right]) {
                    (Some(left), Some(right)) => self.footprints_relate(left, right),
                    _ => true,
                };
                if related {
                    union(&mut parents, left, right);
                }
            }
        }

        // A fixed name such as `sr-only` declares many properties. Its proposal can
        // replace utilities only when the inputs relate to every one of those
        // properties. Checking that requires its CSS, so it is skipped when the users
        // of the name are already joined.
        let inputs: BTreeSet<_> = declarations
            .iter()
            .flatten()
            .flatten()
            .map(|declaration| declaration.property.as_str())
            .collect();
        for (family, members) in &users {
            if family.contains('*')
                || members
                    .iter()
                    .all(|member| find(&mut parents, *member) == find(&mut parents, members[0]))
            {
                continue;
            }
            if self.may_fit(family, &inputs)
                && self
                    .candidate_declarations(family)
                    .is_some_and(|properties| self.proposal_fits(&properties, declarations))
            {
                for member in members {
                    union(&mut parents, members[0], *member);
                }
            }
        }
        parents
    }

    /// Checks, without generating CSS, whether the fixed utility `name` may replace
    /// candidates that set the properties `inputs`. Returns false only when every
    /// definition of `name` declares a standard property that no property in `inputs`
    /// relates to. A true result still needs the full check of `proposal_fits`.
    fn may_fit(&self, name: &str, inputs: &BTreeSet<&str>) -> bool {
        let mut definitions = self
            .context
            .design_system
            .utility_search()
            .definitions_with_prefix(name)
            .filter(|(candidate, _)| *candidate == name)
            .flat_map(|(_, entries)| entries)
            .peekable();
        definitions.peek().is_none()
            || definitions.any(|definition| {
                definition
                    .properties
                    .iter()
                    .filter(|property| !property.starts_with("--"))
                    .all(|property| {
                        inputs.contains(property.as_str())
                            || inputs
                                .iter()
                                .any(|input| self.properties_relate(property, input))
                    })
            })
    }

    /// Returns the index of a candidate in `outside` whose CSS `proposal` can change,
    /// or `None` when there is no such candidate. When the CSS of `proposal` cannot be
    /// read, the method assumes that `proposal` can change every candidate and returns
    /// the first index.
    pub(super) fn related_outside(
        &self,
        proposal: &str,
        outside: &[(usize, &Footprint)],
    ) -> Option<usize> {
        let Some(footprint) = self.footprint(proposal) else {
            return outside.first().map(|(member, _)| *member);
        };
        outside
            .iter()
            .find(|(_, other)| self.footprints_relate(&footprint, other))
            .map(|(member, _)| *member)
    }

    /// Returns the `Footprint` of the candidate `value`, written without the variants
    /// of its group. Returns `None` when `value` does not resolve, or when its CSS
    /// contains a statement that is not a declaration, a rule, or an at-rule.
    fn footprint(&self, value: &str) -> Option<Footprint> {
        let preview = self.context.canonical_preview(value).ok()??;
        let mut declared = Vec::new();
        if !nested_properties(&preview.css, 0, &mut declared) {
            return None;
        }
        let (writes, properties): (BTreeSet<_>, BTreeSet<_>) = declared
            .into_iter()
            .partition(|property| property.starts_with("--"));
        let mut names = custom_property_names(&preview.css);
        names.extend(preview.required_properties.iter().cloned());
        names.extend(writes.iter().cloned());
        Some(Footprint {
            properties,
            writes,
            names,
        })
    }

    /// Checks whether replacing the utility of `left` can change the CSS of the utility
    /// of `right`, or the reverse. This is true when they set related properties, when
    /// one writes a custom property that the other uses, or when they set the value
    /// property and the modifier property of one split family, such as `font-size` and
    /// `line-height` of `text-*`.
    fn footprints_relate(&self, left: &Footprint, right: &Footprint) -> bool {
        let has = |footprint: &Footprint, property: &str| footprint.properties.contains(property);
        left.properties.iter().any(|left| {
            right
                .properties
                .iter()
                .any(|right| self.properties_relate(left, right))
        }) || !left.writes.is_disjoint(&right.names)
            || !right.writes.is_disjoint(&left.names)
            // `text-*` proposals combine every font size with every line height.
            || self
                .context
                .design_system
                .utility_search()
                .split_families()
                .iter()
                .any(|family| {
                    let (value, modifier) = (&family.value_property, &family.modifier_property);
                    (has(left, value) && has(right, modifier))
                        || (has(left, modifier) && has(right, value))
                })
    }

    /// Checks whether the standard properties `left` and `right` interfere, or whether
    /// one covers the other. When the `logicalToPhysical` option is enabled, the method
    /// also compares their physical names.
    fn properties_relate(&self, left: &str, right: &str) -> bool {
        properties_interfere(left, right)
            || self.covers(left, right)
            || self.covers(right, left)
            || (self.canonical_options().logical_to_physical
                && physical_properties(left).iter().any(|left| {
                    physical_properties(right)
                        .iter()
                        .any(|right| properties_interfere(left, right))
                }))
    }
}

/// Appends to `output` the names of the properties that `css` and its nested rules
/// declare. Standard property names are lowercase. `depth` is the nesting level of
/// `css`.
///
/// Returns false when `css` contains a statement that is neither a declaration nor an
/// at-rule, or when rules are nested deeper than 64 levels. `output` may then contain
/// some names.
fn nested_properties(css: &str, depth: usize, output: &mut Vec<LeanStr>) -> bool {
    if depth > 64 {
        return false;
    }
    if let Some(declarations) = Declaration::parse(css) {
        output.extend(
            declarations
                .into_iter()
                .map(|declaration| declaration.property),
        );
        return true;
    }
    let Ok(rules) = CssRule::parse(css) else {
        return false;
    };
    rules.iter().all(|rule| match rule.body.as_deref() {
        Some(body) => nested_properties(body, depth + 1, output),
        None if rule.prelude.trim_start().starts_with('@') => true,
        // `Declaration::parse` rejects some valid declarations, such as vendor
        // prefixed properties. Only the property name matters here.
        None => rule
            .prelude
            .split_once(':')
            .map(|(name, _)| name.trim())
            .filter(|name| {
                !name.is_empty()
                    && name
                        .chars()
                        .all(|character| is_name_character(character) || character == '\\')
            })
            .map(|name| {
                output.push(if name.starts_with("--") {
                    name.to_lean_str()
                } else {
                    name.to_ascii_lowercase().into()
                });
            })
            .is_some(),
    })
}

/// Returns every custom property name, such as `--tw-shadow`, that appears in `css`. A
/// name continues while it contains ASCII letters, digits, `-`, `_`, non-ASCII
/// characters, or escaped characters.
fn custom_property_names(css: &str) -> BTreeSet<LeanStr> {
    let mut names = BTreeSet::new();
    let mut start = 0;
    while let Some(offset) = css[start..].find("--") {
        let begin = start + offset;
        let preceded = css[..begin]
            .chars()
            .next_back()
            .is_some_and(|previous| is_name_character(previous) || previous == '\\');
        let mut end = begin + 2;
        let mut characters = css[end..].chars();
        while let Some(character) = characters.next() {
            if character == '\\' {
                end += 1;
                if let Some(escaped) = characters.next() {
                    end += escaped.len_utf8();
                }
            } else if is_name_character(character) {
                end += character.len_utf8();
            } else {
                break;
            }
        }
        if !preceded && end > begin + 2 {
            names.insert(css[begin..end].to_lean_str());
        }
        start = end.max(begin + 2);
    }
    names
}

/// Checks whether `character` can continue a custom property name without an escape.
fn is_name_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_') || !character.is_ascii()
}

/// Returns the members of each set in the union-find structure `parents`. Sets are
/// ordered by their first member, and members are in increasing order.
fn parts(parents: &mut [usize]) -> Vec<Vec<usize>> {
    let mut parts: Vec<(usize, Vec<usize>)> = Vec::new();
    for index in 0..parents.len() {
        let root = find(parents, index);
        match parts.iter_mut().find(|(part, _)| *part == root) {
            Some((_, members)) => members.push(index),
            None => parts.push((root, vec![index])),
        }
    }
    parts.into_iter().map(|(_, members)| members).collect()
}

/// Returns the representative of the set that contains `index`. The method also points
/// each member that it passes directly to the representative.
fn find(parents: &mut [usize], index: usize) -> usize {
    let mut root = index;
    while parents[root] != root {
        root = parents[root];
    }
    let mut current = index;
    while parents[current] != root {
        let next = parents[current];
        parents[current] = root;
        current = next;
    }
    root
}

/// Joins the sets that contain `left` and `right`. The smaller of the two
/// representatives represents the joined set.
fn union(parents: &mut [usize], left: usize, right: usize) {
    let left = find(parents, left);
    let right = find(parents, right);
    if left != right {
        parents[right.max(left)] = right.min(left);
    }
}

#[cfg(test)]
mod tests {
    use lean_string::LeanStr;

    use stillwind_design_system::DesignSystemSnapshot;
    use stillwind_source::{FileId, SourceFile};
    use stillwind_test_support::expand_css;

    use super::super::group::Group;
    use crate::{CompilerOptions, DesignSystemQuery};

    fn query() -> DesignSystemQuery {
        let source =
            SourceFile::new(FileId(1), None, None, expand_css("@import 'stillwind';")).unwrap();
        let options = CompilerOptions::default();
        let snapshot =
            DesignSystemSnapshot::from_source(&source, "theme.css".into(), options.design_system())
                .unwrap();
        DesignSystemQuery::new(snapshot, options)
    }

    fn group(values: &[&str]) -> Group {
        Group {
            prefix: LeanStr::new(),
            important: false,
            values: values.iter().map(|value| (*value).into()).collect(),
            positions: (0..values.len()).collect(),
        }
    }

    fn ordered(group: &Group) -> Vec<(usize, LeanStr)> {
        let mut output: Vec<_> = group
            .positions
            .iter()
            .copied()
            .zip(group.values.iter().cloned())
            .collect();
        output.sort();
        output
    }

    #[test]
    fn split_groups_match_the_complete_group() {
        let query = query();
        for values in [
            &["w-4", "text-sm", "h-4", "leading-7"][..],
            &["leading-7", "h-4", "text-sm", "w-4"],
            &[
                "overflow-x-hidden",
                "w-8",
                "overflow-y-hidden",
                "h-8",
                "flex",
            ],
            &[
                "text-sm",
                "w-4",
                "leading-7",
                "flex",
                "h-4",
                "overflow-y-auto",
                "overflow-x-auto",
            ],
        ] {
            let mut complete = group(values);
            let mut complete_position = values.len();
            query
                .collapse_component(&mut complete, &mut complete_position, &[])
                .unwrap();
            let mut split = group(values);
            let mut split_position = values.len();
            query
                .collapse_group(&mut split, &mut split_position)
                .unwrap();

            let declarations: Vec<_> = values
                .iter()
                .map(|value| query.candidate_declarations(value))
                .collect();
            let footprints: Vec<_> = values.iter().map(|value| query.footprint(value)).collect();
            let mut parents = query.collapse_links(&declarations, &footprints);
            assert!(super::parts(&mut parents).len() > 1, "{values:?}");
            assert!(complete_position >= values.len() + 2, "{values:?}");
            assert_eq!(ordered(&split), ordered(&complete), "{values:?}");
            assert_eq!(split_position, complete_position, "{values:?}");
        }
    }

    #[test]
    fn unrelated_vendor_prefixes_do_not_block_a_part() {
        let query = query();
        let values = ["z-10", "z-100", "box-decoration-clone"];
        let mut complete = group(&values);
        query
            .collapse_component(&mut complete, &mut values.len(), &[])
            .unwrap();
        assert_eq!(complete.values, values);

        assert_eq!(
            query
                .canonical_candidates(&[
                    "z-10".into(),
                    "z-[100]".into(),
                    "box-decoration-clone".into()
                ])
                .unwrap(),
            ["z-100", "box-decoration-clone"]
        );
    }
}
