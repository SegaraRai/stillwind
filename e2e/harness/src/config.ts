import { fileURLToPath } from "node:url";

/**
 * Vite+ tasks shared by fixture packages; application configuration remains local.
 */
export function fixtureTasks(driver = "vite") {
  const cli = fileURLToPath(new URL("./cli.ts", import.meta.url));
  const typecheck = fileURLToPath(new URL("./typecheck.ts", import.meta.url));

  const command = (mode: string) => `node "${cli}" ${driver} ${mode}`;

  return {
    dev: {
      command: command("dev"),
      cache: false,
    },
    build: {
      command: command("build"),
      dependsOn: ["@stillwind/integration#build"],
      cache: false,
    },
    serve: {
      command: command("serve"),
      cache: false,
    },
    typecheck: {
      command: `node "${typecheck}"`,
      cache: false,
      dependsOn: ["@stillwind/integration#build"],
    },
    test: {
      command: "vp test --run --config acceptance.config.ts",
      dependsOn: ["@stillwind/integration#build"],
      cache: false,
    },
    e2e: {
      command: "vp test --run --config acceptance.config.ts",
      dependsOn: ["@stillwind/integration#build"],
      cache: false,
    },
  };
}
