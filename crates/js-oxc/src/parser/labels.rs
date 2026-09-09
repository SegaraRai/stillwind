use super::Spanned;

pub(super) fn is_strict_ancestor(ancestor: &[usize], descendant: &[usize]) -> bool {
    ancestor.len() < descendant.len() && descendant.starts_with(ancestor)
}

pub(super) fn is_marker_label(label: &Spanned<String>) -> bool {
    !label.value.is_empty()
        && label
            .value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

pub(super) fn is_animation_frame(value: &str) -> bool {
    value == "from"
        || value == "to"
        || value.split(',').all(|part| {
            part.trim()
                .strip_suffix('%')
                .is_some_and(|number| number.trim().parse::<u8>().is_ok())
        })
}
