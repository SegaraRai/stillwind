import type { EmissionCompilation } from "./emission.ts";

declare module "webpack" {
  interface Compilation {
    /** Evaluated settings shared by producer loaders in the current build run. */
    __stillwindConfiguration?: EmissionCompilation["__stillwindConfiguration"];
    /** Reconstructed document CSS, including any inline source map, after producer collection. */
    __stillwindCss?: EmissionCompilation["__stillwindCss"];
  }
}

declare module "@rspack/core" {
  interface Compilation {
    /** Evaluated settings shared by producer loaders in the current build run. */
    __stillwindConfiguration?: EmissionCompilation["__stillwindConfiguration"];
    /** Reconstructed document CSS, including any inline source map, after producer collection. */
    __stillwindCss?: EmissionCompilation["__stillwindCss"];
  }
}
