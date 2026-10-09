import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const e2e = fileURLToPath(new URL("../../", import.meta.url));
const fixtures = [
  join(e2e, "css"),
  ...["frameworks", "bundlers"].flatMap((group) =>
    readdirSync(join(e2e, group), { withFileTypes: true })
      .filter(
        (entry) =>
          entry.isDirectory() &&
          existsSync(join(e2e, group, entry.name, "vite.config.ts")) &&
          existsSync(join(e2e, group, entry.name, "package.json")),
      )
      .map((entry) => join(e2e, group, entry.name)),
  ),
].sort();

export const fixtureTypechecks = fixtures.map(
  (root) => `${JSON.parse(readFileSync(join(root, "package.json"), "utf8")).name}#typecheck`,
);

/**
 * Returns fixture tasks for the driver and the absolute package directory.
 * Assigns each fixture a separate development and preview port.
 */
export function fixtureTasks(driver: string, root: string) {
  const cli = fileURLToPath(new URL("./cli.ts", import.meta.url));
  const typecheck = fileURLToPath(new URL("./typecheck.ts", import.meta.url));
  const metadata = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
  const routes = Boolean(
    metadata.dependencies?.["@tanstack/react-start"] ||
    metadata.dependencies?.["@tanstack/solid-start"],
  );
  const sveltekit = Boolean(metadata.dependencies?.["@sveltejs/kit"]);
  const generateTypes = driver === "nuxt" || sveltekit;
  const port = 5200 + fixtureTypechecks.indexOf(`${metadata.name}#typecheck`);
  const command = (mode: string) =>
    `node "${cli}" ${driver} ${mode} . ${mode === "serve" ? port + 100 : port}`;
  const dependencies = [
    "@stillwind/integration#build",
    "stillwind#build",
    ...(routes ? ["generate:routes"] : []),
    ...(generateTypes ? ["generate:types"] : []),
  ];

  return {
    ...(generateTypes
      ? {
          "generate:types": {
            command: driver === "nuxt" ? "nuxt prepare" : "svelte-kit sync",
            dependsOn: ["@stillwind/integration#build", "stillwind#build"],
            cache: {
              env: ["STILLWIND_E2E_PROFILE"],
              input: [
                { auto: true },
                "!node_modules/.vite-temp/**",
                "!.nuxt/**",
                "!.svelte-kit/**",
              ],
              output:
                driver === "nuxt"
                  ? [".nuxt/**", "!.nuxt/dist/**", "!.nuxt/dev/**", "!.nuxt/cache/**"]
                  : [
                      ".svelte-kit/tsconfig.json",
                      ".svelte-kit/ambient.d.ts",
                      ".svelte-kit/non-ambient.d.ts",
                      ".svelte-kit/env.d.ts",
                      ".svelte-kit/generated/**",
                      ".svelte-kit/types/**",
                      "node_modules/$app/**",
                    ],
            },
          },
        }
      : {}),
    ...(routes
      ? {
          "generate:routes": {
            command: command("prepare"),
            dependsOn: ["@stillwind/integration#build", "stillwind#build"],
            cache: {
              env: ["STILLWIND_E2E_PROFILE"],
              input: [{ auto: true }, "!node_modules/.vite-temp/**"],
              output: ["src/routeTree.gen.ts"],
            },
          },
        }
      : {}),
    dev: {
      command: command("dev"),
      dependsOn: dependencies,
      cache: false,
    },
    build: {
      command: command("build"),
      dependsOn: dependencies,
      cache: {
        env: ["STILLWIND_E2E_PROFILE"],
        input: [
          { auto: true },
          "!node_modules/.vite-temp/**",
          "!dist/**",
          "!build/**",
          "!.next/**",
          "!.nuxt/**",
          "!.output/**",
          "!.astro/**",
          "!.svelte-kit/**",
          "!build-stats.json",
        ],
        output: [{ auto: true }, "!node_modules/.vite-temp/**", "!**/*.tmp"],
      },
    },
    serve: {
      command: command("serve"),
      dependsOn: ["build"],
      cache: false,
    },
    typecheck: {
      command: `node "${typecheck}"`,
      dependsOn: dependencies,
    },
    "test:e2e": {
      command: "vp test --run --config acceptance.config.ts",
      dependsOn: dependencies,
      cache: false,
    },
  };
}
