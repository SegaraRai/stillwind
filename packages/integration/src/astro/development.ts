import process from "node:process";

const projectsKey = Symbol.for("@stillwind/integration.astro.development-css");

/** Active CSS getters keyed by bridge and server-environment identity. */
type Projects = Map<string, () => Promise<string>>;

// Vite module runners can evaluate another copy of this module in the same process.
const host = process as NodeJS.Process & {
  /** Registry shared across copies evaluated by Vite module runners in this process. */
  [projectsKey]?: Projects;
};

/**
 * Registers an in-process CSS getter, replacing any previous getter for the same identity.
 * @param id - Bridge and server-environment identity used by the generated SSR module.
 * @param getCss - Getter for the current document plan, invoked on each SSR request.
 * @returns A disposer that removes only this registration and preserves a later replacement.
 */
export function registerDevelopmentCss(id: string, getCss: () => Promise<string>): () => void {
  const projects = (host[projectsKey] ??= new Map());
  projects.set(id, getCss);
  return () => {
    const current = host[projectsKey];
    if (current?.get(id) === getCss) {
      current.delete(id);
      if (!current.size) {
        delete host[projectsKey];
      }
    }
  };
}

/**
 * Reads the current CSS from a registered project.
 * @param id - Identity registered by the active development bridge.
 * @returns The current document stylesheet from the registered getter.
 * @throws If the registration is absent or its getter fails.
 */
export async function getDevelopmentCss(id: string): Promise<string> {
  const getCss = host[projectsKey]?.get(id);
  if (!getCss) {
    throw new Error("Stillwind Astro development project is unavailable");
  }
  return getCss();
}
