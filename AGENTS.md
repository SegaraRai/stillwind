# Guideline For Agents

## Responsibility and Colocation

- Actively colocate new code by related responsibility, not by item kind. Define errors and types alongside the code that constructs them;
  do not create kind-based files such as `errors.rs` or `types.rs`. Keep each struct immediately followed by its impl blocks.
- Within each source file, put error definitions first, then items of greatest interest to users, each followed by its supporting items;
  repeat this main-item/supporting-items grouping instead of collecting all helpers or types separately.

## Tests and Verification

- Test current behavior and caller contracts. Do not add tests that only require a previous API, name, file, or implementation to remain absent.
- Put JS tests beside the module they exercise and use `.test.ts`. Use `.mts`, `.js`, or `.mjs` only when the test needs their module or execution semantics. A public API alone does not justify a separate test directory.
- Use a dedicated `tests/` directory for integration tests that exercise several modules together. Keep Rust unit tests beside their implementation and Rust public-API integration tests in the owning crate's `tests/` directory.
- Use project fixtures for representative generated JS, CSS, diagnostics, and source maps. Use integration tests for build-tool contracts and browser tests for behavior that requires a browser.
- Use `vite-plus/test` for JS tests and declare `vp test --run` tasks in `vite.config.ts`. Include runtime tests in the package's TypeScript check. Keep compile-only type contracts under `typecheck`.
- Use Vitest `expect` for assertions, including rejected promises and thrown errors. Use `expect.fail` for unexpected branches or mock calls. A mock may throw when the thrown error is an input to the behavior being tested.
- Share test cases between native and WASM bindings, but run the bindings in separate processes. Give tests that write files or start servers independent temporary directories and clean up after them.
- Run `vp run check`, `vp run test`, and `vp run fixtures:check` for full ordinary verification, or `vp run verify` for all three. Use `verify:compat`, `verify:browser`, and `verify:wasm` for the corresponding environments.
- Fixture checks compare without updating tracked files. Run refresh or generation tasks explicitly and review their output. Do not cache checks that read untracked inputs from outside their package.
- Add a test when it protects a distinct observable contract. Remove duplicate assertions that detect the same failure through the same interface.

## Writing Guidelines

These rules apply to all prose, including comments, documentation, explanations, and status reports.

- Write for a reader who knows the subject but may not be a native English speaker. Keep established technical terms and code identifiers.
  Simplify the grammar around them. Explain a term where it is introduced when readers need that definition; do not repeat the definition at every use.
- State each relationship with an explicit verb. Write "`span` identifies the source use, and `guard` contains its conditions" instead of
  "`span` identifies the source use and `guard` its conditions". The conventional opening "Returns ..." or "Records ..." is fine for a function
  comment when the function is clearly the subject. Name the actor when it changes or could be ambiguous.
- Review phrases with three or more nouns and invented hyphenated labels. Express the relationship with a verb or preposition:
  "property records for each use" instead of "source-use property records". Keep established terms such as "canonical marker token" when they
  identify a specific concept. Do not replace precise terms with vague synonyms merely to shorten them.
- Name the concrete object or action when an abstraction makes the reader reconstruct it. Write "the compiler cannot determine which element
  the selector affects" instead of "the target is uncertain". Prefer ordinary verbs such as "adds", "keeps", and "skips" where they convey
  the exact behavior. Keep implementation terms such as "lowering" when that specific operation matters.
- Give each sentence one main relationship. A closely related condition or consequence can stay in the same sentence. Separate independent
  parameters, exceptions, and failure behavior, and use paragraph breaks between those topics. Do not turn every clause into a separate sentence
  or list item when that would only repeat its subject.
- Preserve the contract when simplifying: who acts, what changes, when it happens, and which exceptions apply. For example, if provider rules
  are included, keep "Ordinary utilities with selectors are skipped"; shortening it to "Utilities with selectors are skipped" changes the meaning.
  Preserve distinctions such as "may" versus "must", missing values versus empty values, and failure versus successful omission.
- Reduce grammatical compression without adding information. Do not explain an established term repeatedly, paraphrase the same guarantee twice,
  or add filler such as "in order to" or "it is important to note". Use an example only when it resolves an ambiguity more directly than prose.
- Before finishing, check that readers can identify each sentence's actor, action, object, and conditions without filling in omitted words.
  Then remove duplicated facts and verify that the simpler wording still describes the implementation accurately.

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
- Function doc comments explain purpose and the contract between caller and function: what the caller must guarantee about parameters and prior state,
  and what the function guarantees about its return value and resulting state. Describe what each parameter accepts when present, what the function returns
  when it has a return value, and what it changes when it has side effects, including changes to internal state or passed-in values. Document error
  conditions when returning `Result` and panic conditions when applicable.
- Struct and enum doc comments explain the represented concept and its purpose. Describe usage domains or potential misuse only when useful. Field comments
  document formats, units, ranges, whitespace, ordering, and other constraints that the code relies on. Prefer expressing invalid combinations through types.
- When an enum variant or field merely maps to a serialized form, state that mapping briefly, while retaining range, whitespace, and other contracts. Include
  tuple fields. When moving details into field comments, remove the corresponding duplication from the containing type's doc comment.
- Regular comments explain the operation's purpose and preserved invariants. When rejecting an apparently simpler approach, describe concrete circumstances in
  which it would fail.
- CSS comments explain the rules' purpose, preserved invariants, ordering rationale, custom at-rule and slot usage, placement constraints, and considerations
  before changing them. Separate the opening file overview from comments for individual blocks and slots.
- Avoid restating obvious code, duplicating explanations, or claiming contracts the implementation does not guarantee.
