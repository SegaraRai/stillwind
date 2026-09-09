use super::nested::closing_brace;
use super::variants::VariantApplication;
use super::variants::VariantDeclaration;

pub(super) fn resolve_variant_body(body: &str) -> Option<VariantApplication> {
    if body.is_empty() || !body.contains("@slot") {
        return None;
    }
    let blocks = split_variant_blocks(body)?;
    if blocks.len() > 1 {
        return blocks
            .into_iter()
            .map(resolve_variant_body)
            .collect::<Option<Vec<_>>>()
            .map(VariantApplication::Compound);
    }
    if body.contains("@starting-style") {
        return Some(VariantApplication::StartingStyle);
    }
    if let Some(start) = body.find("@media ") {
        let condition_start = start + "@media ".len();
        let condition_end = body[condition_start..].find('{')? + condition_start;
        let media_end = closing_brace(body, condition_end)?;
        let condition = body[condition_start..condition_end].trim().to_owned();
        let nested = &body[condition_end + 1..media_end];
        if let Some(rule) = selector_rule_from_variant_body(nested) {
            return if rule.declarations.is_empty() {
                Some(VariantApplication::SelectorMedia {
                    selector: rule.selector,
                    condition,
                })
            } else {
                Some(VariantApplication::SelectorMediaRule {
                    selector: rule.selector,
                    declarations: rule.declarations,
                    condition,
                })
            };
        }
        return Some(VariantApplication::Media(condition));
    }
    if let Some(start) = body.find("@container ") {
        let condition_start = start + "@container ".len();
        let condition_end = body[condition_start..].find('{')? + condition_start;
        return Some(VariantApplication::Container(
            body[condition_start..condition_end].trim().to_owned(),
        ));
    }
    if let Some(start) = body.find("@supports ") {
        let condition_start = start + "@supports ".len();
        let condition_end = body[condition_start..].find('{')? + condition_start;
        let condition = body[condition_start..condition_end]
            .trim()
            .strip_prefix('(')
            .and_then(|condition| condition.strip_suffix(')'))?
            .trim()
            .to_owned();
        return Some(VariantApplication::Supports(condition));
    }
    let rule = selector_rule_from_variant_body(body)?;
    if rule.declarations.is_empty() {
        Some(VariantApplication::SelectorTemplate(rule.selector))
    } else {
        Some(VariantApplication::SelectorRule {
            selector: rule.selector,
            declarations: rule.declarations,
        })
    }
}

pub(super) fn split_variant_blocks(body: &str) -> Option<Vec<&str>> {
    let mut blocks = Vec::new();
    let mut cursor = 0;
    while cursor < body.len() {
        let start = body[cursor..]
            .char_indices()
            .find(|(_, character)| !character.is_whitespace())
            .map_or(body.len(), |(offset, _)| cursor + offset);
        if start == body.len() {
            break;
        }
        let open = body[start..].find('{')? + start;
        let close = closing_brace(body, open)?;
        blocks.push(&body[start..=close]);
        cursor = close + 1;
    }
    (!blocks.is_empty()).then_some(blocks)
}

pub(super) struct VariantSelectorRule {
    pub(super) selector: String,
    pub(super) declarations: Vec<VariantDeclaration>,
}

pub(super) fn selector_rule_from_variant_body(body: &str) -> Option<VariantSelectorRule> {
    let open = body.find('{')?;
    let close = closing_brace(body, open)?;
    if !body[close + 1..].trim().is_empty() {
        return None;
    }
    let selector = body[..open].trim();
    if selector.is_empty() || !selector.contains('&') {
        return None;
    }

    let inner = &body[open + 1..close];
    let mut declarations = Vec::new();
    let mut start = 0;
    let mut depth = 0_u32;
    for (offset, character) in inner.char_indices() {
        match character {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.checked_sub(1)?,
            '{' | '}' => return None,
            ';' if depth == 0 => {
                let node = inner[start..offset].trim();
                start = offset + 1;
                if node.is_empty() || node == "@slot" {
                    continue;
                }
                let (property, value) = node.split_once(':')?;
                let property = property.trim();
                let value = value.trim();
                if property.is_empty() || value.is_empty() {
                    return None;
                }
                declarations.push(VariantDeclaration {
                    property: property.to_owned(),
                    value: value.to_owned(),
                });
            }
            _ => {}
        }
    }
    if !inner[start..].trim().is_empty() {
        return None;
    }
    Some(VariantSelectorRule {
        selector: selector.to_owned(),
        declarations,
    })
}
