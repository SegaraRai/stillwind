# Stillwind CLI

The CLI discovers `stillwind.config.ts` above the working directory. `--config path` selects an
explicit configuration. Optional globs filter each project's configured files relative to its
root; globs cannot expand that scope. Imported dependencies can be analyzed without being rewritten.

```sh
stillwind fmt
stillwind fmt --check
stillwind lint
stillwind lint --fix
stillwind lint --format json
stillwind lsp --stdio
```

`fmt` sorts Stillwind inputs using the configured theme and `sort` options. It preserves source
layout and token spellings. It runs independently of lint severity and lint suppression, like the
editor's explicit Sort Utilities command. Use a general formatter afterward:

```sh
stillwind fmt && vp fmt
stillwind fmt --check && vp fmt --check
```

`lint` only reports findings. `lint --fix` applies the existing automatic fixes, including sorting
when the effective `sort-utilities` severity enables it. Lint suppression remains effective.
Fixes run against buffers until they converge, with a ten-pass limit. Findings and the exit code
describe the final buffers. All selected buffers are visible to dependency analysis in each pass.

Both commands compare original and final bytes before saving. Files with identical content are
never opened for writing, and their modification times remain unchanged. UTF-8 BOMs, line endings,
and trailing newlines remain intact outside edited ranges. Files must contain valid UTF-8.
The CLI verifies original bytes before saving and rejects sources changed during planning.
Planning completes for the batch before writes begin; I/O failures during saving can leave earlier
files saved. When projects overlap, every owning project must produce the same final text.

Exit codes:

- `0`: formatting succeeded, formatting check found no changes, or lint found no remaining errors.
- `1`: formatting check found changes, or lint found remaining errors. Warnings alone return `0`.
- `2`: configuration, analysis, planning, or I/O failed, or no targets matched.

`fmt --check` and `lint` never save sources. `fmt` prints changed file paths. `lint --format json`
returns structured findings; configuration logs are forwarded to stderr.
