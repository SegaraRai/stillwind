/// Development-only runtime helpers for lowered Stillwind calls.
///
/// The compiler imports the checked helper under its own export name when it compiles in
/// development mode, so the release helper stays free of checks. The specifier has no
/// export condition: the mode is a compiler input, and a condition would resolve twice,
/// once for the module and once for the name it exports.

export { stillwindClsxChecked } from "./shared.ts";
