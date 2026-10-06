import { fixtureTasks } from "@stillwind-internal/e2e-harness/config";
import { defineConfig, lazyPlugins } from "vite-plus";

const tasks = fixtureTasks("vite", import.meta.dirname);

export default defineConfig({
  run: {
    tasks: {
      "generate:types": {
        command: "svelte-kit sync",
        cache: {
          env: ["STILLWIND_E2E_PROFILE"],
          input: [{ auto: true }, "!node_modules/.vite-temp/**", "!.svelte-kit/**"],
          output: [".svelte-kit/generated/**", ".svelte-kit/types/**", "node_modules/$app/**"],
        },
        dependsOn: ["@stillwind/integration#build", "stillwind#build"],
      },
      ...Object.fromEntries(
        Object.entries(tasks).map(([name, task]) => [
          name,
          {
            ...task,
            dependsOn: [...task.dependsOn, "generate:types"],
          },
        ]),
      ),
    },
  },
  plugins: lazyPlugins(async () => {
    const { stillwind } = await import("@stillwind/integration/vite");
    const { sveltekit } = await import("@sveltejs/kit/vite");
    const { default: adapter } = await import("@sveltejs/adapter-node");
    return [
      stillwind({
        emission: process.env.STILLWIND_E2E_PROFILE === "split" ? "split" : "single",
      }),
      sveltekit({
        adapter: adapter(),
      }),
    ];
  }),
});
