use super::blocks::matching_brace;

/// Browser-ready Preflight with Tailwind's build-time `--theme()` calls
/// lowered to standard `var()` calls.
#[must_use]
pub fn tailwind_v4_preflight() -> String {
    TAILWIND_V4_PREFLIGHT.replace("--theme(", "var(")
}

/// The standard animation keyframes paired with the vendored default theme.
#[must_use]
pub fn tailwind_v4_keyframes() -> String {
    let mut output = String::new();
    let mut cursor = 0;
    while let Some(relative) = TAILWIND_V4_THEME[cursor..].find("@keyframes") {
        let start = cursor + relative;
        let Some(open_relative) = TAILWIND_V4_THEME[start..].find('{') else {
            break;
        };
        let open = start + open_relative;
        let mut depth = 1_u32;
        let mut close = open;
        for (offset, character) in TAILWIND_V4_THEME[open + 1..].char_indices() {
            match character {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        close = open + 1 + offset;
                        break;
                    }
                }
                _ => {}
            }
        }
        output.push_str(&TAILWIND_V4_THEME[start..=close]);
        cursor = close + 1;
    }
    output
}

/// Returns only the vendored keyframes named by the selected animation theme
/// tokens. Keeping this beside the theme source makes tree-shaking a semantic
/// property of the registry rather than a fixture-specific post-process.
#[must_use]
pub fn tailwind_v4_keyframes_for(names: &[String]) -> String {
    let mut output = String::new();
    for name in names {
        let needle = format!("@keyframes {name}");
        let Some(start) = TAILWIND_V4_THEME
            .match_indices(&needle)
            .find_map(|(start, _)| {
                TAILWIND_V4_THEME[start + needle.len()..]
                    .trim_start()
                    .starts_with('{')
                    .then_some(start)
            })
        else {
            continue;
        };
        let Some(open_relative) = TAILWIND_V4_THEME[start..].find('{') else {
            continue;
        };
        let open = start + open_relative;
        let Some(close) = matching_brace(TAILWIND_V4_THEME, open) else {
            continue;
        };
        output.push_str(&TAILWIND_V4_THEME[start..=close]);
    }
    output
}

/// Tailwind CSS v4.3.3 Preflight, vendored under its MIT license.
pub const TAILWIND_V4_PREFLIGHT: &str = include_str!("../../../css/preflight.css");

pub(super) const TAILWIND_V4_THEME: &str = include_str!("../../../css/theme.css");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_keyframes_require_the_complete_name_and_preserve_selection_order() {
        assert!(tailwind_v4_keyframes_for(&[]).is_empty());
        for name in ["missing", "sp", "spin-extra"] {
            assert!(
                tailwind_v4_keyframes_for(&[name.into()]).is_empty(),
                "{name}"
            );
        }
        let selected = tailwind_v4_keyframes_for(&["spin".into(), "ping".into()]);
        assert!(selected.starts_with("@keyframes spin"));
        assert!(selected.contains("@keyframes ping"));
        assert!(!selected.contains("@keyframes bounce"));
    }
}
