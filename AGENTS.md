# Guideline For Agents

## Code Organization

- `index.ts` imports the modules it exposes. Files in its directory or descendants import the responsible module directly, including type-only imports.
- A Rust module with child files uses `foo/mod.rs` and `foo/*.rs`. Do not combine `foo.rs` with a sibling `foo/` directory.
- Group code by responsibility, not by item kind. Define errors and types beside the code that constructs or returns them, and do not create files such as `errors.rs` or `types.rs`. Put each struct's impl blocks directly after it.
- Within a responsibility group, put errors first. Follow each main item with its supporting items.

## Package Names

- Reserve `stillwind` and `@stillwind/*` for packages that can be published.
- Use `@stillwind-internal/apps-*`, `@stillwind-internal/e2e-*`, and `@stillwind-internal/tools-*` for private packages under `apps/`, `e2e/`, and `tools/`, respectively.
- Name the repository root `@stillwind-internal/root` and the benchmark package `@stillwind-internal/benchmarks`.

## Change Intents

- Record a change intent for each new feature and each other user-visible package change. Run `vp exec pnpm change`, and commit the intent file with the change.
- Use `minor` for a new feature and `patch` for a user-visible fix. Name the packages that users install or import for the change. The fixed version group releases the other packages at the same version.
- Write the summary for package users, because it becomes the changelog entry. Describe what users can now do, not how the implementation changed.

## Tests

- Test current behavior and caller contracts. Do not add tests that only check that a previous API, name, file, or implementation is absent.
- Add a test only when it protects a distinct observable contract. Remove assertions that detect the same failure through the same interface.
- Put JS tests beside the module they exercise and use `.test.ts`. Use `.mts`, `.js`, or `.mjs` only when the test needs their module or execution semantics.
- Use a `tests/` directory for integration tests that exercise several modules together. Keep Rust unit tests beside their implementation and Rust public-API integration tests in the owning crate's `tests/` directory.
- Use project fixtures for representative generated JS, CSS, diagnostics, and source maps. Use integration tests for build-tool contracts and browser tests for behavior that requires a browser.
- Keep framework browser acceptance under `e2e/` and performance measurements under `benchmarks/`.
- Use `vite-plus/test` and Vitest `expect`, including for rejected promises and thrown errors. Use `expect.fail` for unexpected branches or mock calls.
- Include runtime tests in the package's TypeScript check. Keep compile-only type contracts under `typecheck`.
- Share test cases between native and WASM bindings, but run the bindings in separate processes.
- Give tests that write files or start servers independent temporary directories, and clean them up.

## Verification

- Run `vp run check` and `vp run test`, or `vp run verify` for both. `check` includes generated-data consistency, and `test` includes project fixture comparisons.
- Do not run several `vp` commands in parallel. `vp run` already runs the dependencies of a task in parallel where the dependency graph allows it, so run one aggregate task instead.
- Run `test:tailwind`, `test:browser`, `test:wasm`, and `test:e2e` for the corresponding environments. Run `bench:compiler`, `bench:host`, and `bench:source-map` only when measuring performance.
- Fixture checks do not update tracked files. Run `generate:*` tasks explicitly and review their output.

## Task Definitions

- Declare `vp test --run` tasks in `vite.config.ts`.
- Name tasks by kind: `test:*` asserts behavior, `check:*` checks static consistency, `verify` aggregates both, `generate:*` writes tracked output, and `refresh:*` fetches mutable external inputs.
- Declare prerequisites in `dependsOn`. Aggregate tasks have an empty command. Do not invoke `vp run` within another task's command.
- Cache finite deterministic tasks. Keep automatic input tracking, add result-affecting environment variables and sources that incremental tools may skip reading, and exclude disposable files from inputs and restored outputs.
- Disable caching for servers, browser acceptance, performance measurements, and checks of external inputs that cannot be tracked.

## Writing Guidelines

These rules apply to all prose, including comments, documentation, change intents, release notes, and status reports.

- Write repository documentation in English. Preserve non-English source examples and fixture data when the contract or test requires them.
- Documentation covers current contracts, usage, and required verification. Update or merge an existing document before adding one. Do not keep documents that record implementation steps, completed work, review transcripts, or one-off benchmark results.
- Keep unresolved design and validation work in `docs/TODO.md`. Remove resolved entries, and do not write separate plans or TODO comments.
- Write for readers who know the subject but may not be native English speakers. Keep established technical terms and identifiers, and use plain grammar around them.
- Make the actor, action, object, and conditions of each sentence clear. Use a verb or preposition instead of stacked nouns or invented hyphenated labels.
- Keep the contract when shortening: who acts, what changes, when it happens, and which exceptions apply. Preserve distinctions such as "may" versus "must", missing versus empty values, and failure versus successful omission.

## Comment Guidelines

- Describe the current code. Do not record session history, received instructions, changes made in the current task, or justifications such as "This satisfies X".
- Write a comment only when it gives information that the surrounding code and linked material do not, and when missing that information could cause misuse or misunderstanding. Avoid the "This is A, not B" pattern.
- Crate and module doc comments explain purpose, responsibilities, non-goals, and main entry points. In `lib.rs` and `mod.rs`, end the doc comment with `Modules:` followed by a bullet list that describes the role of each non-test child module in alphabetical order, such as ``- [`loading`] expands supplied CSS imports.``
- Write an intra-doc link for every item, module, and field that rustdoc can resolve, and link only the first mention in each doc comment. Link to an item that is not in scope with an explicit path, such as ``[`Name`](crate::module::Name)``, instead of a `#[cfg(doc)]` import. Do not write intra-doc links in the doc comments of `#[napi]` items, because napi-rs copies them into `packages/native/binding.d.ts`. Check links with `cargo doc --workspace --no-deps --all-features --document-private-items`.
- Function doc comments explain the contract: what the caller must guarantee about parameters and prior state, what the function returns, and what it changes. Document error conditions for `Result` and panic conditions.
- Type doc comments explain the represented concept and its purpose. Field comments document formats, units, ranges, whitespace, ordering, and other constraints that the code relies on. Prefer types over comments for invalid combinations.
- When an enum variant or field maps to a serialized form, state the mapping briefly and keep its other constraints.
- Regular comments explain purpose and preserved invariants. When code rejects a simpler approach, describe a concrete case in which that approach fails.
- CSS comments explain purpose, preserved invariants, ordering rationale, custom at-rule and slot usage, and placement constraints. Keep the file overview separate from comments for individual blocks and slots.
