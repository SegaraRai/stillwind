//! Content fingerprints shared with source revisions and compiler cache identities.

use napi::bindgen_prelude::{Either, Uint8Array};
use napi_derive::napi;

use stillwind_source::Fingerprint;

use crate::output::NativeText;

/// Returns the compiler's lowercase hexadecimal fingerprint of UTF-8 text or raw bytes.
#[napi(ts_return_type = "string")]
pub fn fingerprint(content: Either<String, Uint8Array>) -> NativeText {
    match content {
        Either::A(text) => (Fingerprint::of(text).0).into(),
        Either::B(bytes) => Fingerprint::of(bytes.as_ref()).0.into(),
    }
}
