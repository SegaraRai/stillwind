//! Converts arbitrary values between the spelling in a candidate and CSS, as Tailwind
//! does. For example, `calc(100%_-_1rem)` in `w-[calc(100%_-_1rem)]` becomes
//! `calc(100% - 1rem)`.
//!
//! The rules follow Tailwind CSS 4.3.3 (`utils/decode-arbitrary-value.ts`,
//! `value-parser.ts`, and `utils/math-operators.ts`). Keep them aligned with Tailwind,
//! because a candidate written for Tailwind must give the same CSS.
//!
//! Modules:
//!
//! - [`decode`] converts the spelling in a candidate to CSS.
//! - [`mod@print`] converts CSS back to a short spelling for a candidate.
//! - [`value`] splits a value into words, separators, and functions.

mod decode;
mod print;
mod value;

pub use decode::decode_arbitrary_value;
pub use print::print_arbitrary_value;
