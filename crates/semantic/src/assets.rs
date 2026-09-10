use serde::{Deserialize, Serialize};

use super::identity::IdentityId;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetMetadata {
    pub label: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetRecord {
    pub identity: IdentityId,
    pub metadata: AssetMetadata,
    pub call_start: u32,
    pub call_end: u32,
}
