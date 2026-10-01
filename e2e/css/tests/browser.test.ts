import { registerScript } from "@stillwind/e2e-harness/script";

for (const name of ["emission", "runtime-if", "animation", "view-transition"]) {
  registerScript(name, new URL("../", import.meta.url), "scripts/verify-" + name + ".ts");
}
