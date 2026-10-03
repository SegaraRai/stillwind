//! Readers for the checked-in Markdown fixture cases and catalogs.
//!
//! Case names use level-two headings with backticks. Case fields use level-three
//! headings followed by fenced code blocks. These readers expect LF line endings.

/// Returns the whitespace-separated candidates in `case_name`, joined by spaces.
/// `fixture` must contain a named case with a fenced `Candidates` text block.
///
/// # Panics
///
/// Panics if the case or its closed candidates block is missing.
pub fn candidates(fixture: &str, case_name: &str) -> String {
    code_block(case_named(fixture, case_name), "Candidates", "text")
        .expect("fixture candidates must exist")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Returns the trimmed body of a fenced block under `heading` in `section`.
/// `heading` excludes the Markdown heading prefix, and `language` names the fence.
/// Returns `None` if the field or its closing fence is missing.
pub fn code_block(section: &str, heading: &str, language: &str) -> Option<String> {
    let marker = format!("### {heading}\n\n```{language}\n");
    let start = section.find(&marker)? + marker.len();
    Some(section[start..].split_once("\n```")?.0.trim().to_owned())
}

/// Returns the first case whose `Candidates` text block contains `candidate`.
/// Candidates match complete whitespace-separated words. The returned slice starts
/// at the case name and ends before the next case heading.
///
/// # Panics
///
/// Panics if no case contains `candidate`.
pub fn case_for_candidate<'a>(fixture: &'a str, candidate: &str) -> &'a str {
    fixture
        .split("## `")
        .skip(1)
        .find(|section| {
            code_block(section, "Candidates", "text").is_some_and(|candidates| {
                candidates
                    .split_whitespace()
                    .any(|value| value == candidate)
            })
        })
        .unwrap_or_else(|| panic!("fixture case for `{candidate}` must exist"))
}

/// Returns the case named `name` from `fixture`.
/// The returned slice starts at the case name and ends before the next case heading.
///
/// # Panics
///
/// Panics if the named case is missing.
pub fn case_named<'a>(fixture: &'a str, name: &str) -> &'a str {
    fixture
        .split("## `")
        .skip(1)
        .find(|section| section.starts_with(&format!("{name}`")))
        .unwrap_or_else(|| panic!("fixture case `{name}` must exist"))
}

/// Returns the whitespace-separated rows in a catalog section's first text fence.
/// `heading` includes the level-two Markdown prefix and may omit a trailing count.
/// The section ends at the next level-two heading, so a missing block cannot select
/// rows from another section.
///
/// # Panics
///
/// Panics if the heading or its closed text block is missing.
pub fn catalog_section<'a>(markdown: &'a str, heading: &str) -> Vec<&'a str> {
    let start = markdown.find(heading).expect("catalog section must exist") + heading.len();
    let section = markdown[start..].split("\n## ").next().unwrap();
    let start = section
        .find("```text\n")
        .expect("catalog section must contain candidates")
        + "```text\n".len();
    section[start..]
        .split_once("\n```")
        .expect("catalog section must close")
        .0
        .split_whitespace()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CASES: &str = "## `one`\n\n### Candidates\n\n```text\np-4  px-4\n```\n\n### Input CSS\n\n```css\n @theme {} \n```\n\n## `two`\n\n### Candidates\n\n```text\np-40\n```\n";

    #[test]
    fn case_readers_select_exact_names_candidates_and_fields() {
        let first = case_named(CASES, "one");
        assert_eq!(candidates(CASES, "one"), "p-4 px-4");
        assert_eq!(case_for_candidate(CASES, "p-4"), first);
        assert_eq!(case_for_candidate(CASES, "p-40"), case_named(CASES, "two"));
        assert_eq!(
            code_block(first, "Input CSS", "css"),
            Some("@theme {}".into())
        );
        assert_eq!(code_block(first, "Expected CSS", "css"), None);
        assert_eq!(code_block(first, "Input CSS", "text"), None);
    }

    #[test]
    fn catalogs_return_rows_only_from_the_selected_section() {
        let catalog =
            "## First (2)\n\n```text\np-4\npx-4\n```\n\n## Second\n\n```text\np-40\n```\n";
        assert_eq!(catalog_section(catalog, "## First"), ["p-4", "px-4"]);
        assert_eq!(catalog_section(catalog, "## First (2)"), ["p-4", "px-4"]);
        assert_eq!(catalog_section(catalog, "## Second"), ["p-40"]);
    }

    #[test]
    #[should_panic(expected = "fixture candidates must exist")]
    fn missing_candidates_do_not_select_the_next_case() {
        candidates(
            "## `one`\n\n## `two`\n\n### Candidates\n\n```text\np-4\n```\n",
            "one",
        );
    }

    #[test]
    #[should_panic(expected = "catalog section must contain candidates")]
    fn missing_catalog_blocks_do_not_select_the_next_section() {
        catalog_section("## First\n\n## Second\n\n```text\np-4\n```\n", "## First");
    }
}
