import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import process from "node:process";

import type { Executable } from "vscode-languageclient/node";

/**
 * `@stillwind/cli` installation that a project controls.
 */
export interface ProjectCli {
  /** Absolute path of the executable module. */
  module: string;
  /** Version from the package manifest. */
  version: string;
}

/**
 * Finds the `@stillwind/cli` installation that Node module resolution selects for a project.
 *
 * @param directories - Absolute directories in priority order. A directory may be missing.
 * @returns The first installation with a `stillwind` executable, or `undefined` when no directory
 * resolves to one. A package with an unreadable manifest counts as missing.
 */
export function findProjectCli(directories: readonly string[]): ProjectCli | undefined {
  for (const directory of directories) {
    try {
      const manifestPath = createRequire(join(directory, "package.json")).resolve(
        "@stillwind/cli/package.json",
      );
      const manifest = JSON.parse(readFileSync(manifestPath, "utf8")) as {
        version: string;
        bin: {
          stillwind: string;
        };
      };
      return {
        module: join(dirname(manifestPath), manifest.bin.stillwind),
        version: manifest.version,
      };
    } catch {
      continue;
    }
  }
  return undefined;
}

/**
 * Describes a CLI server process that runs under the editor runtime.
 *
 * The language client adds `--clientProcessId` to the arguments of a `NodeModule` server, and the
 * CLI rejects unknown options. An `Executable` without `transport` receives its arguments unchanged.
 *
 * @param cli - Project installation to run.
 * @param config - Absolute configuration path, or `undefined` for discovery.
 * @returns Server options for the CLI's `lsp` command.
 */
export function cliServer(cli: ProjectCli, config: string | undefined): Executable {
  return {
    command: process.execPath,
    args: [cli.module, "lsp", "--stdio", ...(config ? ["--config", config] : [])],
    options: {
      env: {
        ...process.env,
        ELECTRON_RUN_AS_NODE: "1",
        ELECTRON_NO_ASAR: "1",
      },
    },
  };
}
