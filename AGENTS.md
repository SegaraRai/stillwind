# Guideline For Agents

## Tidy First

- Before changing behavior, tidy related code where useful; preserve behavior and keep structural edits distinct from functional edits.
- Implement an identified improvement when it needs at most 20 added or modified implementation lines; deleted lines are unlimited.
  Count the complete improvement across files, without splitting it to evade the limit; count contract tests separately below.
- First confirm unit tests cover the affected operation's required contracts. If coverage is missing, add it only if the necessary tests fit within
  30 lines; otherwise skip the improvement. Run the relevant contract tests and ensure they pass.
- When either limit prevents an improvement, add a TODO at the affected operation in at most 3 lines, specifying the concrete fix, its rationale,
  and a realistic, reproducible case that causes an actual problem. Do not invent hypothetical problems to justify cleanup.

## Responsibility and Colocation

- Actively colocate new code by related responsibility, not by item kind. Define errors and types alongside the code that constructs them;
  do not create kind-based files such as `errors.rs` or `types.rs`. Keep each struct immediately followed by its impl blocks.
- If existing or newly added code falls outside its responsibility, add a TODO of at most 3 lines in the aggregating file (`mod.rs`, `lib.rs`,
  `index.ts`, etc.), naming the specific functions, structs, or other items and explaining which responsibility they violate and where they belong.
- Within each source file, put error definitions first, then items of greatest interest to users, each followed by its supporting items;
  repeat this main-item/supporting-items grouping instead of collecting all helpers or types separately.

## TODO Review

- When a TODO appears incorrect, gather objective evidence for both its claim and the rebuttal. Have an independent Sol or Luna subagent judge
  both claims with no conversation history (`fork_turns="none"`); provide the relevant code and evidence without steering its conclusion.
- Only if that review validates the rebuttal, replace the TODO with an explanatory comment of at most 3 lines: "It may appear that X, but Y,
  because Z." State the concrete evidence clearly enough to prevent the same mistaken TODO from being reintroduced; otherwise retain the TODO.

## Comment Guidelines

- Describe the current codebase objectively for its readers. Do not record session history, received instructions, or the changes made in the current task.
- Crate and module doc comments explain their purpose, responsibilities, non-goals, and main entry points with usage guidance. In `lib.rs` / `mod.rs`, also
  describe each module's role. Omit entry-point guidance when there are no main entry points.
- Function doc comments explain purpose and input/output contracts. Document necessary state preconditions and postconditions for state changes, error
  conditions when returning `Result`, and panic conditions when applicable.
- Struct and enum doc comments explain the represented concept and its purpose. Describe usage domains or potential misuse only when useful. Field comments
  document formats, units, ranges, whitespace, ordering, and other constraints that the code relies on. Prefer expressing invalid combinations through types.
- When an enum variant or field merely maps to a serialized form, state that mapping briefly, while retaining range, whitespace, and other contracts. Include
  tuple fields. When moving details into field comments, remove the corresponding duplication from the containing type's doc comment.
- Regular comments explain the operation's purpose and preserved invariants. When rejecting an apparently simpler approach, describe concrete circumstances in
  which it would fail. If the simplification is actually feasible, record it as a TODO.
- CSS comments explain the rules' purpose, preserved invariants, ordering rationale, custom at-rule and slot usage, placement constraints, and considerations
  before changing them. Separate the opening file overview from comments for individual blocks and slots.
- Avoid restating obvious code, duplicating explanations, or claiming contracts the implementation does not guarantee.
