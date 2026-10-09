# Stillwind CLI

The CLI discovers `stillwind.config.*` above the working directory. Supported extensions are
`ts`, `mts`, `cts`, `js`, `mjs`, `cjs`, and `json`, in that order within each directory. `--config path` selects an
explicit configuration. Optional globs filter each project's configured files relative to its
root; globs cannot expand that scope. Imported dependencies can be analyzed without being rewritten.

```sh
stillwind fmt
stillwind fmt --check
stillwind lint
stillwind lint --fix
stillwind lint --format json
stillwind fmt --check --format json
stillwind lint --format github
stillwind lint --color always --hyperlinks always
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
- `2`: command-line options are invalid, configuration, analysis, planning, or I/O failed,
  or no targets matched.

`stillwind --help` lists commands, and `stillwind <command> --help` lists that command's options.
`--version` prints the package version. `lsp` requires `--stdio` and keeps stdout for protocol messages.

`fmt` and `lint` support `--format text|json|github`. The default is `github` when
`GITHUB_ACTIONS=true` and `text` otherwise. Explicit formats override this choice.
Unknown options, missing option values, and unsupported format or display values return `2`.
Text diagnostics include paths and one-based line and column positions. GitHub output uses
workflow annotations that link findings and formatting failures to files in CI.

`--color auto|always|never` controls colors. Auto mode enables colors for terminals and supported
CI logs, respects `NO_COLOR`, `FORCE_COLOR=0`, and `TERM=dumb`, and honors other `FORCE_COLOR`
values. An explicit mode overrides environment detection. `--hyperlinks auto|always|never`
controls OSC 8 links to local source files. Auto mode enables links in recognized terminals,
including VS Code and Windows Terminal, and disables them in CI and redirected output.
Terminals that do not handle OSC 8 can still detect the printed `path:line:column` locations.

JSON and GitHub output never contain display escapes. JSON success results preserve the analysis
API's `config`, `results`, `changed` when available, and `exitCode` fields. Execution failures
produce `{ "exitCode": 2, "error": { "message": "..." } }` on stdout. Configuration logs remain
on stderr. Text execution errors are written to stderr using Consola. JSON source ranges use
zero-based UTF-16 line and column positions and exclusive ends; text and CI positions are one-based.

`fmt --check` and `lint` never save sources. `fmt` prints changed file paths. `lint --format json`
returns structured findings; configuration logs are forwarded to stderr.
