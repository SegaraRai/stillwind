import { resolveFixtureCss } from "@stillwind-internal/tools-fixture-css/stillwind-css";

/** Runs upstream utility fixtures against Stillwind's explicitly selected registry. */
export function stillwindFixtureCss(css: string): string {
  const entries: Record<string, string> = {
    tailwindcss: "stillwind",
    "tailwindcss/theme": "stillwind/theme.css",
    "tailwindcss/preflight": "stillwind/preflight.css",
    "tailwindcss/utilities": "stillwind/utilities.css",
  };
  const selected = css.replace(
    /(@import\s+)(["'])(tailwindcss(?:\/(?:theme|preflight|utilities))?)\2/g,
    (_, keyword: string, quote: string, name: string) =>
      `${keyword}${quote}${entries[name]}${quote}`,
  );
  return resolveFixtureCss(
    /@import\s+["']stillwind(?:\/builtins\.css)?["']/.test(selected)
      ? selected
      : selected + '\n@import "stillwind/builtins.css";',
  );
}
