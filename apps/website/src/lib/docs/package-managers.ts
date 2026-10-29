import type { MdastPluginInput } from "@cloudflare/nimbus-docs/types";

/** Package managers in tab order. Documentation pages write their commands for `vp`. */
export const packageManagers = [
  { id: "vp", label: "Vite+" },
  { id: "npm", label: "npm" },
  { id: "pnpm", label: "pnpm" },
  { id: "yarn", label: "Yarn" },
  { id: "bun", label: "Bun" },
] as const;

export type PackageManager = (typeof packageManagers)[number]["id"];

/** Local storage key for the package manager that the reader selected last. */
export const packageManagerStorageKey = "stillwind:package-manager";

type Commands = Record<Exclude<PackageManager, "vp">, string>;

const installCommands: Commands = {
  npm: "npm install",
  pnpm: "pnpm install",
  yarn: "yarn install",
  bun: "bun install",
};

const addCommands: Commands = {
  npm: "npm install",
  pnpm: "pnpm add",
  yarn: "yarn add",
  bun: "bun add",
};

const devFlags: Commands = {
  npm: "-D",
  pnpm: "-D",
  yarn: "-D",
  bun: "--dev",
};

const prefixes: Record<string, Commands> = {
  // `npx` and `bunx` run a bin from local dependencies before they look up a package with that name.
  exec: {
    npm: "npx",
    pnpm: "pnpm exec",
    yarn: "yarn",
    bun: "bunx",
  },
  dlx: {
    npm: "npx",
    pnpm: "pnpm dlx",
    yarn: "yarn dlx",
    bun: "bunx",
  },
  run: {
    npm: "npm run",
    pnpm: "pnpm run",
    yarn: "yarn run",
    bun: "bun run",
  },
  remove: {
    npm: "npm uninstall",
    pnpm: "pnpm remove",
    yarn: "yarn remove",
    bun: "bun remove",
  },
};

/**
 * Returns the command for `manager` that does the same as the `vp` command `line`. Lines that do
 * not start with `vp` are returned unchanged.
 *
 * Throws when `line` uses a `vp` subcommand or an install option that has no translation.
 */
export function packageManagerCommand(line: string, manager: PackageManager): string {
  const match = /^vp\s+(\S+)\s*(.*)$/.exec(line);
  if (!match || manager === "vp") {
    return line;
  }
  const [, command, rest] = match as unknown as [string, string, string];
  if (command === "i" || command === "install" || command === "add") {
    const args = rest.split(/\s+/).filter(Boolean);
    const dev = args.some((arg) => arg === "-D" || arg === "--save-dev");
    const packages = args.filter((arg) => arg !== "-D" && arg !== "--save-dev");
    const option = packages.find((arg) => arg.startsWith("-"));
    if (option) {
      throw new Error(`Unsupported install option ${option} in \`${line}\``);
    }
    if (!packages.length) {
      return installCommands[manager];
    }
    return [addCommands[manager], ...(dev ? [devFlags[manager]] : []), ...packages].join(" ");
  }
  const prefix = prefixes[command]?.[manager];
  if (!prefix) {
    throw new Error(`Unsupported vp command in \`${line}\``);
  }
  return rest ? `${prefix} ${rest}` : prefix;
}

/**
 * Replaces each shell code block that contains `vp` commands with tabs that show the commands for
 * each package manager. Other lines in such a block must be empty or comments.
 */
export const packageManagerPlugin: MdastPluginInput = {
  name: "stillwind-package-managers",
  code(node, context) {
    if (!["sh", "bash", "shell"].includes(node.lang ?? "")) {
      return;
    }
    const lines = node.value.split("\n");
    if (!lines.some((line) => /^vp\s/.test(line))) {
      return;
    }
    const other = lines.find((line) => line.trim() && !/^(vp\s|#)/.test(line));
    if (other !== undefined) {
      throw new Error(`A code block with vp commands contains another command: \`${other}\``);
    }
    context.replaceNode(node, {
      type: "packageManagers",
      data: {
        hName: "div",
        hProperties: {
          dataPackageManagers: "",
        },
      },
      children: [
        {
          type: "packageManagerTabs",
          data: {
            hName: "div",
            hProperties: {
              role: "tablist",
              ariaLabel: "Package manager",
            },
          },
          children: packageManagers.map(({ id, label }) => ({
            type: "packageManagerTab",
            data: {
              hName: "button",
              hProperties: {
                type: "button",
                role: "tab",
                ariaSelected: String(id === "vp"),
                dataPackageManager: id,
              },
            },
            children: [
              {
                type: "text",
                value: label,
              },
            ],
          })),
        },
        ...packageManagers.map(({ id, label }) => ({
          type: "packageManagerPanel",
          data: {
            hName: "div",
            hProperties: {
              role: "tabpanel",
              ariaLabel: label,
              dataPackageManager: id,
            },
          },
          children: [
            {
              type: "code" as const,
              lang: node.lang,
              meta: node.meta,
              value: lines.map((line) => packageManagerCommand(line, id)).join("\n"),
            },
          ],
        })),
      ],
    });
  },
};
