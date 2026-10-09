import { stripVTControlCharacters, styleText } from "node:util";

import { defineCommand, renderUsage, runCommand, type ArgsDef } from "citty";

import packageJson from "../package.json" with { type: "json" };
import {
  builtinArgs,
  configArgs,
  outputArgs,
  parseInvocation,
  readOutputOptions,
  strictArguments,
  targetArgs,
} from "./arguments.ts";
import {
  displayEnabled,
  renderFormat,
  renderLint,
  writeFailure,
  type OutputOptions,
} from "./output.ts";

const fmtArgs = {
  ...targetArgs,
  check: {
    type: "boolean",
    description: "Check formatting without saving files",
  },
} as const;

const lintArgs = {
  ...targetArgs,
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
    async run({ args }) {
      const { format } = await import("@stillwind/analysis/node");
      const result = await format({
        config: args.config,
        patterns: args._,
        check: args.check,
      });
      write(renderFormat(result, args, !!args.check));
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
    async run({ args }) {
      const { lint } = await import("@stillwind/analysis/node");
      const result = await lint({
        config: args.config,
        patterns: args._,
        fix: args.fix,
      });
      write(renderLint(result, args));
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
  };

  try {
    const invocation = parseInvocation(rawArgs, {
      ...fmtArgs,
      ...lintArgs,
      ...lspArgs,
    });
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
      const normalized = invocation.name
        ? [invocation.name, ...invocation.rawArgs]
        : invocation.rawArgs;
      await runCommand(command, {
        rawArgs: normalized,
      });
    }
  } catch (error) {
    writeFailure(error, options);
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
