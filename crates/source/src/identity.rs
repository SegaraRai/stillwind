//! Opaque host identities shared by source consumers.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// Host-provided identity of a script module, compared without normalization.
///
/// Hosts must use one stable spelling per module in a project session. The serialized
/// form is the original string, without a type tag or path conversion.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModuleId(String);

impl ModuleId {
    /// Borrows the exact host key.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the host key as an owned string.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for ModuleId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl From<String> for ModuleId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ModuleId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

/// Host-provided identity of one CSS source buffer, compared without path semantics.
///
/// Two loader responses with the same name identify the same source; differing
/// content under one name is a protocol error. The serialized form is the original
/// string, without a type tag or path conversion.
#[derive(Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CssSourceName(Arc<str>);

impl CssSourceName {
    /// Borrows the exact host key.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CssSourceName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl From<Arc<str>> for CssSourceName {
    fn from(value: Arc<str>) -> Self {
        Self(value)
    }
}

impl From<String> for CssSourceName {
    fn from(value: String) -> Self {
        Self(value.into())
    }
}

impl From<&str> for CssSourceName {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinct_host_identities_serialize_as_their_original_strings() {
        let module = ModuleId::from(r"src\Panel.tsx");
        let css = CssSourceName::from(r"src\Panel.tsx");
        assert_eq!(
            serde_json::to_string(&module).unwrap(),
            r#""src\\Panel.tsx""#
        );
        assert_eq!(serde_json::to_string(&css).unwrap(), r#""src\\Panel.tsx""#);
        assert_eq!(
            serde_json::from_str::<ModuleId>(r#""src\\Panel.tsx""#).unwrap(),
            module
        );
        assert_eq!(
            serde_json::from_str::<CssSourceName>(r#""src\\Panel.tsx""#).unwrap(),
            css
        );
    }
}
