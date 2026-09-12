//! The source-map research addon: the same algorithms the subprocess test and
//! benchmark run, exposed to Node.
//!
//! The crate exists to answer one question with measurements instead of intuition:
//! how expensive source-map emission is for the edit shapes Stillwind produces.
//! `main.rs` is included rather than duplicated, so the benchmark binary and the
//! N-API addon always run the same code.
//!
//! This is an isolated research addon, not a new Stillwind public API: it is not a
//! workspace member, and nothing in the compiler may depend on it.

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
