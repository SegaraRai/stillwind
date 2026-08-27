#[test]
fn serializes_a_layered_compile_result_for_node_consumers() {
    let output = shorth_napi::compile_react_json(
        r#"import { style } from "stillwind/react";
const props = style("bg-accent hover:bg-accent/90");"#,
        "Button.tsx",
        "@theme { --color-accent: oklch(.6 .2 250); }",
    )
    .unwrap();

    let value: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert!(value["css"].as_str().unwrap().contains("@layer utilities{"));
    assert!(value["css"].as_str().unwrap().contains(":hover{"));
    assert!(value["code"].as_str().unwrap().contains("className"));
}
