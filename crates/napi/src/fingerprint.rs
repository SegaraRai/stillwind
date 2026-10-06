//! Content fingerprints shared with source revisions and compiler cache identities.

use napi::bindgen_prelude::{Either, Uint8Array};
use napi_derive::napi;

use stillwind_source::Fingerprint;

/// Returns the compiler's lowercase hexadecimal fingerprint of UTF-8 text or raw bytes.
#[napi]
pub fn fingerprint(content: Either<String, Uint8Array>) -> String {
    match content {
        Either::A(text) => Fingerprint::of(text).0,
        Either::B(bytes) => Fingerprint::of(bytes.as_ref()).0,
    }
}
