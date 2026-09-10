//! Framework policy, independent of parsers and host converters.

use serde::{Deserialize, Serialize};

mod astro;
mod preact;
mod react;
mod solid;
mod svelte;
mod vue;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AdapterKind {
    React,
    Preact,
    Solid,
    Vue,
    Svelte,
    Astro,
}

pub struct FrameworkSpec {
    pub id: AdapterKind,
    pub macro_import: &'static str,
    pub class_property: &'static str,
    /// Enabled only after the original-source projection path is validated.
    pub projected_input: bool,
}

pub const FRAMEWORKS: &[&FrameworkSpec] = &[
    &react::SPEC,
    &preact::SPEC,
    &solid::SPEC,
    &vue::SPEC,
    &svelte::SPEC,
    &astro::SPEC,
];

impl AdapterKind {
    pub const fn spec(self) -> &'static FrameworkSpec {
        match self {
            Self::React => &react::SPEC,
            Self::Preact => &preact::SPEC,
            Self::Solid => &solid::SPEC,
            Self::Vue => &vue::SPEC,
            Self::Svelte => &svelte::SPEC,
            Self::Astro => &astro::SPEC,
        }
    }
}

pub fn from_macro_import(source: &str) -> Option<&'static FrameworkSpec> {
    FRAMEWORKS
        .iter()
        .copied()
        .find(|spec| spec.macro_import == source)
}
