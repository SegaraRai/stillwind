import process from "node:process";

const projectsKey = Symbol.for("@stillwind/integration.astro.development-css");

type Projects = Map<string, () => Promise<string>>;

// Vite module runners can evaluate another copy of this module in the same process.
const host = process as NodeJS.Process & { [projectsKey]?: Projects };

/** Registers an in-process CSS getter and returns a disposer for that registration. */
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

/** Reads the current CSS from a registered project. Unknown or closed projects reject. */
export async function getDevelopmentCss(id: string): Promise<string> {
  const getCss = host[projectsKey]?.get(id);
  if (!getCss) {
    throw new Error("Stillwind Astro development project is unavailable");
  }
  return getCss();
}
