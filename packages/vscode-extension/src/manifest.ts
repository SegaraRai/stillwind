import { configurationFiles } from "@stillwind/config";

import packageManifest from "../package.json" with { type: "json" };

/** Manifest shared by Marketplace packaging and browser embeddings. */
export const extensionManifest = {
  name: "stillwind",
  displayName: "Stillwind",
  publisher: "stillwind",
  icon: "icon.png",
  version: packageManifest.version,
  description: "Stillwind diagnostics, CSS hover, completion, colors, and utility sorting",
  license: "MIT",
  homepage: "https://stillwind.dev",
  bugs: {
    url: "https://github.com/SegaraRai/stillwind/issues",
  },
  engines: {
    vscode: "^1.110.0",
  },
  categories: ["Programming Languages", "Linters"],
  main: "./dist/node/extension.cjs",
  browser: "./dist/browser/extension.js",
  activationEvents: [
    "onLanguage:javascript",
    "onLanguage:javascriptreact",
    "onLanguage:typescript",
    "onLanguage:typescriptreact",
    "onLanguage:css",
    ...configurationFiles.map((file) => `workspaceContains:${file}`),
  ],
  capabilities: {
    virtualWorkspaces: true,
    untrustedWorkspaces: {
      supported: false,
      description: "Stillwind evaluates project configuration as JavaScript.",
    },
  },
  contributes: {
    css: {
      customData: ["./dist/css-data.json"],
    },
    commands: [
      {
        command: "stillwind.restart",
        title: "Stillwind: Restart Language Server",
      },
      {
        command: "stillwind.snapshot",
        title: "Stillwind: Capture Workspace",
      },
      {
        command: "stillwind.compile",
        title: "Stillwind: Compile Module",
      },
      {
        command: "stillwind.sortSource",
        title: "Stillwind: Sort Source Text",
      },
      {
        command: "stillwind.sortUtilities",
        title: "Stillwind: Sort Utilities",
      },
    ],
    menus: {
      commandPalette: [
        {
          command: "stillwind.snapshot",
          when: "false",
        },
        {
          command: "stillwind.compile",
          when: "false",
        },
        {
          command: "stillwind.sortSource",
          when: "false",
        },
      ],
    },
    configuration: {
      title: "Stillwind",
      properties: {
        "stillwind.configFile": {
          type: "string",
          default: "",
          description:
            "Configuration path relative to the workspace root. Empty discovers stillwind.config.* (ts, mts, cts, js, mjs, cjs, or json).",
        },
        "stillwind.completion.registryDirectives": {
          type: "boolean",
          default: false,
          description:
            "Include registry directives when editing utility libraries and CSS metadata. This changes editor suggestions only.",
        },
      },
    },
  },
} as const;
