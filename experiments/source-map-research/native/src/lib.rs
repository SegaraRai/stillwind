// Share the exact experimental algorithms with the subprocess test/benchmark.
// This is an isolated research addon, not a new Stillwind public API.
#![allow(dead_code)]
include!("main.rs");

#[napi_derive::napi]
pub fn emit_source(input: String) -> napi::Result<String> {
    let input: SourceInput =
        serde_json::from_str(&input).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let (code, map) = emit(&input);
    Ok(
        json!({"code":code,"map":serde_json::from_str::<serde_json::Value>(&map).unwrap()})
            .to_string(),
    )
}

#[napi_derive::napi]
pub fn encode_tokens(input: String) -> napi::Result<String> {
    let input: Input =
        serde_json::from_str(&input).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(encode(&input))
}
