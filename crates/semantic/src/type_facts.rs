use super::symbolic::ExportedValue;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TypeFact {
    pub start: u32,
    pub end: u32,
    pub value: ExportedValue,
}
