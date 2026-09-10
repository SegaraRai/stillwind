use serde::{Deserialize, Serialize};

use super::symbolic::ExportedValue;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TypeFact {
    pub start: u32,
    pub end: u32,
    pub value: ExportedValue,
}
