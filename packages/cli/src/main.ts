#!/usr/bin/env node
import { parseArgs } from "node:util";

import { format, formatLint, lint } from "@stillwind/analysis/node";
import { startLanguageServer } from "@stillwind/language-server/node";

try {
  const { values, positionals } = parseArgs({
    allowPositionals: true,
    options: {
      config: {
        type: "string",
        short: "c",
      },
      format: {
        type: "string",
        default: "text",
      },
      stdio: {
        type: "boolean",
      },
      check: {
        type: "boolean",
      },
      fix: {
        type: "boolean",
      },
      help: {
        type: "boolean",
        short: "h",
      },
    },
  });
  const [command, ...patterns] = positionals;
  if (!values.help) {
    if (values.check && command !== "fmt") {
      throw new Error("--check requires the fmt command");
    }
    if (values.fix && command !== "lint") {
      throw new Error("--fix requires the lint command");
    }
  }
  if (values.help || !command) {
    process.stdout.write(
      "Usage: stillwind fmt [globs] [--config path] [--check]\n       stillwind lint [globs] [--config path] [--fix] [--format text|json]\n       stillwind lsp --stdio [--config path]\n",
    );
  } else if (command === "lint") {
    if (!["text", "json"].includes(values.format)) {
      throw new Error("--format must be text or json");
    }
    const result = await lint({ config: values.config, patterns, fix: values.fix });
    const output = values.format === "json" ? JSON.stringify(result) : formatLint(result);
    if (output) {
      process.stdout.write(output + "\n");
    }
    process.exitCode = result.exitCode;
  } else if (command === "fmt") {
    const result = await format({ config: values.config, patterns, check: values.check });
    if (result.changed.length) {
      process.stdout.write(result.changed.join("\n") + "\n");
    }
    process.exitCode = result.exitCode;
  } else if (command === "lsp") {
    startLanguageServer(values.config);
  } else {
    throw new Error(`Unknown Stillwind command ${command}`);
  }
} catch (error) {
  process.stderr.write(String(error) + "\n");
  process.exitCode = 2;
}
