import type { EmissionCompilation } from "./emission.ts";

declare module "webpack" {
  interface Compilation {
    /** Reconstructed document CSS, including any inline source map, after producer collection. */
    __stillwindCss?: EmissionCompilation["__stillwindCss"];
  }
}

declare module "@rspack/core" {
  interface Compilation {
    /** Reconstructed document CSS, including any inline source map, after producer collection. */
    __stillwindCss?: EmissionCompilation["__stillwindCss"];
  }
}
