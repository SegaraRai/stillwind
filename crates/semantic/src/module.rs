use std::collections::BTreeMap;

use serde::Serialize;

use stillwind_source::Fingerprint;

use crate::{AssetMetadata, IdentityId, TypeFact};

/// Host-provided module identity, preserving the source path verbatim.
///
/// ```compile_fail
/// use stillwind_semantic::ModuleId;
/// let module: ModuleId = String::from("entry.tsx");
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[serde(transparent)]
pub struct ModuleId(pub String);

impl ModuleId {
    pub fn as_str(&self) -> &str {
        &self.0
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

impl std::borrow::Borrow<str> for ModuleId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone, Debug)]
pub struct TypeFacts {
    pub schema: u32,
    pub module_id: ModuleId,
    pub source_revision: Fingerprint,
    pub semantic_revision: Fingerprint,
    pub facts: Vec<TypeFact>,
    pub assets: BTreeMap<IdentityId, AssetMetadata>,
}

impl TypeFacts {
    pub const SCHEMA: u32 = 1;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegrationDiagnostic {
    pub module_id: Option<ModuleId>,
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub struct TypeFactsUpdate {
    pub modules: BTreeMap<ModuleId, TypeFacts>,
    pub diagnostics: Vec<IntegrationDiagnostic>,
}
