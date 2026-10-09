import { parseArgs as tokenizeArgs } from "node:util";

import { defineCittyPlugin, parseArgs, type ArgsDef } from "citty";

import type { DisplayMode, OutputFormat, OutputOptions } from "./output.ts";

export const configArgs = {
  config: {
    type: "string",
    alias: "c",
    description: "Use this configuration file",
    valueHint: "path",
  },
} as const;

export const outputArgs = {
  format: {
    type: "enum",
    options: ["text", "json", "github"] as OutputFormat[],
    default: process.env.GITHUB_ACTIONS === "true" ? "github" : "text",
    description: "Output format",
  },
  color: {
    type: "enum",
    options: ["auto", "always", "never"] as DisplayMode[],
    default: "auto",
    description: "Control terminal colors",
  },
  hyperlinks: {
    type: "enum",
    options: ["auto", "always", "never"] as DisplayMode[],
    default: "auto",
    description: "Control links to source locations",
  },
} as const;

export const targetArgs = {
  ...configArgs,
  ...outputArgs,
  globs: {
    type: "positional",
    required: false,
    description: "Filter configured targets with one or more globs",
  },
} as const;

export const builtinArgs = {
  help: {
    type: "boolean",
    alias: "h",
    description: "Show usage",
  },
  version: {
    type: "boolean",
    alias: "v",
    description: "Show version",
  },
} as const;

/**
 * Updates display options using citty's enum validation and defaults.
 * A valid format is retained if a later display option fails, so the error uses the requested format.
 */
export function readOutputOptions(rawArgs: string[], options: OutputOptions): void {
  options.format = parseArgs<{
    format: typeof outputArgs.format;
  }>(rawArgs, {
    format: outputArgs.format,
  }).format;
  const { color, hyperlinks } = parseArgs<typeof outputArgs>(rawArgs, outputArgs);
  options.color = color;
  options.hyperlinks = hyperlinks;
}

/** Rejects unknown named options and empty string options that citty otherwise accepts. */
export const strictArguments = defineCittyPlugin({
  name: "strict-arguments",
  async setup({ args, cmd }) {
    const definitions: ArgsDef =
      (await (typeof cmd.args === "function" ? cmd.args() : cmd.args)) ?? {};
    const names = new Set(["_", ...Object.keys(definitions)]);
    for (const definition of Object.values(definitions)) {
      if ("alias" in definition) {
        for (const alias of [definition.alias ?? []].flat()) {
          names.add(alias);
        }
      }
    }
    for (const name of Object.keys(args)) {
      if (!names.has(name)) {
        throw new Error(`Unknown option: --${name}`);
      }
    }
    for (const [name, definition] of Object.entries(definitions)) {
      if (
        definition.type === "string" &&
        args[name] !== undefined &&
        (typeof args[name] !== "string" || args[name] === "")
      ) {
        throw new Error(`Missing or invalid value for --${name}`);
      }
    }
  },
});

/**
 * Locates the command without mistaking an option's value for it.
 * Keeps options before the command and removes help/version flags handled by the entrypoint.
 */
export function parseInvocation(rawArgs: string[], definitions: ArgsDef) {
  const combined: ArgsDef = { ...definitions, ...builtinArgs };
  const options: Record<string, { type: "boolean" | "string"; short?: string }> = {};
  for (const [name, definition] of Object.entries(combined)) {
    if (definition.type === "positional") {
      continue;
    }
    const option = {
      type: definition.type === "boolean" ? ("boolean" as const) : ("string" as const),
    };
    options[name] = option;
    if ("alias" in definition && typeof definition.alias === "string") {
      options[name].short = definition.alias;
    }
  }
  const { tokens } = tokenizeArgs({
    args: rawArgs,
    options,
    strict: false,
    allowPositionals: true,
    allowNegative: true,
    tokens: true,
  });
  const first = tokens.find(
    (token) => token.kind === "positional" || token.kind === "option-terminator",
  );
  const commandIndex = first?.kind === "positional" ? first.index : undefined;
  const builtinTokens = tokens.filter(
    (token) => token.kind === "option" && ["help", "version"].includes(token.name),
  );
  const flags = parseArgs<typeof builtinArgs>(
    builtinTokens.map((token) => rawArgs[token.index]),
    builtinArgs,
  );
  const omitted = new Set(builtinTokens.map((token) => token.index));
  if (commandIndex !== undefined) {
    omitted.add(commandIndex);
  }

  return {
    name: commandIndex === undefined ? undefined : rawArgs[commandIndex],
    rawArgs: rawArgs.filter((_, index) => !omitted.has(index)),
    help: flags.help,
    version: flags.version,
  };
}
