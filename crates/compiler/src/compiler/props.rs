use serde::Serialize;

use stillwind_ir::{AdapterKind, AttributeEntry};
use stillwind_source::{SourceFile, Span};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeneratedClass {
    pub class_name: String,
    pub call_span: Span,
    pub guard: Option<Span>,
    pub guard_when_true: bool,
}

#[derive(Clone, Debug)]
pub(in crate::compiler) struct ViewTransitionStyleEntry {
    pub(in crate::compiler) value: Option<Span>,
    pub(in crate::compiler) identity: Option<String>,
    pub(in crate::compiler) candidates: Vec<String>,
    pub(in crate::compiler) key: Option<Span>,
}

#[derive(Clone, Debug)]
pub(in crate::compiler) struct DynamicAssetClassEntry {
    pub(in crate::compiler) value: Span,
    pub(in crate::compiler) classes: Vec<(String, String)>,
}

#[derive(Clone, Copy)]
pub(in crate::compiler) struct PropsParts<'a> {
    pub(in crate::compiler) classes: &'a [GeneratedClass],
    pub(in crate::compiler) attributes: &'a [AttributeEntry],
    pub(in crate::compiler) handle_markers: &'a [String],
    pub(in crate::compiler) external_classes: &'a [Span],
    pub(in crate::compiler) dynamic_handle_classes: &'a [Span],
    pub(in crate::compiler) dynamic_asset_classes: &'a [DynamicAssetClassEntry],
    pub(in crate::compiler) view_transition_style: Option<&'a ViewTransitionStyleEntry>,
}

fn guarded_class(source: &SourceFile, class: &GeneratedClass) -> Option<String> {
    let Some(span) = class.guard else {
        return Some(format!("\"{}\"", class.class_name));
    };
    let guard = source.slice(span).ok()?;
    let guard = if guard
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
    {
        guard.to_owned()
    } else {
        format!("({guard})")
    };
    let negation = if class.guard_when_true { "" } else { "!" };
    Some(format!("{negation}{guard}&&\"{}\"", class.class_name))
}

pub(in crate::compiler) fn render_props(
    source: &SourceFile,
    parts: &PropsParts<'_>,
    adapter: AdapterKind,
) -> Option<String> {
    let PropsParts {
        classes,
        attributes,
        handle_markers,
        external_classes,
        dynamic_handle_classes,
        dynamic_asset_classes,
        view_transition_style,
    } = *parts;
    let mut entries = Vec::new();
    if !classes.is_empty()
        || !handle_markers.is_empty()
        || !external_classes.is_empty()
        || !dynamic_handle_classes.is_empty()
        || !dynamic_asset_classes.is_empty()
    {
        let mut class_names = handle_markers
            .iter()
            .map(|marker| format!("\"{marker}\""))
            .collect::<Vec<_>>();
        class_names.extend(
            classes
                .iter()
                .map(|class| guarded_class(source, class))
                .collect::<Option<Vec<_>>>()?,
        );
        class_names.extend(
            external_classes
                .iter()
                .map(|value| source.slice(*value).ok().map(str::to_owned))
                .collect::<Option<Vec<_>>>()?,
        );
        class_names.extend(
            dynamic_handle_classes
                .iter()
                .map(|value| {
                    source
                        .slice(*value)
                        .ok()
                        .map(|value| format!("stillwindHandleClass({value})"))
                })
                .collect::<Option<Vec<_>>>()?,
        );
        class_names.extend(
            dynamic_asset_classes
                .iter()
                .map(|entry| {
                    let value = source.slice(entry.value).ok()?;
                    let classes = entry
                        .classes
                        .iter()
                        .map(|(identity, class_name)| format!("\"{identity}\":\"{class_name}\""))
                        .collect::<Vec<_>>()
                        .join(",");
                    Some(format!("stillwindAssetClass({value},{{{classes}}})"))
                })
                .collect::<Option<Vec<_>>>()?,
        );
        let class_value = if external_classes.is_empty() && class_names.len() == 1 {
            class_names.into_iter().next()?
        } else {
            format!("stillwindClsx({})", class_names.join(","))
        };
        let property = adapter.spec().class_property;
        entries.push(format!("{property}:{class_value}"));
    }
    for attribute in attributes {
        let value = source.slice(attribute.value).ok()?;
        entries.push(format!("\"{}\":{value}", attribute.name.value));
    }
    if let Some(entry) = view_transition_style {
        let value = match (entry.value, &entry.identity) {
            (Some(value), None) => source.slice(value).ok()?.to_owned(),
            (None, Some(identity)) => format!("\"{identity}\""),
            _ => return None,
        };
        let key = match entry.key {
            Some(key) => source.slice(key).ok()?.to_owned(),
            None => "undefined".to_owned(),
        };
        let candidates = entry
            .candidates
            .iter()
            .map(|candidate| format!("\"{candidate}\""))
            .collect::<Vec<_>>()
            .join(",");
        entries.push(format!(
            "style:stillwindViewTransitionStyle({value},{key},[{candidates}],\"--sw-vt-name\",\"--sw-vt-class\")"
        ));
    }
    Some(format!("{{{}}}", entries.join(",")))
}
