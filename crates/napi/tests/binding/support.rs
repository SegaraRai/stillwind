//! Shared source builders and result helpers for these contracts.

use super::*;

pub(super) fn compile_input(source: &str, module_id: &str, theme: &str) -> CompileSourceInput {
    CompileSourceInput {
        source: source.into(),
        module_id: module_id.into(),
        uri: SourceUri::parse("stillwind-input:source").unwrap(),
        source_map_source: Some(module_id.into()),
        language: ScriptLanguage::Tsx,
        module_kind: ScriptModuleKind::Unambiguous,
        css: css_input(theme),
    }
}

pub(super) fn css_input(content: &str) -> CompileCssInput {
    CompileCssInput {
        content: content.into(),
        name: "stillwind.css".into(),
        uri: SourceUri::parse("stillwind-input:theme").unwrap(),
        source_map_source: Some("stillwind.css".into()),
    }
}

pub(super) fn sources_from_value(mut value: serde_json::Value) -> Vec<HostPreparedSource> {
    for source in value.as_array_mut().unwrap() {
        let path = source["moduleId"].as_str().unwrap().to_owned();
        let uri = format!("test:///{path}");
        source["document"]["uri"] = serde_json::Value::String(uri.clone());
        source["document"]["sourceMapSource"] = serde_json::Value::String(uri);
    }
    serde_json::from_value(value).unwrap()
}
