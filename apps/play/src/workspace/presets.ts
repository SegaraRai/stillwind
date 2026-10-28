const sources = import.meta.glob<string>("../../examples/*/**/*", {
  query: "?raw",
  import: "default",
  eager: true,
});

/**
 * Workspace that the playground can start from.
 */
export interface Preset {
  /** Directory name under `apps/play/examples`, also used in URLs. */
  id: string;
  title: string;
  /** Initial files keyed by absolute virtual path under `/workspace/`. */
  files: Map<string, string>;
}

/**
 * Presets in menu order. The first preset is the default, so it stays small enough to serve as
 * the starting point of a reproduction. Vite embeds the files of `apps/play/examples/<id>` as
 * text when it builds the site.
 */
export const presets: Preset[] = [
  {
    id: "minimal",
    title: "Minimal",
  },
  {
    id: "release-checklist",
    title: "Release checklist",
  },
].map(({ id, title }) => {
  const prefix = `../../examples/${id}/`;
  const files = new Map(
    Object.entries(sources)
      .filter(([path]) => path.startsWith(prefix))
      .map(([path, text]) => [`/workspace/${path.slice(prefix.length)}`, text]),
  );
  if (files.size === 0) {
    throw new Error(`Preset ${id} has no files`);
  }
  return {
    id,
    title,
    files,
  };
});
