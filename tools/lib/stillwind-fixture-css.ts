/** Runs upstream utility fixtures against Stillwind's explicitly selected registry. */
export function stillwindFixtureCss(css: string): string {
  const entries: Record<string, string> = {
    tailwindcss: "stillwind",
    "tailwindcss/theme": "stillwind/theme.css",
    "tailwindcss/preflight": "stillwind/preflight.css",
    "tailwindcss/utilities": "stillwind/utilities.css",
  };
  return (
    css.replace(
      /(@import\s+)(["'])(tailwindcss(?:\/(?:theme|preflight|utilities))?)\2/g,
      (_, keyword: string, quote: string, name: string) =>
        `${keyword}${quote}${entries[name]}${quote}`,
    ) + '\n@import "stillwind/builtins.css";'
  );
}
