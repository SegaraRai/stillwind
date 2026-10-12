import { stripVTControlCharacters, styleText } from "node:util";

import { defineCommand, renderUsage, runCommand, type ArgsDef, type CommandDef } from "citty";

import packageJson from "../package.json" with { type: "json" };
import {
  builtinArgs,
  configArgs,
  outputArgs,
  parseInvocation,
  readOutputOptions,
  strictArguments,
  targetArgs,
  warningArgs,
  warningLimit,
} from "./arguments.ts";
import { displayEnabled, writeResult, type OutputOptions } from "./output.ts";
import { cliResult, failureResult, type CliCommand } from "./result.ts";

const fmtArgs = {
  ...targetArgs,
  check: {
    type: "boolean",
    description: "Check formatting without saving files",
  },
} as const;

const lintArgs = {
  ...targetArgs,
  format: {
    ...outputArgs.format,
    options: ["text", "json", "github"] as ("text" | "json" | "github")[],
  },
  ...warningArgs,
  fix: {
    type: "boolean",
    description: "Apply converged automatic fixes",
  },
} as const;

const lspArgs = {
  ...configArgs,
  stdio: {
    type: "boolean",
    required: true,
    description: "Use the Language Server Protocol over stdin/stdout",
  },
} as const;

const subCommands = {
  fmt: defineCommand({
    meta: {
      name: "fmt",
      description: "Sort Stillwind utilities",
    },
    args: fmtArgs,
    plugins: [strictArguments],
    async run({ args, data }) {
      const options = data as OutputOptions;
      options.check = args.check;
      const { format } = await import("@stillwind/analysis/node");
      const analysis = await format({
        config: args.config,
        cwd: options.cwd,
        patterns: args._,
        check: args.check,
      });
      const result = cliResult("fmt", analysis, { check: args.check });
      await writeResult(result, options);
      process.exitCode = result.exitCode;
    },
  }),
  lint: defineCommand({
    meta: {
      name: "lint",
      description: "Check Stillwind styles",
    },
    args: lintArgs,
    plugins: [strictArguments],
    async run({ args, data }) {
      const options = data as OutputOptions;
      const maxWarnings = warningLimit(args["max-warnings"]);
      options.quiet = args.quiet;
      options.maxWarnings = maxWarnings;
      const { lint } = await import("@stillwind/analysis/node");
      const analysis = await lint({
        config: args.config,
        cwd: options.cwd,
        patterns: args._,
        fix: args.fix,
      });
      const result = cliResult("lint", analysis, { quiet: args.quiet, maxWarnings });
      await writeResult(result, options);
      process.exitCode = result.exitCode;
    },
  }),
  lsp: defineCommand({
    meta: {
      name: "lsp",
      description: "Start the language server",
    },
    args: lspArgs,
    plugins: [strictArguments],
    async run({ args }) {
      if (!args.stdio || args._.length) {
        throw new Error("lsp requires --stdio and accepts no globs");
      }
      const { startLanguageServer } = await import("@stillwind/language-server/node");
      startLanguageServer(args.config);
    },
  }),
};

export const command = defineCommand({
  meta: {
    name: "stillwind",
    version: packageJson.version,
    description: "Sort and check Stillwind styles",
  },
  args: {
    ...configArgs,
    ...outputArgs,
    ...builtinArgs,
  },
  subCommands,
});

export async function runCli(rawArgs = process.argv.slice(2)): Promise<void> {
  const options: OutputOptions = {
    format: outputArgs.format.default,
    color: "auto",
    hyperlinks: "auto",
    cwd: process.cwd(),
  };
  let name: CliCommand = null;

  try {
    const invocation = parseInvocation(rawArgs, {
      ...fmtArgs,
      ...lintArgs,
      ...lspArgs,
    });
    name = invocation.name === "fmt" || invocation.name === "lint" ? invocation.name : null;
    if (invocation.name === "lsp") {
      options.format = "text";
    } else {
      readOutputOptions(rawArgs, options);
    }
    if (invocation.name && !Object.hasOwn(subCommands, invocation.name)) {
      throw new Error(`Unknown command: ${invocation.name}`);
    }

    if (!rawArgs.length || invocation.help) {
      const sub = subCommands[invocation.name as keyof typeof subCommands];
      const subject = sub ?? command;
      const usage = await renderUsage<ArgsDef>(
        {
          meta: subject.meta,
          args: subject.args,
          subCommands: subject.subCommands,
        },
        sub
          ? {
              meta: command.meta,
            }
          : undefined,
      );
      write(displayUsage(usage, options));
    } else if (rawArgs.length === 1 && invocation.version) {
      write(packageJson.version);
    } else {
      if (invocation.version) {
        throw new Error("--version must be used without a command");
      }
      if (!invocation.name) {
        throw new Error("A command is required: fmt, lint, or lsp");
      }
      await runCommand(subCommands[invocation.name as keyof typeof subCommands] as CommandDef, {
        rawArgs: invocation.rawArgs,
        data: options,
      });
    }
  } catch (error) {
    const result = failureResult(name, error, options);
    await writeResult(
      result,
      options.format === "files" ? { ...options, format: "text" } : options,
    );
    process.exitCode = 2;
  }
}

function write(output: string): void {
  if (output) {
    process.stdout.write(output + "\n");
  }
}

function displayUsage(usage: string, options: OutputOptions): string {
  const plain = stripVTControlCharacters(usage);
  return displayEnabled(options.color, "color", !!process.stdout.isTTY, process.env)
    ? styleText("cyan", plain, {
        validateStream: false,
      })
    : plain;
}
