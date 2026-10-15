//! Transfers owned UTF-8 text without changing its Rust allocation before Node-API copies it.

use std::{fmt, ops::Deref, sync::Arc};

use lean_string::{LeanStr, LeanString};
use napi::bindgen_prelude::{ToNapiValue, TypeName};
use napi::{ValueType, sys};
use serde::{Serialize, Serializer};

use stillwind_source::{CssSourceName, SourceFile, SourceUri};

/// Owned UTF-8 text that Node-API copies directly into a JavaScript string.
#[derive(Clone, Debug)]
pub struct NativeText(TextStorage);

impl NativeText {
    /// Borrows the UTF-8 contents for serialization.
    pub fn as_str(&self) -> &str {
        match &self.0 {
            TextStorage::Shared(value) => value,
            TextStorage::Mutable(value) => value,
            TextStorage::Owned(value) => value,
            TextStorage::Source(source) => source.text(),
            TextStorage::Uri(uri) => uri.as_str(),
            TextStorage::SourceName(name) => name.as_str(),
        }
    }

    pub(crate) fn source_text(source: Arc<SourceFile>) -> Self {
        Self(TextStorage::Source(source))
    }
}

impl From<SourceUri> for NativeText {
    fn from(value: SourceUri) -> Self {
        Self(TextStorage::Uri(value))
    }
}

impl From<CssSourceName> for NativeText {
    fn from(value: CssSourceName) -> Self {
        Self(TextStorage::SourceName(value))
    }
}

impl From<LeanStr> for NativeText {
    fn from(value: LeanStr) -> Self {
        Self(TextStorage::Shared(value))
    }
}

impl From<LeanString> for NativeText {
    fn from(value: LeanString) -> Self {
        Self(TextStorage::Mutable(value))
    }
}

impl From<String> for NativeText {
    fn from(value: String) -> Self {
        Self(TextStorage::Owned(value))
    }
}

impl From<&str> for NativeText {
    fn from(value: &str) -> Self {
        LeanStr::from(value).into()
    }
}

impl Deref for NativeText {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for NativeText {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for NativeText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl<T: AsRef<str> + ?Sized> PartialEq<T> for NativeText {
    fn eq(&self, other: &T) -> bool {
        self.as_str() == other.as_ref()
    }
}

impl Eq for NativeText {}

impl Serialize for NativeText {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl TypeName for NativeText {
    fn type_name() -> &'static str {
        "String"
    }

    fn value_type() -> ValueType {
        ValueType::String
    }
}

impl ToNapiValue for NativeText {
    #[allow(unsafe_code)]
    unsafe fn to_napi_value(env: sys::napi_env, value: Self) -> napi::Result<sys::napi_value> {
        // Node-API copies the UTF-8 bytes before returning, while `value` still owns them.
        unsafe { <&str as ToNapiValue>::to_napi_value(env, value.as_str()) }
    }
}

#[derive(Clone, Debug)]
enum TextStorage {
    Shared(LeanStr),
    Mutable(LeanString),
    Owned(String),
    Source(Arc<SourceFile>),
    Uri(SourceUri),
    SourceName(CssSourceName),
}

#[cfg(test)]
mod tests {
    use stillwind_source::FileId;

    use super::*;

    #[test]
    fn serializes_text_buffers_and_sources_as_json_strings() {
        let text = "a long string with Unicode \u{1f600}, quotes \" and a NUL \0";
        let expected = serde_json::to_string(text).unwrap();
        for value in [
            NativeText::from(LeanStr::from(text)),
            NativeText::from(LeanString::from(text)),
            NativeText::from(text.to_owned()),
            NativeText::source_text(Arc::new(
                SourceFile::new(FileId(1), None, None, text).unwrap(),
            )),
        ] {
            assert_eq!(serde_json::to_string(&value).unwrap(), expected);
            assert_eq!(value.as_str(), text);
        }
    }
}
