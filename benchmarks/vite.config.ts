import { defineConfig } from "vite-plus";
export default defineConfig({
  test: { include: ["source-map/*.test.ts"] },
  run: {
    tasks: {
      test: { command: "vp test --run", cache: false },
      typecheck: { command: "tsc --noEmit", dependsOn: ["@stillwind/host#build"] },
      compiler: {
        command: "node compiler/run.ts",
        dependsOn: ["@stillwind/native#build"],
        cache: false,
      },
      host: {
        command: "node compiler/host.ts",
        dependsOn: ["@stillwind/host#build"],
        cache: false,
      },
      "source-map": {
        command: "node source-map/run.ts",
        dependsOn: ["@stillwind/host#build"],
        cache: false,
      },
    },
  },
});
