import { fixtureTasks } from "@stillwind/e2e-harness/config";
import { defineConfig } from "vite-plus";

export default defineConfig({
  run: {
    tasks: fixtureTasks("next"),
  },
});
