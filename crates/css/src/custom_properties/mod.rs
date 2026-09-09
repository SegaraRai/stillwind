pub(super) mod references;
pub(super) mod registration;
pub(super) mod usage;

pub use registration::{
    CustomPropertyActivation, CustomPropertyDefinition, parse_property_definitions,
};
pub use usage::{
    CustomPropertyUsage, PropertyUsageError, custom_property_usage, referenced_custom_properties,
    used_custom_properties,
};
