import { parseArgs as validateArgs, stripVTControlCharacters, styleText } from "node:util";

import { defineCommand, parseArgs, renderUsage, runCommand, type ArgsDef } from "citty";

import packageJson from "../package.json" with { type: "json" };
import {
  displayEnabled,
  renderFormat,
  renderLint,
  writeFailure,
  type OutputOptions,
} from "./output.ts";

const { version } = packageJson;

const configArgs = {
  config: {
    type: "string",
    alias: "c",
    description: "Use this configuration file",
    valueHint: "path",
  },
} as const;
const outputArgs = {
  format: {
    type: "enum",
    options: ["text", "json", "github"] as string[],
    description: "Output format (defaults to github in GitHub Actions, otherwise text)",
  },
  color: {
    type: "enum",
    options: ["auto", "always", "never"] as string[],
    default: "auto",
    description: "Control terminal colors",
  },
  hyperlinks: {
    type: "enum",
    options: ["auto", "always", "never"] as string[],
    default: "auto",
    description: "Control links to source locations",
  },
} as const;
const targetArgs = {
  ...configArgs,
  ...outputArgs,
  globs: {
    type: "positional",
    required: false,
    description: "Filter configured targets with one or more globs",
  },
} as const;

function argumentOptions(definitions: ArgsDef) {
  return Object.fromEntries(
    Object.entries(definitions)
      .filter(([, def]) => def.type !== "positional")
      .map(([name, def]) => [
        name,
        {
          type: def.type === "boolean" ? ("boolean" as const) : ("string" as const),
          ...("alias" in def && typeof def.alias === "string" ? { short: def.alias } : {}),
        },
      ]),
  );
}
function validate(rawArgs: string[], definitions: ArgsDef): void {
  validateArgs({ args: rawArgs, allowPositionals: true, options: argumentOptions(definitions) });
}
function outputOptions(rawArgs: string[]): OutputOptions {
  const args = parseArgs(rawArgs, {
    format: { type: "string" },
    color: { type: "string" },
    hyperlinks: { type: "string" },
  });
  return {
    format:
      args.format === "json"
        ? "json"
        : args.format === "github"
          ? "github"
          : args.format === "text"
            ? "text"
            : process.env.GITHUB_ACTIONS === "true"
              ? "github"
              : "text",
    color: args.color === "always" ? "always" : args.color === "never" ? "never" : "auto",
    hyperlinks:
      args.hyperlinks === "always" ? "always" : args.hyperlinks === "never" ? "never" : "auto",
  };
}
function write(output: string): void {
  if (output) {
    process.stdout.write(output + "\n");
  }
}

const fmtArgs = {
  ...targetArgs,
  check: { type: "boolean", description: "Check formatting without saving files" },
} as const;
const lintArgs = {
  ...targetArgs,
  fix: { type: "boolean", description: "Apply converged automatic fixes" },
} as const;
const lspArgs = {
  ...configArgs,
  stdio: {
    type: "boolean",
    required: true,
    description: "Use the Language Server Protocol over stdin/stdout",
  },
} as const;
export const command = defineCommand({
  meta: { name: "stillwind", version, description: "Sort and check Stillwind styles" },
  subCommands: {
    fmt: defineCommand({
      meta: { name: "fmt", description: "Sort Stillwind utilities" },
      args: fmtArgs,
      setup: ({ rawArgs }) => validate(rawArgs, fmtArgs),
      async run({ args, rawArgs }) {
        const { format } = await import("@stillwind/analysis/node");
        const result = await format({ config: args.config, patterns: args._, check: args.check });
        write(renderFormat(result, outputOptions(rawArgs), !!args.check));
        process.exitCode = result.exitCode;
      },
    }),
    lint: defineCommand({
      meta: { name: "lint", description: "Check Stillwind styles" },
      args: lintArgs,
      setup: ({ rawArgs }) => validate(rawArgs, lintArgs),
      async run({ args, rawArgs }) {
        const { lint } = await import("@stillwind/analysis/node");
        const result = await lint({ config: args.config, patterns: args._, fix: args.fix });
        write(renderLint(result, outputOptions(rawArgs)));
        process.exitCode = result.exitCode;
      },
    }),
    lsp: defineCommand({
      meta: { name: "lsp", description: "Start the language server" },
      args: lspArgs,
      setup({ rawArgs, args }) {
        validate(rawArgs, lspArgs);
        if (!args.stdio || args._.length) {
          throw new Error("lsp requires --stdio and accepts no globs");
        }
      },
      async run({ args }) {
        const { startLanguageServer } = await import("@stillwind/language-server/node");
        startLanguageServer(args.config);
      },
    }),
  },
});

export async function runCli(rawArgs = process.argv.slice(2)): Promise<void> {
  let options =
    rawArgs[0] === "lsp"
      ? { ...outputOptions(rawArgs), format: "text" as const }
      : outputOptions(rawArgs);
  try {
    const parsed = validateArgs({
      args: rawArgs,
      options: argumentOptions({
        ...fmtArgs,
        ...lintArgs,
        ...lspArgs,
        help: { type: "boolean", alias: "h" },
        version: { type: "boolean", alias: "v" },
      }),
      strict: false,
      allowPositionals: true,
      tokens: true,
    });
    const index = parsed.tokens.find((token) => token.kind === "positional")?.index;
    if (index !== undefined && index > 0) {
      rawArgs = [rawArgs[index], ...rawArgs.slice(0, index), ...rawArgs.slice(index + 1)];
    }
    if (rawArgs[0] === "lsp") {
      options = { ...options, format: "text" };
    }
    const separator = rawArgs.indexOf("--");
    const flags = separator < 0 ? rawArgs : rawArgs.slice(0, separator);
    if (!rawArgs.length || flags.includes("--help") || flags.includes("-h")) {
      const subcommands = command.subCommands as Record<string, typeof command>;
      const sub = subcommands[rawArgs[0]];
      const usage = await renderUsage(sub ?? command, sub ? command : undefined);
      write(displayUsage(usage, options));
    } else if (rawArgs.length === 1 && ["--version", "-v"].includes(rawArgs[0])) {
      write(version);
    } else {
      await runCommand(command, { rawArgs });
    }
  } catch (error) {
    writeFailure(error, options);
    process.exitCode = 2;
  }
}
function displayUsage(usage: string, options: OutputOptions): string {
  const plain = stripVTControlCharacters(usage);
  return displayEnabled(options.color, "color", !!process.stdout.isTTY, process.env)
    ? styleText("cyan", plain, { validateStream: false })
    : plain;
}
