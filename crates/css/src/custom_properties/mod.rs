pub(super) mod references;
pub(super) mod registration;
pub(super) mod usage;

pub use references::custom_property_references;
pub use registration::{
    CustomPropertyActivation, CustomPropertyDefinition, extract_authored_property_names,
    extract_property_definitions, parse_property_definitions,
};
pub use usage::{
    CustomPropertyUsage, PropertyUsageError, custom_property_usage, referenced_custom_properties,
    used_custom_properties,
};
