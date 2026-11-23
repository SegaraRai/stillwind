import { format, type FormatConfig } from "vite-plus/fmt";

/** Shared formatting options used by the root `vite.config.ts` and Stillwind project hooks. */
export const workspaceFmtOptions = {
  sortImports: true,
  sortPackageJson: true,
  astro: true,
  svelte: true,
} satisfies FormatConfig;

/**
 * Formats buffers modified by Stillwind using `vite-plus/fmt` with the workspace formatting
 * options. Unchanged files are skipped.
 */
export const vitePlusFormatHook = {
  name: "vite-plus-fmt",
  async format(file: { readonly path: string; readonly code: string; readonly changed: boolean }) {
    if (!file.changed) {
      return;
    }
    const result = await format(file.path, file.code, workspaceFmtOptions);
    if (result.errors.length) {
      throw new Error(result.errors.map((error) => error.message).join("\n"));
    }
    return result.code;
  },
} as const;
