pub(super) fn extract_authored_keyframes(css: &str, names: &[String]) -> String {
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

pub(super) fn without_keyframes(css: &str) -> String {
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
