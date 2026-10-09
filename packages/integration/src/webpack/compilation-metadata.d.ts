import type { EmissionCompilation } from "./emission.ts";

declare module "webpack" {
  /** Stillwind settings and document CSS retained on the webpack compilation. */
  interface Compilation {
    /** Evaluated settings shared by producer loaders in the current build run. */
    __stillwindConfiguration?: EmissionCompilation["__stillwindConfiguration"];
    /** Reconstructed document CSS, including any inline source map, after producer collection. */
    __stillwindCss?: EmissionCompilation["__stillwindCss"];
  }
}

declare module "@rspack/core" {
  /** Stillwind settings and document CSS retained on the Rspack compilation. */
  interface Compilation {
    /** Evaluated settings shared by producer loaders in the current build run. */
    __stillwindConfiguration?: EmissionCompilation["__stillwindConfiguration"];
    /** Reconstructed document CSS, including any inline source map, after producer collection. */
    __stillwindCss?: EmissionCompilation["__stillwindCss"];
  }
}
