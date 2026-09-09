pub(in crate::compiler) fn extract_authored_keyframes(css: &str, names: &[String]) -> String {
    let mut output = String::new();
    for name in names {
        let needle = format!("@keyframes {name}");
        let Some(start) = css.find(&needle) else {
            continue;
        };
        let Some(open_rel) = css[start..].find('{') else {
            continue;
        };
        let open = start + open_rel;
        let mut depth = 0_u32;
        let mut close = None;
        for (offset, ch) in css[open..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        close = Some(open + offset);
                        break;
                    }
                }
                _ => {}
            }
        }
        if let Some(close) = close {
            output.push_str(&css[start..=close]);
        }
    }
    output
}

pub(in crate::compiler) fn without_keyframes(css: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    while let Some(relative) = css[cursor..].find("@keyframes ") {
        let start = cursor + relative;
        output.push_str(&css[cursor..start]);
        let Some(open_rel) = css[start..].find('{') else {
            break;
        };
        let open = start + open_rel;
        let mut depth = 0_u32;
        let mut close = None;
        for (offset, ch) in css[open..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        close = Some(open + offset);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(close) = close else { break };
        cursor = close + 1;
    }
    output.push_str(&css[cursor..]);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_requested_frames_and_preserves_non_frame_css() {
        let css = "a{}@keyframes spin {from{x:0}to{x:1}}b{}@keyframes fade {to{x:0}}";
        assert_eq!(
            extract_authored_keyframes(css, &["fade".into(), "missing".into()]),
            "@keyframes fade {to{x:0}}"
        );
        assert_eq!(without_keyframes(css), "a{}b{}");
        assert_eq!(
            extract_authored_keyframes("@keyframes x {", &["x".into()]),
            ""
        );
    }
}
