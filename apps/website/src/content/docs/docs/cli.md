---
title: CLI
slug: docs/cli
description: Commands, options, file selection, exit codes, and output formats of the Stillwind CLI.
---

`@stillwind/cli` sorts utilities, reports lint findings, applies automatic fixes,
shows generated CSS, and starts the language server. It reads the same `stillwind.config.ts` as the
build integration and the editor.

```sh
vp i -D @stillwind/cli
```

To run the CLI in CI, see [Checks in the editor and CI](/docs/guides/lint-adoption).

## Commands

| Command             | Action                                               |
| ------------------- | ---------------------------------------------------- |
| `stillwind fmt`     | Sorts utilities                                      |
| `stillwind lint`    | Reports lint findings, and can apply automatic fixes |
| `stillwind inspect` | Shows the CSS that source code generates             |
| `stillwind lsp`     | Starts the language server                           |

`stillwind --help` lists the commands, and `stillwind <command> --help` lists the
options of a command. `stillwind --version` prints the package version. The short
forms of these options are `-h` and `-v`.

## stillwind fmt

```sh
vp exec stillwind fmt
vp exec stillwind fmt --check
```

`fmt` sorts utilities with the configured theme and
[`sort` options](/docs/configuration#sorting), and saves the changed files. It keeps
the source layout and the spelling of each token. Lint severity and suppression
comments do not affect `fmt`.

`fmt` does not change whitespace. To format whitespace too, see
[Formatters](/docs/formatters).

| Option    | Effect                                                  |
| --------- | ------------------------------------------------------- |
| `--check` | Reports the files that need sorting, and saves no files |

`fmt` also accepts the [shared options](#shared-options) and
[globs](#select-files).

Text output shows a status for each listed file and a summary.

## stillwind lint

```sh
vp exec stillwind lint
vp exec stillwind lint --fix
vp exec stillwind lint --max-warnings 0
```

`lint` reports findings and saves no files. Text output shows a `help:` message when
a finding has a suggested edit. Not every suggested edit is an automatic fix.

| Option               | Effect                                                                |
| -------------------- | --------------------------------------------------------------------- |
| `--fix`              | Applies the enabled automatic fixes, and saves the changed files      |
| `--quiet`            | Hides warnings, and keeps their count in the summary                  |
| `--max-warnings <n>` | Fails when the number of warnings exceeds `n`, a non-negative integer |

`lint` also accepts the [shared options](#shared-options) and
[globs](#select-files).

`--fix` respects suppression comments. It sorts utilities only when
[prefer-sorted-utilities](/docs/lints/prefer-sorted-utilities) is enabled. It
repeats until the files stop changing, with at most ten changing passes per file.
Findings and the exit code describe the files after the last pass.

`--max-warnings` counts distinct warnings in each project, including warnings that
`--quiet` hides. When hidden warnings exceed the limit, the CLI still prints the
reason for the failure.

## stillwind inspect

```sh
vp exec stillwind inspect src/Button.tsx
vp exec stillwind inspect src/Button.tsx:12
vp exec stillwind inspect src/Button.tsx:12:8
vp exec stillwind inspect 'src/**/*.tsx' src/App.tsx:30 --format json
```

`inspect` shows the CSS that `style()` calls, keyframes, and view transitions
generate. The CSS writes the styled element as `&`, and it includes the styles that
the call composes. `inspect` saves no files.

Each target is a [path or glob](#select-files), and you can pass several targets.
Without a target, `inspect` covers every file that `files` selects.

| Suffix         | Selected CSS                                                  |
| -------------- | ------------------------------------------------------------- |
| None           | Every generated occurrence in the file                        |
| `:line`        | The occurrences that overlap the line                         |
| `:line:column` | The utility at the position, or else the occurrence around it |

Lines and columns count from one, and columns count UTF-16 code units. A suffix on a
glob applies to each matched file.

`inspect` does not accept stylesheets, and a glob skips them. A target that matches no
file fails the command.

The output lists the findings in the range of each occurrence. An occurrence with an
error is marked as failed, and `inspect` returns exit code `1`. A warning does not fail
the occurrence or change the exit code. With `unknown-utility` set to `warn`, an
occurrence stays valid even though its CSS omits the unknown utility, so check the
findings as well. A file with incomplete analysis has no occurrences in the output, and
`inspect` reports it as a failure.

The output shows the CSS of each occurrence and not the result of the cascade. Runtime
values and rules from other occurrences can still change the applied styles.

`inspect` also accepts the [shared options](#shared-options). The `files` format is
not available.

## stillwind lsp

```sh
vp exec stillwind lsp --stdio
```

`lsp` starts the [language server](/docs/editors#other-editors) over stdin and
stdout. `--stdio` is required, and stdout carries only protocol messages. `lsp`
accepts `--config`, but no globs.

## Shared options

`fmt`, `lint`, and `inspect` accept these options:

| Option                  | Effect                                              |
| ----------------------- | --------------------------------------------------- |
| `-c`, `--config <path>` | Uses this [configuration file](#select-files)       |
| `--format <format>`     | Selects the [output format](#output-formats)        |
| `--color <mode>`        | Controls [colors](#colors-and-links)                |
| `--hyperlinks <mode>`   | Controls [links to source files](#colors-and-links) |

An unknown option, a missing value, or an unsupported value returns exit code `2`.

## Select files

The CLI searches upward from the working directory for `stillwind.config.*`. In each
directory, it tries the extensions `ts`, `mts`, `cts`, `js`, `mjs`, `cjs`, and `json`
in that order. `--config` selects a file directly.

The CLI checks every file that `files` selects in each project, including files that
your application never imports. It can read other imported files for analysis, but
it does not change them.

Globs narrow this selection to some of these files. A glob is relative to each project
root, and it cannot add files outside `files`. You can pass several globs:

```sh
vp exec stillwind lint 'src/**/*.tsx' 'src/**/*.css'
```

A leading `./` is accepted. An absolute path selects that file when `files` includes
it. When nothing matches, the CLI returns exit code `2`.

## Saving files

`fmt` and `lint --fix` work out the changes for all selected files before they save
the first file. The CLI then saves only the files whose content changed, so
modification times of other files stay the same.

Outside edited ranges, UTF-8 BOMs, line endings, and trailing newlines stay intact.
Files must contain valid UTF-8.

When something fails:

- A file that cannot be read, analyzed, or rewritten stays unchanged. The CLI
  continues with the other files, even after a saving failure.
- A file that changed while the CLI worked is rejected and stays as it is.
- A configuration or CSS failure affects every file that needs that configuration
  or CSS.
- When projects overlap, every project that owns a file must produce the same final
  text.

The CLI reports all failures and all saved files, and it returns exit code `2`.

## Exit codes

| Code | Meaning                                                                                                                                                 |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `0`  | The command succeeded. `lint` may still report warnings when you set no `--max-warnings` limit                                                          |
| `1`  | `fmt --check` found files to sort, `lint` found errors, the warnings exceeded `--max-warnings`, or `inspect` found a failed occurrence or no occurrence |
| `2`  | The options were invalid, no files matched, or configuration, analysis, or file access failed                                                           |

Code `2` takes priority over code `1`.

## Output formats

| Format   | Output                                                                                |
| -------- | ------------------------------------------------------------------------------------- |
| `text`   | Messages for reading in a terminal; the default outside GitHub Actions                |
| `github` | Annotations that link findings to files; the default when `GITHUB_ACTIONS=true`       |
| `json`   | [One JSON object](#json-output) for scripts and AI agents                             |
| `files`  | One path per line for each file that `fmt` changes or needs to change; only for `fmt` |

Results use stdout, and execution failures in `text` and `files` formats use stderr.
Configuration logs use stderr in every format. Only `text` output contains colors
and links.

### Positions and paths

`text` and `github` output count lines and columns from one. Columns count UTF-16
code units.

`text` paths are relative to the working directory. `github` paths are relative to
`GITHUB_WORKSPACE`, or to the working directory when that variable is not set.
A path on another filesystem volume stays absolute.

### Source excerpts

A finding at a source range shows a source excerpt, related locations, notes, and
suggestions. Excerpts show tabs as four spaces, and they align wide characters and
emoji. A long range shows its first four lines.

A finding about a whole document has no position or excerpt. The CLI also omits the
excerpt when it cannot read the source, or when the source changed after the
analysis.

## Colors and links

`--color` and `--hyperlinks` accept `auto`, `always`, and `never`. `always` and
`never` ignore the environment.

With `--color auto`, the CLI uses colors in terminals and supported CI logs. It
respects `NO_COLOR`, `FORCE_COLOR`, and `TERM=dumb`.

`--hyperlinks` controls OSC 8 links, which open local source files from the
terminal. With `auto`, the CLI uses them in recognized terminals, including VS Code
and Windows Terminal, and not in CI or redirected output. Terminals without OSC 8
support can still detect the printed `path:line:column` locations.

## JSON output

`--format json` prints one object with the same fields in every case, including
failures:

```json
{
  "command": "lint",
  "config": "/project/stillwind.config.ts",
  "diagnostics": [],
  "failures": [],
  "changed": [],
  "previews": [],
  "summary": {
    "files": 1,
    "errors": 0,
    "warnings": 0,
    "failures": 0,
    "changed": 0,
    "previews": 0,
    "invalidPreviews": 0,
    "suggestions": 0,
    "warningLimitExceeded": false
  },
  "exitCode": 0
}
```

| Field         | Value                                                                                         |
| ------------- | --------------------------------------------------------------------------------------------- |
| `command`     | `"fmt"`, `"lint"`, or `"inspect"`, or `null` when the CLI failed before it selected a command |
| `config`      | The configuration path, or `null` when the CLI failed before the analysis                     |
| `diagnostics` | The findings; `--quiet` removes warnings from this list                                       |
| `failures`    | The execution failures                                                                        |
| `changed`     | The files that the command changed; with `--check`, the files that it would change            |
| `previews`    | The generated CSS that `inspect` selected; empty for other commands                           |
| `summary`     | Counts for the fields above; `warnings` includes warnings that `--quiet` hides                |
| `exitCode`    | The [exit code](#exit-codes)                                                                  |

Each diagnostic contains `project`, a zero-based project index, and `report`. The
report contains the message, its locations, notes, and suggested edits.

Each failure contains `message` and `details`, and it can contain `project` and
`file`. `details` lists the dependency problems that caused the failure.

Each preview contains `project`, `location`, `source`, `css`, `valid`, and `diagnostics`.
`location` contains the range of the occurrence, and `source` contains the text of that
range. `diagnostics` lists the `severity`, `code`, and `message` of each finding in the
range. `valid` is `false` when the list contains an error or when a style that the
occurrence requires failed. `css` can be empty for an occurrence with findings, and it
is empty when `documentation` describes a marker class. `runtimeNames` lists the runtime
expressions that supply view-transition names, and it is absent when there are none.

File paths are absolute. Locations in reports and previews use URIs and zero-based UTF-16
positions, and their ends are exclusive.
