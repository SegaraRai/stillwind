/// One parsed Tailwind-compatible candidate. The original spelling remains
/// available through [`Candidate::raw`] for stable hashing and diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    pub raw: String,
    pub variants: Vec<Variant>,
    pub important: bool,
    pub negative: bool,
    pub utility: Utility,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Variant {
    pub raw: String,
    pub kind: VariantKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VariantKind {
    Named(String),
    Arbitrary(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Utility {
    Named {
        /// The complete utility name before a `/` modifier. Registry matching
        /// determines the longest valid root, because grammar alone cannot
        /// distinguish `bg-red-500` from `translate-x-4`.
        name: String,
        modifier: Option<Value>,
    },
    ArbitraryProperty {
        property: String,
        value: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    Named(String),
    Arbitrary(String),
    CssVariable(String),
}
