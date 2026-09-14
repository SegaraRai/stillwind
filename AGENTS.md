# Guideline For Agents

## Responsibility and Colocation

- Actively colocate new code by related responsibility, not by item kind. Define errors and types alongside the code that constructs them;
  do not create kind-based files such as `errors.rs` or `types.rs`. Keep each struct immediately followed by its impl blocks.
- If existing or newly added code falls outside its responsibility, add a TODO of at most 3 lines in the aggregating file (`mod.rs`, `lib.rs`,
  `index.ts`, etc.), naming the specific functions, structs, or other items and explaining which responsibility they violate and where they belong.
- Within each source file, put error definitions first, then items of greatest interest to users, each followed by its supporting items;
  repeat this main-item/supporting-items grouping instead of collecting all helpers or types separately.

## Tidy First

- Autonomous tidy means cleanup of surrounding code outside the user's requested work. Perform it where useful without an explicit cleanup request,
  while following any explicit user constraints.
  Preserve behavior and keep structural edits distinct from functional edits, tidying before behavior changes where useful.
- For autonomous tidy, implement an identified improvement when it needs at most 20 added or modified implementation lines; deletions are unlimited.
  Count the complete implementation improvement across files, without splitting it to evade the limit; exclude contract tests from this count.
- Before autonomous tidy, confirm unit tests cover the affected operation's required contracts. If coverage is missing, add it only if the tests fit within
  30 lines; otherwise skip the improvement. Run the relevant contract tests and ensure they pass.
- When either autonomous-tidy limit prevents an improvement, add a TODO at the affected operation in at most 3 lines, specifying the fix, its rationale,
  and a realistic, reproducible case that causes an actual problem. Do not invent hypothetical problems to justify cleanup.
- _These implementation and test limits apply only to autonomous tidy._ User-requested implementation, refactoring, TODO processing, and their
  required tests are exempt: never skip, defer to a TODO, or narrow requested work because of these limits.

## TODO Review

- Autonomously review a TODO only when, during implementation, you notice a contradiction in a TODO in surrounding code.
  Do not seek out or scan TODOs for review unless the user explicitly requests it; follow the requested scope when they do.
- When a TODO appears incorrect, gather objective evidence for both its claim and the rebuttal. Have an independent Sol or Luna subagent judge
  both claims with no conversation history (`fork_turns="none"`); provide the relevant code and evidence without steering its conclusion.
- Only if that review validates the rebuttal, replace the TODO with an explanatory comment of at most 3 lines that directly states the relevant fact
  and concrete evidence, following Comment Guidelines; otherwise retain the TODO.

## Comment Guidelines

- Describe the current codebase objectively for its readers. Do not record session history, received instructions, or the changes made in the current task.
- Write a factual statement such as "This is A" only when it is worth reading. It must satisfy both conditions: it provides new information absent
  from the surrounding context (including adjacent lines and linked material) that would otherwise require reading the implementation to discover;
  and ignorance of that fact could cause critical misuse or misunderstanding. Apply this threshold to all comments described below.
- Avoid formulations such as "This is A, not B" and "A; B": these are typical ways to slip history or excuses into comments. Merely echoing a user's
  request to "satisfy X" as "This satisfies X" is also an excuse, not useful documentation. State the relevant fact directly only if it meets both
  conditions above; otherwise omit the comment.
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
