# Stillwind CLI

Stillwind compiles Tailwind CSS v4 utilities into component props and static CSS.
It uses theme rules and compiler checks to keep styling code in order, even when
AI agents write much of it.

`@stillwind/cli` is Stillwind's command-line interface for sorting, linting,
automatic fixes, and CI checks. It uses the same project configuration as the
build integration and editor.

## Install

```sh
vp i -D @stillwind/cli
```

Create a [Stillwind project configuration](https://stillwind.dev/docs/configuration)
before running commands. The CLI searches upward from the working directory for
`stillwind.config.*`. Use `--config path` to select another file.

## Commands

```sh
stillwind fmt
stillwind lint
stillwind lint --fix
```

`fmt` sorts utility inputs and preserves source layout and token spellings. Run a
general formatter afterward, or use
[prettier-plugin-stillwind](https://github.com/SegaraRai/stillwind/blob/main/packages/prettier-plugin/README.md).
`lint` reports findings; `lint --fix` applies enabled automatic fixes. Only changed
files are saved.

Optional file patterns narrow the configured project scopes:

```sh
stillwind lint 'src/**/*.tsx'
```

## CI and machine-readable output

```sh
stillwind fmt --check
stillwind lint --max-warnings 0
stillwind lint --format json
```

`fmt --check` and `lint` do not save sources. JSON and GitHub annotations use stdout;
configuration logs use stderr. GitHub annotations are the default in GitHub Actions.

| Exit code | Meaning                                                                                                              |
| --------- | -------------------------------------------------------------------------------------------------------------------- |
| `0`       | Formatting succeeded, formatting check found no changes, or lint found no errors and stayed within any warning limit |
| `1`       | Formatting changes are required, lint found errors, or the warning limit was exceeded                                |
| `2`       | Invalid options, failed configuration/analysis/planning/I/O, or no matching targets                                  |

Warnings alone return `0` when no limit is set. `stillwind --help` lists commands;
`stillwind <command> --help` lists options. `stillwind lsp --stdio` starts the
language server for clients using the Language Server Protocol.

See the [CLI reference](https://github.com/SegaraRai/stillwind/blob/main/docs/cli.md)
for configuration discovery, saving guarantees, output formats, and the JSON schema.
