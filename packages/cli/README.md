# Stillwind CLI

The CLI discovers `stillwind.config.*` above the working directory. Supported extensions are
`ts`, `mts`, `cts`, `js`, `mjs`, `cjs`, and `json`, in that order within each directory. `--config path` selects an
explicit configuration. Optional globs filter each project's configured files relative to its
root; globs cannot expand that scope. A leading `./` is accepted, and absolute file paths select
that file within its configured scope. Imported dependencies can be analyzed without being rewritten.

```sh
stillwind fmt
stillwind fmt --check
stillwind fmt --check --format files
stillwind lint
stillwind lint --fix
stillwind lint --format json
stillwind fmt --check --format json
stillwind lint --format github
stillwind lint --quiet --max-warnings 0
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
- `1`: formatting check found changes, lint found remaining errors, or the warning count exceeded
  `--max-warnings`. Warnings alone return `0` when no limit is supplied.
- `2`: command-line options are invalid, configuration, analysis, planning, or I/O failed,
  or no targets matched.

`stillwind --help` lists commands, and `stillwind <command> --help` lists that command's options.
`--version` prints the package version. `lsp` requires `--stdio` and keeps stdout for protocol messages.

`fmt` and `lint` support `--format text|json|github`. The default is `github` when
`GITHUB_ACTIONS=true` and `text` otherwise. Explicit formats override this choice.
Unknown options, missing option values, and unsupported format or display values return `2`.
Text diagnostics include paths and one-based line and column positions. GitHub output uses
workflow annotations that link findings and formatting failures to files in CI.
Text paths are relative to the invocation's working directory. GitHub paths are relative to
`GITHUB_WORKSPACE`, falling back to the working directory. Paths across filesystem volumes remain
absolute. OSC 8 links use absolute file URLs.

`--quiet` hides lint warnings while retaining their summary counts. `--max-warnings N` accepts a
non-negative integer and counts distinct warnings per project before applying `--quiet`.
Exceeding the limit still prints a failure reason when warnings are hidden.

`fmt` prints a status for each changed file and a summary. `fmt --check` reports files that need
formatting and succeeds with a summary when all selected files are formatted. `fmt --format files`
prints only changed paths, one per line; execution failures use stderr. `lint` does not accept this
format. Suggested edits appear as `help:` messages; their presence does not mean that every
suggestion is an automatic fix.

`--color auto|always|never` controls colors. Auto mode enables colors for terminals and supported
CI logs, respects `NO_COLOR`, `FORCE_COLOR=0`, and `TERM=dumb`, and honors other `FORCE_COLOR`
values. An explicit mode overrides environment detection. `--hyperlinks auto|always|never`
controls OSC 8 links to local source files. Auto mode enables links in recognized terminals,
including VS Code and Windows Terminal, and disables them in CI and redirected output.
Terminals that do not handle OSC 8 can still detect the printed `path:line:column` locations.

JSON, GitHub, and files output never contain display escapes. CLI JSON uses the same envelope
for successful commands, findings, empty selection, and execution failures:

```json
{
  "command": "lint",
  "config": "/project/stillwind.config.ts",
  "diagnostics": [],
  "failures": [],
  "changed": [],
  "summary": {
    "files": 1,
    "errors": 0,
    "warnings": 0,
    "failures": 0,
    "changed": 0,
    "suggestions": 0,
    "warningLimitExceeded": false
  },
  "exitCode": 0
}
```

Each diagnostic contains a zero-based `project` and the existing structured `report`, including
related locations, notes, and revision-pinned actions. Failures contain a message, optional project
and absolute file path, and relevant dependency details. Configuration is `null` when execution
fails before analysis, and command is `null` when no analysis command was selected. Analysis graphs
and generated code are omitted. JSON file paths remain absolute, and report locations retain their
original URIs and zero-based UTF-16 positions with exclusive ends. The analysis API still returns
its complete `results`; this CLI projection does not change that API.

JSON and GitHub results use stdout. Text execution failures use stderr. Configuration logs remain
on stderr in every mode.

`fmt --check` and `lint` never save sources. `fmt` prints changed file paths. `lint --format json`
returns structured findings; configuration logs are forwarded to stderr.
