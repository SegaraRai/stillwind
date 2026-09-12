//! Verifies the actual coordinate units crossing the binding output boundary.

use stillwind_napi::compile_react_input;

#[test]
fn transform_mappings_use_utf16_even_with_multibyte_source_and_replacements() {
    let source = "const emoji = '😀'; import { style } from 'stillwind/react'; export const x = style('flex');";
    let output = compile_react_input(
        source,
        "entry.tsx",
        "@utility flex { display:flex; } @stillwind-slot utilities;",
    )
    .unwrap();
    assert!(output.diagnostics.is_empty());
    let call_byte = source.find("style('flex')").unwrap();
    let call_utf16 = u32::try_from(source[..call_byte].encode_utf16().count()).unwrap();
    assert_ne!(usize::try_from(call_utf16).unwrap(), call_byte);
    let mapping = output
        .mappings
        .iter()
        .find(|m| m.original_start == call_utf16)
        .unwrap();
    assert_eq!(mapping.original_end - mapping.original_start, 13);
    let code = output.code.unwrap();
    let generated = code.encode_utf16().collect::<Vec<_>>();
    let replacement = String::from_utf16(
        &generated[mapping.generated_start as usize..mapping.generated_end as usize],
    )
    .unwrap();
    assert!(replacement.contains("className"), "{replacement}");
}
