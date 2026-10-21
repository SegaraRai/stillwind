//! Expansion of nesting selector templates at rule construction time.
//!
//! [`list`] validates selector lists and splits them into branches, [`target`]
//! classifies the element that each branch styles, [`subject`] finds, inserts, and
//! replaces the nesting selector `&`, [`element`] handles conditions on the selected
//! element, and [`classes`] lists the class names that a selector matches.

mod classes;
mod element;
mod list;
mod subject;
mod target;

pub use classes::selector_class_names;
pub use element::{
    append_element_condition, selector_is_element_condition, selector_is_subject_condition,
};
pub use list::{selector_is_valid, selector_list_branches};
pub use subject::{
    absolutize_nested_selector, implicit_nesting_positions, instantiate_selector,
    selector_repeats_subject, selector_uses_subject, subject_as_universal, subject_compound_parts,
};
pub use target::{SelectorTarget, selector_target};
