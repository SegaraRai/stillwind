const sources = import.meta.glob<string>("../../example/**/*", {
  query: "?raw",
  import: "default",
  eager: true,
});

/**
 * Files displayed and compiled when a playground workspace starts, keyed by absolute virtual path.
 * Vite embeds the files of `apps/play/example` as text when it builds the site.
 */
export const exampleFiles = new Map<string, string>(
  Object.entries(sources).map(([path, text]) => [
    path.replace("../../example/", "/workspace/"),
    text,
  ]),
);
